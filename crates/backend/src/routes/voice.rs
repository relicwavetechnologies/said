//! POST /v1/voice/polish
//!
//! Receives a multipart form with:
//!   audio        — WAV bytes  (required)
//!   target_app   — bundle-id of the focused app  (optional)
//!   pre_transcript — transcript already obtained via Deepgram WS streaming  (optional, P5)
//!
//! Pipeline: auth → load prefs → STT (skipped if pre_transcript present) → embed‖prompt →
//!           RAG → LLM stream → SSE.
//!
//! P2: Deepgram embedding is spawned concurrently with prompt-skeleton building
//!     the moment the transcript is available.
//!
//! P5: If the caller (Tauri) already streamed audio to Deepgram via WebSocket and
//!     has a transcript ready, it is supplied as `pre_transcript`.  The STT step is
//!     skipped, saving ~1.2–2 s on every recording.

use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    response::{
        IntoResponse,
        sse::{Event, KeepAlive, Sse},
    },
};
use serde_json::json;
use std::convert::Infallible;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use uuid::Uuid;

// ── Audio file helpers ────────────────────────────────────────────────────────

/// Directory where WAV recordings are saved locally (1-day retention).
fn audio_dir() -> std::path::PathBuf {
    let base = dirs::data_local_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".local/share")))
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
    base.join("VoicePolish").join("audio")
}

/// Save WAV bytes to disk. Returns the path on success.
fn save_audio(id: &str, data: &[u8]) -> Option<std::path::PathBuf> {
    let dir = audio_dir();
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(format!("{id}.wav"));
    std::fs::write(&path, data).ok()?;
    debug!("[voice] saved audio to {}", path.display());
    Some(path)
}

/// Delete WAV files older than 24 hours. Called from the cleanup task.
pub fn cleanup_old_audio() {
    let dir = audio_dir();
    let cutoff = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(86_400))
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        let Ok(modified) = meta.modified() else {
            continue;
        };
        if modified < cutoff {
            let _ = std::fs::remove_file(entry.path());
            debug!("[voice] deleted old audio {}", entry.path().display());
        }
    }
}

use crate::{
    AppState,
    embedder::gemini,
    llm::{
        gateway, gemini_direct, groq, openai_codex,
        prompt::{build_system_prompt_with_vocab_entries, build_user_message, VocabEntry},
    },
    store::{
        history::{InsertRecording, insert_recording},
        openai_oauth, stt_replacements,
        vectors::retrieve_similar,
        vocab_embeddings, vocabulary,
    },
    stt::deepgram,
};

pub async fn polish(State(state): State<AppState>, mut multipart: Multipart) -> impl IntoResponse {
    // ── Extract multipart fields ───────────────────────────────────────────────
    let mut wav_data: Vec<u8> = Vec::new();
    let mut target_app: Option<String> = None;
    let mut pre_transcript: Option<String> = None; // P5: from Deepgram WS

    while let Ok(Some(field)) = multipart.next_field().await {
        match field.name() {
            Some("audio") => {
                wav_data = field.bytes().await.unwrap_or_default().to_vec();
            }
            Some("target_app") => {
                target_app = field.text().await.ok();
            }
            Some("pre_transcript") => {
                pre_transcript = field.text().await.ok().filter(|s| !s.is_empty());
            }
            _ => {}
        }
    }

    // Allow empty WAV when the caller supplied a pre_transcript (P5 / WS path).
    if wav_data.is_empty() && pre_transcript.is_none() {
        warn!("[voice] received empty audio and no pre_transcript");
        return StatusCode::BAD_REQUEST.into_response();
    }

    // Save audio to disk (1-day retention) before exposing audio_id in history.
    // This costs only a few ms, and prevents UI play/download buttons from
    // pointing at a WAV file that failed to save.
    let audio_id = Uuid::new_v4().to_string();
    let saved_audio_id = if !wav_data.is_empty() {
        let aid = audio_id.clone();
        let data = wav_data.clone();
        match tokio::task::spawn_blocking(move || save_audio(&aid, &data).is_some()).await {
            Ok(true) => Some(audio_id.clone()),
            Ok(false) => {
                warn!("[voice] failed to save audio");
                None
            }
            Err(e) => {
                warn!("[voice] audio save task failed: {e}");
                None
            }
        }
    } else {
        None
    };

    let user_id = state.default_user_id.as_str().to_string();
    let pool = state.pool.clone();

    let http_client = state.http_client.clone();

    // ── Pre-fetch all DB-backed data in parallel, BEFORE opening the SSE stream ──
    // Prefs (async RwLock), lexicon (async RwLock), and vocab terms (spawn_blocking)
    // run concurrently so total wait ≈ max(each) instead of their sum (~8 ms saved).
    let vocab_task = {
        let pool_c = pool.clone();
        let uid_c = user_id.clone();
        // Load full VocabTerm rows so we can carry example_context into the
        // polish prompt — the foundational signal that lets the LLM do
        // context-aware recognition of unseen STT mishearings.
        tokio::task::spawn_blocking(move || vocabulary::top_terms(&pool_c, &uid_c, 100))
    };
    let (prefs_opt, (word_corrections, stt_replacement_rules), vocab_full) = tokio::join!(
        crate::get_prefs_cached(&state.prefs_cache, &pool, &user_id),
        crate::get_lexicon_cached(&state.lexicon_cache, &pool, &user_id),
        async { vocab_task.await.unwrap_or_default() },
    );
    // Bare term strings for Deepgram keyterm bias (always all top terms by
    // weight — Deepgram bias has no context awareness, so we feed it the
    // most-trusted slate).
    let vocab_terms: Vec<String> = vocab_full.iter().map(|v| v.term.clone()).collect();
    // The polish-prompt vocab slice is computed below, AFTER the transcript
    // embedding lands, so we can do relevance retrieval. Reserve the binding
    // here so the existing build_system_prompt_with_vocab_entries call site
    // doesn't need to move.
    let vocab_entries_pre: Vec<VocabEntry> = vocab_full
        .iter()
        .map(|v| VocabEntry {
            term:      v.term.clone(),
            context:   v.example_context.clone(),
            term_type: v.term_type.clone(),
        })
        .collect();

    // ── Build SSE stream ───────────────────────────────────────────────────────
    let audio_id_ref = saved_audio_id.clone();
    let stream = async_stream::stream! {
        let total_start = Instant::now();
        let aid = audio_id_ref.as_deref();

        let prefs = match prefs_opt {
            Some(p) => p,
            None => {
                yield Ok::<Event, Infallible>(
                    Event::default().event("error").data(
                        json!({"message": "preferences not found", "audio_id": aid}).to_string()
                    )
                );
                return;
            }
        };

        let deepgram_key = prefs.deepgram_api_key.clone()
            .or_else(|| std::env::var("DEEPGRAM_API_KEY").ok())
            .unwrap_or_default();
        let gemini_key = prefs.gemini_api_key.clone()
            .or_else(|| std::env::var("GEMINI_API_KEY").ok())
            .unwrap_or_default();
        let gateway_key = prefs.gateway_api_key.clone()
            .or_else(|| std::env::var("GATEWAY_API_KEY").ok())
            .or_else(|| { let k = voice_polish_core::api_key(); if k.is_empty() { None } else { Some(k.to_string()) } })
            .unwrap_or_default();
        let groq_key = prefs.groq_api_key.clone()
            .or_else(|| std::env::var("GROQ_API_KEY").ok())
            .unwrap_or_default();

        // ── STEP 1: STT ───────────────────────────────────────────────────────────
        let (stt_transcript_raw, enriched_raw, stt_confidence, transcribe_ms) = if let Some(t) = pre_transcript {
            let plain = strip_confidence_markers(&t);
            let ms = total_start.elapsed().as_millis();
            info!("[timing] STT=0ms (WS pre-transcript, {} words) @{ms}ms", plain.split_whitespace().count());
            (plain, t, 0.95_f64, 0_i64)
        } else {
            yield Ok(Event::default().event("status")
                .data(json!({"phase": "transcribing"}).to_string()));
            let t_start = Instant::now();
            match deepgram::transcribe(&http_client, &deepgram_key, wav_data, &prefs.language, &vocab_terms).await {
                Ok(r) => {
                    let ms = t_start.elapsed().as_millis() as i64;
                    info!("[timing] STT={}ms (batch, {} words, conf={:.2})",
                        ms, r.transcript.split_whitespace().count(), r.confidence);
                    (r.transcript, r.enriched_transcript, r.confidence, ms)
                }
                Err(e) => {
                    warn!("[voice] STT error: {e}");
                    yield Ok(Event::default().event("error").data(
                        json!({"message": e, "audio_id": aid}).to_string()
                    ));
                    return;
                }
            }
        };

        let (stt_transcript, enriched_transcript) = if stt_replacement_rules.is_empty() {
            (stt_transcript_raw, enriched_raw)
        } else {
            let plain_rewritten = stt_replacements::apply(&stt_transcript_raw, &stt_replacement_rules);
            let enriched_rewritten = stt_replacements::apply(&enriched_raw, &stt_replacement_rules);
            if plain_rewritten != stt_transcript_raw {
                info!("[voice] lexicon replacement: {:?} → {:?}", stt_transcript_raw, plain_rewritten);
            }
            (plain_rewritten, enriched_rewritten)
        };

        let status_payload = json!({"phase": "polishing", "transcript": &stt_transcript}).to_string();
        yield Ok(Event::default().event("status").data(status_payload));

        // ── STEP 2: Embed (awaited — needed for vocab relevance selection) ─────────
        // Earlier this was fire-and-forget for hot-path latency. We now await
        // it because vocab relevance selection (step 4) needs a query
        // embedding to pick the right vocab entries. The embedding still
        // populates the cache so the NEXT recording's RAG benefits too.
        // Cache hits are < 1ms; cold call is 50-150ms.
        let transcript_for_embed = stt_transcript.clone();
        let http_for_embed       = http_client.clone();
        let pool_for_embed       = pool.clone();
        let gemini_key_embed     = gemini_key.clone();
        let embed_t0 = tokio::time::Instant::now();
        let embedding = gemini::embed(&http_for_embed, &pool_for_embed, &transcript_for_embed, &gemini_key_embed).await;
        let embed_ms = embed_t0.elapsed().as_millis() as i64;
        info!("[timing] embed={}ms ({})", embed_ms, if embedding.is_some() { "ok" } else { "skip/no-key" });

        let model        = voice_polish_core::resolve_model(&prefs.selected_model).to_string();
        let user_message = build_user_message(&enriched_transcript, &prefs.output_language);

        // ── STEP 3: RAG retrieval — k-NN over preference_vectors ──────────────────
        let rag_examples = match &embedding {
            Some(emb) => {
                let emb_clone = emb.clone();
                let pool_rag  = pool.clone();
                let uid_rag   = user_id.clone();
                tokio::task::spawn_blocking(move || {
                    retrieve_similar(&pool_rag, &uid_rag, &emb_clone, 5, 0.65)
                }).await.unwrap_or_default()
            }
            None => vec![],
        };
        let rag_ms: u128 = 0; // included in embed_ms above
        let examples_used = rag_examples.len();
        info!("[rag] {} example(s) retrieved", examples_used);

        // ── STEP 4: Relevance-aware vocabulary slice ──────────────────────────────
        // Use the transcript embedding to pick the vocab entries that match
        // what the user actually said. Skip flooding the prompt with all 200
        // vocab rows — pick starred + top-weight + top-relevance (deduped,
        // capped at 25). Falls back to starred + top-weight when no embedding.
        let vocab_entries: Vec<VocabEntry> = {
            let pool_v   = pool.clone();
            let uid_v    = user_id.clone();
            let lang_v   = prefs.output_language.clone();
            let emb_v    = embedding.clone();
            const N_TOP_WEIGHT: usize = 8;
            const K_RELEVANT:   usize = 12;
            const MAX_TOTAL:    usize = 25;
            const MIN_SIM:      f32   = 0.55;
            let txt_v = stt_transcript.clone();
            let chosen = tokio::task::spawn_blocking(move || {
                // Hybrid selector: dense (cosine on time-decayed centroids)
                // ⊕ sparse (BM25 on term + example_context) fused via RRF.
                // Dense alone misses exact-match jargon (acronyms, IDs);
                // BM25 alone misses semantic neighbours. Together they
                // catch ~15-30% more relevant entries (Weaviate / OpenSearch
                // hybrid-search docs).
                vocab_embeddings::select_for_polish_hybrid(
                    &pool_v, &uid_v, &lang_v,
                    emb_v.as_deref(), Some(&txt_v),
                    N_TOP_WEIGHT, K_RELEVANT, MAX_TOTAL, MIN_SIM,
                )
            }).await.unwrap_or_default();
            if chosen.is_empty() {
                // Fresh install / no embeddings yet — fall back to the full
                // pre-computed slate so existing users aren't degraded.
                info!("[voice] vocab selector empty — using full slate ({} entries)",
                      vocab_entries_pre.len());
                vocab_entries_pre.clone()
            } else {
                info!("[voice] vocab selector picked {}/{} entries (relevance-aware)",
                      chosen.len(), vocab_full.len());
                chosen.into_iter().map(|v| VocabEntry {
                    term:      v.term,
                    context:   v.example_context,
                    term_type: v.term_type,
                }).collect()
            }
        };

        let system_prompt = build_system_prompt_with_vocab_entries(
            &prefs, &rag_examples, &word_corrections, &vocab_entries,
        );

        // ── STEP 5: LLM stream ────────────────────────────────────────────────────
        let llm_provider  = prefs.llm_provider.clone();
        let (token_tx, mut token_rx) = mpsc::channel::<String>(64);
        let sys_p       = system_prompt.clone();
        let usr_m       = user_message.clone();
        let client_c    = http_client.clone();

        let (model_for_llm, openai_token_opt) = if llm_provider == "openai_codex" {
            let pool_tok = pool.clone();
            let uid_tok  = user_id.clone();
            let tok = tokio::task::spawn_blocking(move || openai_oauth::get_token(&pool_tok, &uid_tok))
                .await
                .unwrap_or(None);
            let m = openai_codex::MODEL_MINI.to_string();
            (m, tok.map(|t| t.access_token))
        } else if llm_provider == "gemini_direct" {
            (gemini_direct::GEMINI_DIRECT_MODEL.to_string(), None)
        } else if llm_provider == "groq" {
            (groq::GROQ_MODEL_DEFAULT.to_string(), None)
        } else {
            (model.clone(), None)
        };

        let gk          = gateway_key.clone();
        let gk_gemini   = gemini_key.clone();
        let gk_groq     = groq_key.clone();

        let llm_start = Instant::now();
        info!("[timing] LLM start — provider={llm_provider:?} model={model_for_llm:?}");

        let llm_task = tokio::spawn(async move {
            if llm_provider == "openai_codex" {
                let access_token = openai_token_opt.as_deref().unwrap_or("");
                if access_token.is_empty() {
                    return Err("OpenAI not connected — go to Settings to connect your account".to_string());
                }
                openai_codex::stream_polish(
                    &client_c, access_token, &model_for_llm, &sys_p, &usr_m, token_tx,
                ).await
            } else if llm_provider == "gemini_direct" {
                gemini_direct::stream_polish(
                    &client_c, &gk_gemini, &model_for_llm, &sys_p, &usr_m, token_tx,
                ).await
            } else if llm_provider == "groq" {
                groq::stream_polish(
                    &client_c, &gk_groq, &model_for_llm, &sys_p, &usr_m, token_tx,
                ).await
            } else {
                gateway::stream_polish(&client_c, &gk, &model_for_llm, &sys_p, &usr_m, token_tx).await
            }
        });

        // Yield each token as an SSE event
        while let Some(token) = token_rx.recv().await {
            yield Ok(Event::default().event("token")
                .data(json!({"token": token}).to_string()));
        }

        let llm_result = match llm_task.await {
            Ok(Ok(r))   => r,
            Ok(Err(e))  => {
                warn!("[voice] LLM error: {e}");
                yield Ok(Event::default().event("error").data(
                    json!({"message": e, "audio_id": aid}).to_string()
                ));
                return;
            }
            Err(e) => {
                warn!("[voice] LLM task panicked: {e}");
                yield Ok(Event::default().event("error").data(
                    json!({"message": "internal error", "audio_id": aid}).to_string()
                ));
                return;
            }
        };

        let llm_ms   = llm_start.elapsed().as_millis() as i64;
        let total_ms = total_start.elapsed().as_millis() as i64;
        let word_count = llm_result.polished.split_whitespace().count() as i64;
        info!("[timing] LLM={}ms (TTFT inside) | total={}ms ← STT={}ms embed={}ms rag={}ms llm={}ms",
            llm_ms, total_ms, transcribe_ms, embed_ms, rag_ms, llm_ms);

        let recording_id = Uuid::new_v4().to_string();

        // 7. Persist recording before emitting `done`, so the UI refresh that
        // follows the done event can see both the row and its audio_id.
        {
            let pool2   = pool.clone();
            let id2     = recording_id.clone();
            let uid2    = user_id.clone();
            let t2      = stt_transcript.clone();
            let p2      = llm_result.polished.clone();
            let ta2     = target_app.clone();
            let model2  = model.clone();
            let conf    = stt_confidence;
            let t_ms    = transcribe_ms;
            let e_ms    = embed_ms;
            let p_ms    = llm_result.polish_ms as i64;
            let aid2    = saved_audio_id.clone();
            let inserted = tokio::task::spawn_blocking(move || {
                insert_recording(&pool2, InsertRecording {
                    id: &id2, user_id: &uid2,
                    transcript: &t2, polished: &p2,
                    word_count, recording_seconds: (total_ms as f64 / 1000.0),
                    model_used: &model2,
                    confidence:    Some(conf),
                    transcribe_ms: Some(t_ms),
                    embed_ms:      Some(e_ms),
                    polish_ms:     Some(p_ms),
                    target_app:    ta2.as_deref(),
                    source:        "voice",
                    audio_id:      aid2.as_deref(),
                }).is_some()
            }).await.unwrap_or(false);
            if !inserted {
                warn!("[voice] failed to insert recording history row");
            }

            // Reinforcement-on-use: bump last_used + use_count for vocab
            // terms that were in this polish prompt. This is the "use
            // signal" half of the time-decay scoring — terms that get
            // surfaced AND retained (the polish completed without error)
            // get rewarded, freshening their decay clock and pushing them
            // up the rank for future similar transcripts.
            let pool3  = pool.clone();
            let uid3   = user_id.clone();
            let terms3: Vec<String> = vocab_entries.iter().map(|e| e.term.clone()).collect();
            tokio::task::spawn_blocking(move || {
                vocab_embeddings::bump_last_used(&pool3, &uid3, &terms3);
            });
        }

        yield Ok(Event::default().event("done").data(
            json!({
                "recording_id": recording_id,
                "polished":     llm_result.polished,
                "model_used":   model,
                "confidence":   stt_confidence,
                "latency_ms": {
                    "stt":      transcribe_ms,
                    "embed":    embed_ms,
                    "rag":      rag_ms,
                    "llm":      llm_ms,
                    "total":    total_ms,
                },
                "examples_used": examples_used,
            })
            .to_string()
        ));
    };

    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

/// Strip `[word?XX%]` confidence markers from an enriched transcript
/// to recover the plain text (for DB storage, embedding, display).
fn strip_confidence_markers(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '[' {
            // Try to parse [word?XX%] — extract just the word
            let mut inner = String::new();
            let mut found_close = false;
            for ic in chars.by_ref() {
                if ic == ']' {
                    found_close = true;
                    break;
                }
                inner.push(ic);
            }
            if found_close {
                if let Some(qpos) = inner.rfind('?') {
                    let after_q = &inner[qpos + 1..];
                    if after_q.ends_with('%') && after_q[..after_q.len() - 1].parse::<f64>().is_ok()
                    {
                        // Valid confidence marker — emit just the word part
                        result.push_str(&inner[..qpos]);
                        continue;
                    }
                }
                // Not a valid marker — emit the brackets and content as-is
                result.push('[');
                result.push_str(&inner);
                result.push(']');
            } else {
                result.push('[');
                result.push_str(&inner);
            }
        } else {
            result.push(c);
        }
    }

    result
}

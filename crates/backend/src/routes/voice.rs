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
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
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

    let Ok(entries) = std::fs::read_dir(&dir) else { return };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        let Ok(modified) = meta.modified() else { continue };
        if modified < cutoff {
            let _ = std::fs::remove_file(entry.path());
            debug!("[voice] deleted old audio {}", entry.path().display());
        }
    }
}

use crate::{
    embedder::gemini,
    llm::{gateway, gemini_direct, groq, openai_codex, prompt::{build_system_prompt_with_vocab, build_user_message}},
    stt::deepgram,
    store::{
        corrections,
        history::{insert_recording, InsertRecording},
        openai_oauth,
        stt_replacements,
        vectors::retrieve_similar,
        vocabulary,
    },
    AppState,
};

pub async fn polish(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    // ── Extract multipart fields ───────────────────────────────────────────────
    let mut wav_data:       Vec<u8>         = Vec::new();
    let mut target_app:     Option<String>  = None;
    let mut pre_transcript: Option<String>  = None;  // P5: from Deepgram WS

    while let Ok(Some(field)) = multipart.next_field().await {
        match field.name() {
            Some("audio")          => { wav_data      = field.bytes().await.unwrap_or_default().to_vec(); }
            Some("target_app")     => { target_app    = field.text().await.ok(); }
            Some("pre_transcript") => { pre_transcript = field.text().await.ok().filter(|s| !s.is_empty()); }
            _ => {}
        }
    }

    if wav_data.is_empty() {
        warn!("[voice] received empty audio");
        return StatusCode::BAD_REQUEST.into_response();
    }

    // Save audio to disk for 1-day retention (retry support)
    let audio_id = Uuid::new_v4().to_string();
    save_audio(&audio_id, &wav_data);

    let user_id = state.default_user_id.as_str().to_string();
    let pool    = state.pool.clone();

    // Load prefs, lexicon (corrections + stt_replacements), and grab shared HTTP
    // client — all BEFORE entering the stream so we don't block on .await inside
    // async_stream::stream! and to avoid creating a new Client per request.
    let prefs_opt = crate::get_prefs_cached(&state.prefs_cache, &pool, &user_id).await;
    let (word_corrections, stt_replacement_rules) =
        crate::get_lexicon_cached(&state.lexicon_cache, &pool, &user_id).await;
    let http_client = state.http_client.clone();

    // ── Build SSE stream ───────────────────────────────────────────────────────
    let audio_id_ref = audio_id.clone();
    let stream = async_stream::stream! {
        let total_start = Instant::now();
        let aid = audio_id_ref.as_str(); // available for error payloads

        // 1. Load prefs (already fetched above — just unwrap here)
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
        debug!(
            "[voice] prefs: lang={:?} tone={:?} model={:?}",
            prefs.output_language, prefs.tone_preset, prefs.selected_model
        );

        // Keys: prefer DB-stored value, fall back to env var (useful in dev)
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

        // ── Personal STT bias: load top vocabulary terms (capped) ──────────────
        // rusqlite is synchronous — off-load to a blocking thread so we don't
        // stall the async executor (avoids jitter on the SSE stream open).
        let pool_clone = pool.clone();
        let uid_clone  = user_id.clone();
        let vocab_terms = tokio::task::spawn_blocking(move || {
            vocabulary::top_term_strings(&pool_clone, &uid_clone, 100)
        }).await.unwrap_or_default();
        if !vocab_terms.is_empty() {
            info!("[voice] biasing STT with {} personal term(s)", vocab_terms.len());
        }

        // 2. Transcribe (skip if pre_transcript supplied by Tauri WS pipeline)
        //
        // Two transcripts are maintained:
        //   stt_transcript_raw — plain text from Deepgram (or pre_transcript stripped)
        //   enriched_raw       — same text but with [word?XX%] markers on
        //                        low-confidence words (sent to the LLM)
        let (stt_transcript_raw, enriched_raw, stt_confidence, transcribe_ms) = if let Some(t) = pre_transcript {
            // P5: Tauri already had a transcript from Deepgram WS — skip HTTP STT.
            // The WS path sends an enriched transcript (with [word?XX%] markers)
            // as-is.  It contains both the plain words and the confidence markers.
            info!("[voice] using pre-transcript ({} chars) — STT step skipped", t.len());
            // Strip markers for the plain version (used in DB/embedding)
            let plain = strip_confidence_markers(&t);
            (plain, t, 0.95_f64, 0_i64)
        } else {
            // Normal path: send WAV to Deepgram HTTP batch API
            yield Ok(Event::default().event("status")
                .data(json!({"phase": "transcribing"}).to_string()));

            let t_start = Instant::now();
            match deepgram::transcribe(&http_client, &deepgram_key, wav_data, &prefs.language, &vocab_terms).await {
                Ok(r) => {
                    let ms = t_start.elapsed().as_millis() as i64;
                    info!("[voice] transcript in {ms}ms ({} uncertain words): {}", r.uncertain_count, r.transcript);
                    if r.uncertain_count > 0 {
                        info!("[voice] enriched for LLM: {}", r.enriched_transcript);
                    }
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

        // ── Apply post-STT phonetic/literal replacements ───────────────────────
        // Applied to BOTH the plain transcript (used for storage/embedding) and
        // the enriched transcript (sent to the LLM).  apply() preserves the
        // surrounding [...?XX%] punctuation around tokens it rewrites, so a
        // marker like "[written?47%]" becomes "[n8n?47%]" — the LLM still sees
        // the uncertainty signal but on the corrected token.
        // stt_replacement_rules loaded from LexiconCache before stream started
        let (stt_transcript, enriched_transcript) = if stt_replacement_rules.is_empty() {
            (stt_transcript_raw, enriched_raw)
        } else {
            let plain_rewritten = stt_replacements::apply(&stt_transcript_raw, &stt_replacement_rules);
            let enriched_rewritten = stt_replacements::apply(&enriched_raw, &stt_replacement_rules);
            if plain_rewritten != stt_transcript_raw {
                info!(
                    "[voice] post-STT replacement applied: {:?} → {:?}",
                    stt_transcript_raw, plain_rewritten
                );
            }
            (plain_rewritten, enriched_rewritten)
        };

        let status_payload = json!({"phase": "polishing", "transcript": &stt_transcript}).to_string();
        yield Ok(Event::default().event("status").data(status_payload));

        // ── P2: Spawn embedding immediately — runs in parallel with prompt building ──
        let transcript_for_embed = stt_transcript.clone();
        let http_for_embed       = http_client.clone();
        let pool_for_embed       = pool.clone();
        let gemini_key_embed     = gemini_key.clone();
        let embed_start          = Instant::now();

        let embed_task = tokio::spawn(async move {
            gemini::embed(&http_for_embed, &pool_for_embed, &transcript_for_embed, &gemini_key_embed).await
        });

        // Build prompt skeleton concurrently while embedding runs
        // (RAG examples will be injected once the embedding is available)
        let model = voice_polish_core::resolve_model(&prefs.selected_model).to_string();
        // Send the ENRICHED transcript to the LLM (with [word?XX%] markers)
        // so it knows which words to scrutinize for context-based correction.
        let user_message = build_user_message(&enriched_transcript, &prefs.output_language);

        // 3. Wait for embedding result (P2: overlapped with prompt prep above)
        let embedding  = embed_task.await.unwrap_or(None);
        let embed_ms   = embed_start.elapsed().as_millis() as i64;

        // 4. RAG retrieval (fast, ~5-10 ms)
        let rag_examples = match &embedding {
            Some(emb) => {
                let hits = retrieve_similar(&pool, &user_id, emb, 5, 0.65);
                info!("[rag] {} example(s) retrieved for transcript", hits.len());
                for (i, ex) in hits.iter().enumerate() {
                    info!("[rag] example {}: ai={:?}  kept={:?}", i + 1, ex.ai_output, ex.user_kept);
                }
                hits
            }
            None => {
                info!("[rag] skipped — embedding unavailable");
                vec![]
            }
        };
        let examples_used = rag_examples.len();

        // 4b. Word corrections loaded from LexiconCache before stream started
        if !word_corrections.is_empty() {
            info!("[voice] {} word correction(s) loaded", word_corrections.len());
        }

        // 5. Build full system prompt — RAG examples + soft polish-layer corrections
        //    + personal vocabulary (preserve-verbatim instruction).
        let system_prompt = build_system_prompt_with_vocab(
            &prefs, &rag_examples, &word_corrections, &vocab_terms,
        );

        // 6. Stream LLM tokens — dispatch to openai_codex / gemini_direct / gateway
        let llm_provider  = prefs.llm_provider.clone();
        let (token_tx, mut token_rx) = mpsc::channel::<String>(64);
        let sys_p       = system_prompt.clone();
        let usr_m       = user_message.clone();
        let client_c    = http_client.clone();

        // Resolve model string and gather any provider-specific credentials
        let (model_for_llm, openai_token_opt) = if llm_provider == "openai_codex" {
            let tok = openai_oauth::get_token(&pool, &user_id);
            // Always use mini — smart model removed
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

        info!("[voice] LLM provider={llm_provider:?} model={model_for_llm:?}");

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

        let total_ms     = total_start.elapsed().as_millis() as i64;
        let recording_id = Uuid::new_v4().to_string();
        let word_count   = llm_result.polished.split_whitespace().count() as i64;

        // 7. Persist recording (fire-and-forget)
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
            let aid2    = audio_id.clone();
            tokio::spawn(async move {
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
                    audio_id:      Some(&aid2),
                });
            });
        }

        debug!("[voice] done — {total_ms}ms total, {word_count} words, {examples_used} RAG examples");

        // 8. Final SSE event
        yield Ok(Event::default().event("done").data(
            json!({
                "recording_id": recording_id,
                "polished":     llm_result.polished,
                "model_used":   model,
                "confidence":   stt_confidence,
                "latency_ms": {
                    "transcribe": transcribe_ms,
                    "embed":      embed_ms,
                    "retrieve":   0,
                    "polish":     llm_result.polish_ms,
                    "total":      total_ms,
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
                if ic == ']' { found_close = true; break; }
                inner.push(ic);
            }
            if found_close {
                if let Some(qpos) = inner.rfind('?') {
                    let after_q = &inner[qpos + 1..];
                    if after_q.ends_with('%') && after_q[..after_q.len() - 1].parse::<f64>().is_ok() {
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

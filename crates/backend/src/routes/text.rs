//! POST /v1/text/polish
//!
//! Same pipeline as /v1/voice/polish but skips Deepgram.
//! Body: { "text": "...", "target_app"?: "..." }
//! Response: SSE stream identical to voice/polish.

use axum::{
    extract::State,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::convert::Infallible;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
    embedder::gemini,
    llm::{
        gateway, gemini_direct, groq, openai_codex,
        prompt::{build_system_prompt_with_vocab, build_tray_system_prompt, build_user_message},
    },
    store::{
        corrections,
        history::{insert_recording, InsertRecording},
        openai_oauth,
        vectors::retrieve_similar,
        vocabulary,
    },
    AppState,
};

#[derive(Deserialize)]
pub struct TextPolishBody {
    pub text:          String,
    pub target_app:    Option<String>,
    /// When set (by tray "Polish my message"), overrides the user's stored tone_preset
    /// and forces English output — the preset label already encodes the output language.
    pub tone_override: Option<String>,
}

pub async fn polish(
    State(state): State<AppState>,
    Json(body): Json<TextPolishBody>,
) -> impl IntoResponse {
    if body.text.trim().is_empty() {
        return axum::http::StatusCode::BAD_REQUEST.into_response();
    }

    let user_id       = state.default_user_id.as_str().to_string();
    let pool          = state.pool.clone();
    let transcript    = body.text.clone();
    let target_app    = body.target_app.clone();
    let tone_override = body.tone_override.clone();

    // Load prefs + lexicon from cache and grab shared HTTP client before stream.
    let prefs_opt = crate::get_prefs_cached(&state.prefs_cache, &pool, &user_id).await;
    let (word_corrections_cached, _) =
        crate::get_lexicon_cached(&state.lexicon_cache, &pool, &user_id).await;
    let http_client = state.http_client.clone();

    let stream = async_stream::stream! {
        let total_start = Instant::now();

        let prefs = match prefs_opt {
            Some(p) => p,
            None => {
                yield Ok::<Event, Infallible>(Event::default().event("error")
                    .data(json!({"message": "preferences not found"}).to_string()));
                return;
            }
        };

        info!("[text] polishing {} chars with model={}", transcript.len(), prefs.selected_model);

        yield Ok(Event::default().event("status")
            .data(json!({"phase": "polishing", "transcript": transcript}).to_string()));

        // 1. Embed transcript + retrieve RAG examples
        let gemini_key = prefs.gemini_api_key.clone()
            .or_else(|| std::env::var("GEMINI_API_KEY").ok())
            .unwrap_or_default();
        // http_client is the shared client from AppState (loaded before stream)
        let e_start = Instant::now();
        let embedding   = gemini::embed(&http_client, &pool, &transcript, &gemini_key).await;
        let embed_ms    = e_start.elapsed().as_millis() as i64;

        let rag_examples = match &embedding {
            Some(emb) => {
                let hits = retrieve_similar(&pool, &user_id, emb, 5, 0.65);
                debug!("[text] RAG: {} examples retrieved", hits.len());
                hits
            }
            None => {
                debug!("[text] RAG skipped (no embedding)");
                vec![]
            }
        };
        let examples_used = rag_examples.len();

        // 2. Word corrections from LexiconCache (loaded before stream started)
        let word_corrections = if tone_override.is_none() {
            word_corrections_cached
        } else {
            vec![]
        };
        if !word_corrections.is_empty() {
            info!("[text] {} word correction(s) loaded", word_corrections.len());
        }

        // tone_override → use tray-specific English-locked prompt (no RAG, no persona)
        // Otherwise → use full RACC prompt with user prefs + RAG examples + corrections
        let vocab_terms = if tone_override.is_none() {
            vocabulary::top_term_strings(&pool, &user_id, 100)
        } else {
            vec![]
        };
        let system_prompt = if let Some(ref tone) = tone_override {
            build_tray_system_prompt(tone)
        } else {
            build_system_prompt_with_vocab(&prefs, &rag_examples, &word_corrections, &vocab_terms)
        };
        let user_message  = build_user_message(&transcript, &prefs.output_language);

        let (token_tx, mut token_rx) = mpsc::channel::<String>(64);
        let gateway_key = prefs.gateway_api_key.clone()
            .or_else(|| std::env::var("GATEWAY_API_KEY").ok())
            .or_else(|| { let k = voice_polish_core::api_key(); if k.is_empty() { None } else { Some(k.to_string()) } })
            .unwrap_or_default();
        let gemini_key_text = prefs.gemini_api_key.clone()
            .or_else(|| std::env::var("GEMINI_API_KEY").ok())
            .unwrap_or_default();
        let groq_key_text = prefs.groq_api_key.clone()
            .or_else(|| std::env::var("GROQ_API_KEY").ok())
            .unwrap_or_default();

        // Resolve model + provider
        let llm_provider = prefs.llm_provider.clone();
        let model = voice_polish_core::resolve_model(&prefs.selected_model).to_string();
        let sys_p       = system_prompt.clone();
        let usr_m       = user_message.clone();
        let client_c    = http_client.clone();

        let (model_for_llm, openai_token_opt) = if llm_provider == "openai_codex" {
            let tok = openai_oauth::get_token(&pool, &user_id);
            let m = if prefs.selected_model == "mini" || prefs.selected_model == "fast" {
                openai_codex::MODEL_MINI.to_string()
            } else {
                openai_codex::MODEL_SMART.to_string()
            };
            (m, tok.map(|t| t.access_token))
        } else if llm_provider == "gemini_direct" {
            (gemini_direct::GEMINI_DIRECT_MODEL.to_string(), None)
        } else if llm_provider == "groq" {
            (groq::GROQ_MODEL_DEFAULT.to_string(), None)
        } else {
            (model.clone(), None)
        };

        info!("[text] LLM provider={llm_provider:?} model={model_for_llm:?}");

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
                    &client_c, &gemini_key_text, &model_for_llm, &sys_p, &usr_m, token_tx,
                ).await
            } else if llm_provider == "groq" {
                groq::stream_polish(
                    &client_c, &groq_key_text, &model_for_llm, &sys_p, &usr_m, token_tx,
                ).await
            } else {
                gateway::stream_polish(&client_c, &gateway_key, &model_for_llm, &sys_p, &usr_m, token_tx).await
            }
        });

        while let Some(token) = token_rx.recv().await {
            yield Ok(Event::default().event("token")
                .data(json!({"token": token}).to_string()));
        }

        let llm_result = match llm_task.await {
            Ok(Ok(r))  => r,
            Ok(Err(e)) => {
                warn!("[text] LLM error: {e}");
                yield Ok(Event::default().event("error")
                    .data(json!({"message": e}).to_string()));
                return;
            }
            Err(_) => {
                yield Ok(Event::default().event("error")
                    .data(json!({"message": "internal error"}).to_string()));
                return;
            }
        };

        let total_ms     = total_start.elapsed().as_millis() as i64;
        let recording_id = Uuid::new_v4().to_string();
        let word_count   = llm_result.polished.split_whitespace().count() as i64;

        // Persist recording
        {
            let pool2  = pool.clone();
            let id2    = recording_id.clone();
            let uid2   = user_id.clone();
            let t2     = transcript.clone();
            let p2     = llm_result.polished.clone();
            let ta2    = target_app.clone();
            let model2 = model.clone(); // resolved string e.g. "gpt-5.4", not mode key "smart"
            let e_ms   = embed_ms;
            let p_ms   = llm_result.polish_ms as i64;
            tokio::spawn(async move {
                insert_recording(&pool2, InsertRecording {
                    id: &id2, user_id: &uid2,
                    transcript: &t2, polished: &p2,
                    word_count, recording_seconds: (total_ms as f64 / 1000.0),
                    model_used: &model2,
                    confidence:    None,
                    transcribe_ms: None,
                    embed_ms:      Some(e_ms),
                    polish_ms:     Some(p_ms),
                    target_app:    ta2.as_deref(),
                    source:        "text",
                    audio_id:      None,
                });
            });
        }

        debug!("[text] done — {total_ms}ms total, {word_count} words, {examples_used} RAG examples");

        yield Ok(Event::default().event("done").data(
            json!({
                "recording_id": recording_id,
                "polished":     llm_result.polished,
                "model_used":   model,  // resolved string e.g. "gpt-5.4"
                "confidence":   null,
                "latency_ms": {
                    "transcribe": 0,
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

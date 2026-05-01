//! Deepgram batch transcription client.
//! POST /v1/listen — returns the top transcript string.
//!
//! API docs: https://developers.deepgram.com/reference/listen-file

use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, warn};

const DEEPGRAM_URL: &str = "https://api.deepgram.com/v1/listen";

#[derive(Deserialize)]
struct DGResponse {
    results: DGResults,
}
#[derive(Deserialize)]
struct DGResults {
    channels: Vec<DGChannel>,
}
#[derive(Deserialize)]
struct DGChannel {
    alternatives: Vec<DGAlternative>,
}
#[derive(Deserialize)]
struct DGAlternative {
    transcript: String,
    confidence: f64,
}

pub struct TranscriptResult {
    pub transcript: String,
    pub confidence: f64,
}

/// Send WAV audio bytes to Deepgram and return the top transcript.
/// `language` defaults to `"hi"` (Hindi/Hinglish) if empty.
pub async fn transcribe(
    client:   &Client,
    api_key:  &str,
    wav_data: Vec<u8>,
    language: &str,
) -> Result<TranscriptResult, String> {
    let lang = if language.is_empty() || language == "auto" { "hi" } else { language };

    let url = format!(
        "{DEEPGRAM_URL}?model=nova-3&language={lang}&smart_format=true&punctuate=true"
    );

    debug!("[stt] sending {} bytes to Deepgram (lang={lang})", wav_data.len());

    let resp = client
        .post(&url)
        .header("Authorization", format!("Token {api_key}"))
        .header("Content-Type", "audio/wav")
        .body(wav_data)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| format!("Deepgram request failed: {e}"))?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        let preview = &body[..body.len().min(300)];
        return Err(format!("Deepgram error {status}: {preview}"));
    }

    let dg: DGResponse = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse Deepgram response: {e}"))?;

    let alt = dg
        .results
        .channels
        .into_iter()
        .next()
        .and_then(|c| c.alternatives.into_iter().next())
        .ok_or_else(|| "Deepgram returned no transcript".to_string())?;

    if alt.transcript.is_empty() {
        warn!("[stt] empty transcript from Deepgram");
        return Err("empty transcript — nothing spoken?".into());
    }

    debug!("[stt] transcript ({:.2}): {}", alt.confidence, alt.transcript);
    Ok(TranscriptResult {
        transcript: alt.transcript,
        confidence: alt.confidence,
    })
}

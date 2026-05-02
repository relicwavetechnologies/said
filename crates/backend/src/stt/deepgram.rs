//! Deepgram batch transcription client.
//! POST /v1/listen — returns the top transcript string plus per-word confidence.
//!
//! API docs: https://developers.deepgram.com/reference/listen-file

use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, info, warn};

const DEEPGRAM_URL: &str = "https://api.deepgram.com/v1/listen";

/// Confidence threshold below which a word is flagged as uncertain.
/// Deepgram typically gives >0.9 for clear words, 0.4–0.8 for ambiguous ones.
pub const LOW_CONFIDENCE_THRESHOLD: f64 = 0.85;

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
    #[serde(default)]
    words: Vec<DGWord>,
}
#[derive(Deserialize)]
struct DGWord {
    word: String,
    confidence: f64,
    #[serde(default)]
    punctuated_word: Option<String>,
}

pub struct TranscriptResult {
    /// Plain transcript text (for storage, display, embedding).
    pub transcript: String,
    /// Enriched transcript with low-confidence words marked as [word?XX%].
    /// Sent to the LLM so it knows which words to scrutinize.
    pub enriched_transcript: String,
    /// Overall utterance-level confidence (0.0–1.0).
    pub confidence: f64,
    /// Number of words flagged as low-confidence.
    pub uncertain_count: usize,
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

    // Build enriched transcript with confidence markers
    let (enriched, uncertain_count) = if alt.words.is_empty() {
        // No word-level data — fall back to plain transcript
        (alt.transcript.clone(), 0)
    } else {
        enrich_words(&alt.words)
    };

    debug!("[stt] transcript ({:.2}): {}", alt.confidence, alt.transcript);
    if uncertain_count > 0 {
        info!(
            "[stt] {} uncertain word(s) flagged (threshold < {:.0}%)",
            uncertain_count,
            LOW_CONFIDENCE_THRESHOLD * 100.0
        );
        info!("[stt] enriched: {enriched}");
    }

    Ok(TranscriptResult {
        transcript: alt.transcript,
        enriched_transcript: enriched,
        confidence: alt.confidence,
        uncertain_count,
    })
}

/// Build an enriched transcript from word-level data.
/// Words with confidence < threshold are marked as `[word?XX%]`.
fn enrich_words(words: &[DGWord]) -> (String, usize) {
    let mut parts = Vec::with_capacity(words.len());
    let mut uncertain = 0usize;

    for w in words {
        let display = w.punctuated_word.as_deref().unwrap_or(&w.word);

        if w.confidence < LOW_CONFIDENCE_THRESHOLD {
            parts.push(format!("[{}?{:.0}%]", display, w.confidence * 100.0));
            uncertain += 1;
        } else {
            parts.push(display.to_string());
        }
    }

    (parts.join(" "), uncertain)
}

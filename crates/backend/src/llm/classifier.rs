//! Groq-based edit classifier.
//!
//! Given (transcript, ai_output, user_kept), determines whether the user's edit
//! is a meaningful **AI correction** that should enter the learning corpus.
//!
//! Uses a fast Groq model (llama-3.1-8b-instant, ~100-200ms) with structured
//! JSON output.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{info, warn};

const GROQ_ENDPOINT: &str = "https://api.groq.com/openai/v1/chat/completions";
const CLASSIFIER_MODEL: &str = "llama-3.1-8b-instant";

/// A single word/phrase-level correction identified by the classifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Correction {
    /// What the user originally spoke
    pub spoke: String,
    /// What the AI changed it to
    pub ai_said: String,
    /// What the user corrected it to
    pub user_wants: String,
}

/// Result from the edit classifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyResult {
    /// Whether this edit should enter the learning corpus.
    pub should_learn: bool,
    /// Human-readable explanation of the decision.
    pub reason: String,
    /// Specific word/phrase corrections found (empty if should_learn=false).
    pub corrections: Vec<Correction>,
}

const SYSTEM_PROMPT: &str = r#"You are an edit classifier for a voice-to-text polishing app. The app works like this:
1. User SPEAKS a message (transcript)
2. AI POLISHES it into clean text (ai_output)
3. User EDITS the polished text if the AI got something wrong (user_kept)

Your job: determine if the user's edit is correcting an AI mistake that we should LEARN from.

## LEARN (should_learn = true)
The AI wrongly changed/translated a word the user intentionally spoke, and the user corrected it.
Examples:
- User said "please", AI changed to "kindly", user changed back to "please" → LEARN (AI shouldn't have changed "please")
- User said "thoda", AI translated to "slightly", user changed to "a bit" → LEARN (user prefers "a bit" for "thoda")
- User said "kal", AI wrote "yesterday", user changed to "tomorrow" → LEARN (AI mistranslated "kal")

## DON'T LEARN (should_learn = false)
- User ADDED words that weren't in the transcript (adding context, not correcting AI)
- User deleted everything and retyped (full replacement, not a correction)
- User changed a word they themselves spoke with high confidence (rephrasing, not correcting AI)
- The edit is purely whitespace, punctuation, or capitalization
- The user replaced most of the sentence (>50% of words changed = rewrite, not correction)

## Rules
1. The transcript is ground truth of what the user intended to say
2. If a word exists in the transcript AND in the ai_output unchanged, and the user changes it → NOT learning (user is changing their own spoken word)
3. If the AI changed/removed/mistranslated a word from the transcript, and the user corrects it → LEARNING
4. If user_kept adds entirely new words/phrases not traceable to the transcript → NOT learning
5. If more than 50% of the ai_output words were changed → NOT learning (it's a rewrite)

Respond with ONLY valid JSON, no markdown, no code fences:
{"should_learn": true/false, "reason": "one sentence explanation", "corrections": [{"spoke": "what user said", "ai_said": "what AI changed it to", "user_wants": "what user corrected to"}]}"#;

/// Classify an edit using Groq's fast LLM.
///
/// Returns `None` if the API call fails (caller should skip learning gracefully).
pub async fn classify_edit(
    client:       &Client,
    groq_api_key: &str,
    transcript:   &str,
    ai_output:    &str,
    user_kept:    &str,
) -> Option<ClassifyResult> {
    if groq_api_key.is_empty() {
        warn!("[classifier] no Groq API key — skipping classification");
        return None;
    }

    let user_message = format!(
        "<transcript>\n{transcript}\n</transcript>\n\n\
         <ai_output>\n{ai_output}\n</ai_output>\n\n\
         <user_kept>\n{user_kept}\n</user_kept>"
    );

    let body = json!({
        "model":       CLASSIFIER_MODEL,
        "temperature": 0.0,
        "max_tokens":  500,
        "messages": [
            { "role": "system", "content": SYSTEM_PROMPT },
            { "role": "user",   "content": user_message  },
        ]
    });

    let start = std::time::Instant::now();
    info!("[classifier] POST {GROQ_ENDPOINT} model={CLASSIFIER_MODEL}");

    let resp = match client
        .post(GROQ_ENDPOINT)
        .header("Authorization", format!("Bearer {groq_api_key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            warn!("[classifier] request failed: {e}");
            return None;
        }
    };

    let status = resp.status();
    if !status.is_success() {
        let body_text = resp.text().await.unwrap_or_default();
        warn!("[classifier] HTTP {status}: {}", &body_text[..body_text.len().min(300)]);
        return None;
    }

    let resp_json: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            warn!("[classifier] failed to parse response: {e}");
            return None;
        }
    };

    let ms = start.elapsed().as_millis();

    // Extract the assistant's message content
    let content = resp_json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("");

    if content.is_empty() {
        warn!("[classifier] empty response content");
        return None;
    }

    // Strip markdown code fences if the model wraps the JSON
    let clean = content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    match serde_json::from_str::<ClassifyResult>(clean) {
        Ok(result) => {
            info!(
                "[classifier] {ms}ms — should_learn={}, reason={:?}, corrections={}",
                result.should_learn,
                result.reason,
                result.corrections.len()
            );
            Some(result)
        }
        Err(e) => {
            warn!("[classifier] failed to parse LLM JSON: {e} — raw: {clean:?}");
            // Fallback: try to extract should_learn from partial JSON
            if clean.contains("\"should_learn\": true") || clean.contains("\"should_learn\":true") {
                Some(ClassifyResult {
                    should_learn: true,
                    reason: "parsed from partial JSON".into(),
                    corrections: vec![],
                })
            } else {
                Some(ClassifyResult {
                    should_learn: false,
                    reason: format!("classifier parse error: {e}"),
                    corrections: vec![],
                })
            }
        }
    }
}

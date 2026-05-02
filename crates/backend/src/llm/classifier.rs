//! Four-way edit classifier (Groq llama-3.1-8b-instant, ~150 ms).
//!
//! Given (transcript, ai_output, user_kept), assign the edit to exactly one of:
//!
//!   • `STT_ERROR`     — STT mis-heard a word the user actually said. The user-
//!                       final form is correct; transcript and polish are wrong.
//!                       Promote to STT-layer vocabulary / phonetic replacement.
//!
//!   • `POLISH_ERROR`  — STT got the word right; polish rewrote it wrongly. The
//!                       user reverted to the transcript word. Promote to LLM
//!                       polish corrections.
//!
//!   • `USER_REPHRASE` — User changed a word that was correctly transcribed and
//!                       correctly polished — they just prefer different wording.
//!                       Do not learn (or learn weakly into style RAG only).
//!
//!   • `USER_REWRITE`  — Large-scale rewrite (>50 % of words changed) or user
//!                       added new context. Do not learn.
//!
//! KEY INVARIANT (the bug the old binary classifier had):
//!     The transcript is **NOT** ground truth. STT errors are exactly the case
//!     where transcript and polish agree on the wrong word.  When the user-final
//!     token looks jargon-like (digits, mixed case, rare term) and is missing
//!     from the transcript, prefer `STT_ERROR` over `USER_REPHRASE`.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{info, warn};

const GROQ_ENDPOINT:    &str = "https://api.groq.com/openai/v1/chat/completions";
const CLASSIFIER_MODEL: &str = "llama-3.1-8b-instant";

/// One concrete word/phrase pair the classifier identified.  Always populated
/// for STT_ERROR and POLISH_ERROR; usually empty for REPHRASE/REWRITE.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    /// Best guess at what the user actually said (may equal `correct_form`).
    pub spoke:           String,
    /// What the transcript captured (i.e. STT output for this token).
    pub transcript_form: String,
    /// What the polish produced.
    pub polish_form:     String,
    /// What the user kept after editing.
    pub correct_form:    String,
}

/// The four mutually-exclusive classes of edit.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EditClass {
    SttError,
    PolishError,
    UserRephrase,
    UserRewrite,
}

impl EditClass {
    pub fn as_str(self) -> &'static str {
        match self {
            EditClass::SttError     => "STT_ERROR",
            EditClass::PolishError  => "POLISH_ERROR",
            EditClass::UserRephrase => "USER_REPHRASE",
            EditClass::UserRewrite  => "USER_REWRITE",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_uppercase().as_str() {
            "STT_ERROR"     | "STTERROR"     => Some(Self::SttError),
            "POLISH_ERROR"  | "POLISHERROR"  => Some(Self::PolishError),
            "USER_REPHRASE" | "USERREPHRASE" => Some(Self::UserRephrase),
            "USER_REWRITE"  | "USERREWRITE"  => Some(Self::UserRewrite),
            _ => None,
        }
    }

    /// Should this class produce any learning artifacts?
    pub fn is_learnable(self) -> bool {
        matches!(self, Self::SttError | Self::PolishError)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyResult {
    pub class:      EditClass,
    pub reason:     String,
    pub candidates: Vec<Candidate>,
    /// Self-reported confidence ∈ [0, 1]; the LLM is asked to be honest.
    /// Treated as a soft signal — promotion logic also uses heuristic gates.
    #[serde(default)]
    pub confidence: f64,
}

const SYSTEM_PROMPT: &str = r#"You classify a user's post-dictation edit into exactly one of four classes.

PIPELINE: user SPOKE audio → STT produced TRANSCRIPT → LLM produced POLISH → user EDITED to USER_KEPT.

Classes (pick exactly one):

1. STT_ERROR — The STT engine misheard a word the user actually said. Both transcript and polish contain the wrong word; user_kept restores what was actually spoken. Common when the spoken word is a name, brand, code identifier, technical term, acronym, or non-English word.
   Detection signal: a token in user_kept is absent from the transcript AND the polish, AND it looks jargon-like (mixed case, digits, rare/specialized term).
   Examples (LEARN):
   - transcript: "I use written for automation"
     polish:     "I use written for automation"
     user_kept:  "I use n8n for automation"
     → STT_ERROR. User said "n8n" (a tool name); STT misheard as "written".
   - transcript: "we passed na is great"
     polish:     "We passed na is great"
     user_kept:  "Vipassana is great"
     → STT_ERROR. User said "Vipassana"; STT split it into "we passed na".

2. POLISH_ERROR — STT captured the word correctly, but the polish step rewrote it. user_kept restores the transcript word.
   Detection signal: the user_kept token IS present in the transcript but NOT in the polish.
   Examples (LEARN):
   - transcript: "please send the report"
     polish:     "kindly send the report"
     user_kept:  "please send the report"
     → POLISH_ERROR. AI changed "please" to "kindly"; user reverted.

3. USER_REPHRASE — STT and polish are both reasonable. The user is just expressing a personal style preference, swapping a word they said for a different word.
   Detection signal: the changed word is present in the transcript AND in the polish AND the new word is common English / not jargon.
   Examples (DON'T LEARN strongly):
   - transcript: "the meeting was good"
     polish:     "The meeting was good"
     user_kept:  "The meeting was great"
     → USER_REPHRASE. User is changing their own word for stylistic reasons.

4. USER_REWRITE — More than half the words changed, or the user added substantial new content not present in the transcript.
   Detection signal: many tokens in user_kept are not in the polish; the change is structural.
   Example: user_kept has new sentences or rewrites the whole paragraph → USER_REWRITE.

CRITICAL RULES (DO NOT VIOLATE):
A. Treat the transcript as POSSIBLY WRONG. STT errors are exactly the case where transcript and polish both contain the same wrong word. Never assume "word in transcript = user said it".
B. If a user_kept token is absent from BOTH transcript and polish AND it looks jargon-like (digits like "n8n", "k8s"; mixed case like "GraphQL"; or proper-noun-like) → STT_ERROR, NOT USER_REPHRASE.
C. If the change is a single short uppercase/digit-bearing token replacing a common word, that is almost certainly STT_ERROR.
D. Whitespace/punctuation/capitalization-only diffs → USER_REPHRASE (effectively no-op, low confidence).

OUTPUT — strict JSON only, no markdown, no commentary:
{
  "class":      "STT_ERROR" | "POLISH_ERROR" | "USER_REPHRASE" | "USER_REWRITE",
  "reason":     "one sentence explanation referencing the actual tokens",
  "candidates": [{"spoke": "...", "transcript_form": "...", "polish_form": "...", "correct_form": "..."}],
  "confidence": 0.0
}

For STT_ERROR: candidates[i].spoke == correct_form (best guess at what was actually spoken). transcript_form and polish_form should be the wrong tokens that appeared.
For POLISH_ERROR: transcript_form == correct_form (transcript was right). polish_form is the wrong word the LLM produced.
For USER_REPHRASE / USER_REWRITE: candidates may be empty."#;

/// Classify an edit using Groq.  Returns `None` if the API call fails — the
/// caller must skip learning silently (we never crash a user's flow over a
/// classifier hiccup).
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
         <polish>\n{ai_output}\n</polish>\n\n\
         <user_kept>\n{user_kept}\n</user_kept>"
    );

    let body = json!({
        "model":           CLASSIFIER_MODEL,
        "temperature":     0.0,
        "max_tokens":      600,
        "response_format": { "type": "json_object" },
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
        Err(e) => { warn!("[classifier] request failed: {e}"); return None; }
    };

    let status = resp.status();
    if !status.is_success() {
        let body_text = resp.text().await.unwrap_or_default();
        warn!("[classifier] HTTP {status}: {}", &body_text[..body_text.len().min(300)]);
        return None;
    }

    let resp_json: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => { warn!("[classifier] parse error: {e}"); return None; }
    };

    let ms = start.elapsed().as_millis();
    let content = resp_json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    if content.is_empty() {
        warn!("[classifier] empty response content");
        return None;
    }

    parse_classifier_json(content)
        .map(|r| {
            info!(
                "[classifier] {ms}ms — class={} candidates={} conf={:.2} reason={:?}",
                r.class.as_str(), r.candidates.len(), r.confidence, r.reason,
            );
            r
        })
        .or_else(|| {
            warn!("[classifier] failed to parse JSON: {content:?}");
            None
        })
}

/// Parse the classifier's JSON output into a `ClassifyResult`.  Tolerant of
/// minor schema drift (string class field; missing confidence; missing
/// candidates).  Returns `None` only if the class field is unrecognizable.
pub fn parse_classifier_json(s: &str) -> Option<ClassifyResult> {
    #[derive(Deserialize)]
    struct Raw {
        class:      String,
        #[serde(default)]
        reason:     String,
        #[serde(default)]
        candidates: Vec<Candidate>,
        #[serde(default)]
        confidence: f64,
    }

    let raw: Raw = serde_json::from_str(s).ok()?;
    let class = EditClass::parse(&raw.class)?;
    Some(ClassifyResult {
        class,
        reason:     raw.reason,
        candidates: raw.candidates,
        confidence: raw.confidence.clamp(0.0, 1.0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_stt_error() {
        let json = r#"{
            "class": "STT_ERROR",
            "reason": "n8n is jargon, absent from transcript and polish",
            "candidates": [{
                "spoke": "n8n",
                "transcript_form": "written",
                "polish_form": "written",
                "correct_form": "n8n"
            }],
            "confidence": 0.9
        }"#;
        let r = parse_classifier_json(json).unwrap();
        assert_eq!(r.class, EditClass::SttError);
        assert_eq!(r.candidates.len(), 1);
        assert_eq!(r.candidates[0].correct_form, "n8n");
        assert!(r.class.is_learnable());
    }

    #[test]
    fn parse_polish_error() {
        let json = r#"{
            "class":"POLISH_ERROR",
            "reason":"AI replaced 'please' with 'kindly'",
            "candidates":[{"spoke":"please","transcript_form":"please","polish_form":"kindly","correct_form":"please"}],
            "confidence":0.85
        }"#;
        let r = parse_classifier_json(json).unwrap();
        assert_eq!(r.class, EditClass::PolishError);
        assert!(r.class.is_learnable());
    }

    #[test]
    fn parse_rephrase_no_candidates() {
        let json = r#"{"class":"USER_REPHRASE","reason":"style preference","candidates":[],"confidence":0.5}"#;
        let r = parse_classifier_json(json).unwrap();
        assert_eq!(r.class, EditClass::UserRephrase);
        assert!(!r.class.is_learnable());
    }

    #[test]
    fn parse_rewrite() {
        let json = r#"{"class":"USER_REWRITE","reason":"most words changed","candidates":[]}"#;
        let r = parse_classifier_json(json).unwrap();
        assert_eq!(r.class, EditClass::UserRewrite);
        assert!(!r.class.is_learnable());
    }

    #[test]
    fn parse_rejects_unknown_class() {
        let json = r#"{"class":"MAYBE","reason":""}"#;
        assert!(parse_classifier_json(json).is_none());
    }

    #[test]
    fn parse_accepts_lowercase_class() {
        let json = r#"{"class":"stt_error","reason":"","candidates":[],"confidence":0.7}"#;
        let r = parse_classifier_json(json).unwrap();
        assert_eq!(r.class, EditClass::SttError);
    }

    #[test]
    fn confidence_is_clamped() {
        let json = r#"{"class":"STT_ERROR","reason":"","confidence":2.5,"candidates":[]}"#;
        let r = parse_classifier_json(json).unwrap();
        assert!(r.confidence <= 1.0 && r.confidence >= 0.0);
    }
}

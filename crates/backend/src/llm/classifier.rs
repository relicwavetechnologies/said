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

use super::edit_diff::Hunk;

const GROQ_ENDPOINT:    &str = "https://api.groq.com/openai/v1/chat/completions";
const CLASSIFIER_MODEL: &str = "llama-3.1-8b-instant";

/// One labelled hunk — pairing the deterministic diff hunk with the LLM's
/// class assignment.  Replaces the old `Candidate` struct: candidates are no
/// longer LLM-generated, they're diff-derived.  The LLM only labels them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelledHunk {
    pub hunk:       Hunk,
    pub class:      EditClass,
    pub confidence: f64,
}

/// Backwards-compatible alias for the route layer — a labelled hunk is the
/// candidate now.  The route consumes `transcript_form` / `polish_form` /
/// `correct_form` getters defined below.
pub type Candidate = LabelledHunk;

impl LabelledHunk {
    /// What STT transcribed for this region (may be empty if the diff
    /// couldn't positionally align transcript to polish).
    pub fn transcript_form(&self) -> &str { &self.hunk.transcript_window }
    /// What the polish step produced.
    pub fn polish_form(&self) -> &str { &self.hunk.polish_window }
    /// What the user kept (the proposed correct form).
    pub fn correct_form(&self) -> &str { &self.hunk.kept_window }
    /// Spoken form — best guess at what was actually spoken.  For STT_ERROR
    /// this is the kept_window (user restored what they actually said).
    pub fn spoke(&self) -> &str { &self.hunk.kept_window }
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
    /// One entry per diff hunk (in input order).  Each carries the original
    /// hunk plus the LLM's class label and confidence.  The route uses the
    /// hunk's text — it never relies on the LLM having "invented" a term.
    pub candidates: Vec<LabelledHunk>,
    /// Mean confidence across labelled hunks ∈ [0, 1].  Soft signal — promotion
    /// logic also uses script + phonetic + jargon gates as defense in depth.
    #[serde(default)]
    pub confidence: f64,
}

const SYSTEM_PROMPT: &str = r#"You are a LABELER. You assign one class label to each pre-extracted hunk of a user's post-dictation edit.

PIPELINE: user SPOKE audio → STT produced TRANSCRIPT → LLM produced POLISH → user EDITED to USER_KEPT.

A hunk is a contiguous region where polish and user_kept differ. Hunks are computed deterministically from the actual text — you DID NOT and CANNOT invent them. Each hunk gives you:
  • transcript_window — the corresponding region of the original transcript (may be empty if positional alignment failed)
  • polish_window     — what the polish step produced for this region (may be empty for pure insertions)
  • kept_window       — what the user actually kept for this region (may be empty for pure deletions)

For each hunk, pick exactly ONE class:

1. STT_ERROR — The STT engine misheard a word the user actually said. Both transcript and polish contain the wrong word; user_kept restores what was actually spoken. Most common for names, brands, code identifiers (n8n, k8s), technical terms, non-English words.
   Signature: kept_window is absent from transcript AND polish AND looks jargon-like (digits, mixed case, rare term).
   Example: transcript_window="written", polish_window="written", kept_window="n8n" → STT_ERROR.

2. POLISH_ERROR — STT captured correctly, polish rewrote wrongly, user reverted to transcript form.
   Signature: kept_window equals transcript_window AND differs from polish_window.
   Example: transcript_window="please", polish_window="kindly", kept_window="please" → POLISH_ERROR.

3. USER_REPHRASE — Stylistic preference swap. STT and polish are both reasonable; user just prefers different wording.
   Signature: kept_window is a common-language synonym of polish_window AND not jargon-like.
   Example: polish_window="good", kept_window="great" → USER_REPHRASE.

4. USER_REWRITE — Pure insertion (polish_window empty), pure deletion (kept_window empty), or large structural change.
   Example: polish_window="", kept_window="[anish@gmail.com](mailto:anish@gmail.com)" → USER_REWRITE.

CRITICAL RULES (DO NOT VIOLATE):
A. Use ONLY the text inside the hunks I gave you. NEVER invent words that don't appear in transcript / polish / kept windows.
B. If kept_window contains characters in a different script than the user's output_language preference, the answer is USER_REPHRASE — do NOT propose script translation as a learnable error.
C. The transcript may itself be wrong. STT_ERROR is exactly the case where transcript and polish both contain the same wrong word.
D. If polish_window is empty (pure insertion) → USER_REWRITE. If kept_window is empty (pure deletion) → USER_REWRITE.

OUTPUT — strict JSON only, no markdown, no commentary:
{
  "labels":     [{"hunk_index": 0, "class": "STT_ERROR" | "POLISH_ERROR" | "USER_REPHRASE" | "USER_REWRITE", "confidence": 0.0}],
  "overall":    "STT_ERROR" | "POLISH_ERROR" | "USER_REPHRASE" | "USER_REWRITE",
  "reason":     "one sentence explanation referencing the actual hunk windows"
}

`overall` is the single dominant class for the whole edit (use the most-learnable class present, with priority STT_ERROR > POLISH_ERROR > USER_REPHRASE > USER_REWRITE).
`labels[i]` MUST exist for every hunk_index from 0 to N-1 in input order.
`confidence` ∈ [0,1]. Be honest — low when the hunk is ambiguous."#;

/// Label diff-derived hunks via Groq.  Hunks are computed by `edit_diff::diff`
/// before this is called — the LLM only assigns class labels, it cannot
/// invent candidates.  Returns `None` if the API call fails (caller skips
/// learning gracefully).
///
/// `hunks` MUST be non-empty.  Empty hunks should be short-circuited at the
/// pre-filter stage (no learnable change → don't waste an API call).
pub async fn classify_edit(
    client:          &Client,
    groq_api_key:    &str,
    transcript:      &str,
    ai_output:       &str,
    user_kept:       &str,
    hunks:           &[Hunk],
    output_language: &str,
) -> Option<ClassifyResult> {
    if groq_api_key.is_empty() {
        warn!("[classifier] no Groq API key — skipping classification");
        return None;
    }
    if hunks.is_empty() {
        warn!("[classifier] called with zero hunks — refusing");
        return None;
    }

    // Compact, structured hunk presentation.  The LLM sees indices it must
    // label and cannot fabricate new ones.
    let hunks_block: String = hunks
        .iter()
        .enumerate()
        .map(|(i, h)| {
            format!(
                "  {{\"hunk_index\": {i}, \
                  \"transcript_window\": {tx:?}, \
                  \"polish_window\": {pw:?}, \
                  \"kept_window\": {kw:?}}}",
                tx = h.transcript_window,
                pw = h.polish_window,
                kw = h.kept_window,
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");

    let user_message = format!(
        "<output_language>{output_language}</output_language>\n\n\
         <transcript>\n{transcript}\n</transcript>\n\n\
         <polish>\n{ai_output}\n</polish>\n\n\
         <user_kept>\n{user_kept}\n</user_kept>\n\n\
         <hunks>\n[\n{hunks_block}\n]\n</hunks>\n\n\
         Label each hunk by hunk_index in input order. \
         Return overall + labels[] + reason as strict JSON."
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

    parse_label_response(content, hunks)
        .map(|r| {
            info!(
                "[classifier] {ms}ms — overall={} labelled={}/{} mean_conf={:.2} reason={:?}",
                r.class.as_str(), r.candidates.len(), hunks.len(), r.confidence, r.reason,
            );
            r
        })
        .or_else(|| {
            warn!("[classifier] failed to parse JSON: {content:?}");
            None
        })
}

/// Parse the labeler response.  Pairs each LLM label with its corresponding
/// diff hunk by `hunk_index`.  If the LLM omits a hunk we treat its class
/// as USER_REPHRASE (safe default — no learning).
///
/// Tolerates minor schema drift: missing fields, lowercase enum values, and
/// the legacy invent-candidates schema (mapped to USER_REPHRASE since we
/// can't trust LLM-fabricated terms anyway).
pub fn parse_label_response(s: &str, hunks: &[Hunk]) -> Option<ClassifyResult> {
    #[derive(Deserialize)]
    struct Raw {
        #[serde(default)]
        overall: String,
        #[serde(default)]
        reason:  String,
        #[serde(default)]
        labels:  Vec<RawLabel>,
    }
    #[derive(Deserialize)]
    struct RawLabel {
        #[serde(default)]
        hunk_index: usize,
        #[serde(default)]
        class:      String,
        #[serde(default)]
        confidence: f64,
    }

    let raw: Raw = serde_json::from_str(s).ok()?;
    // Priority: STT_ERROR > POLISH_ERROR > USER_REPHRASE > USER_REWRITE.
    fn priority(c: EditClass) -> u8 {
        match c {
            EditClass::SttError     => 4,
            EditClass::PolishError  => 3,
            EditClass::UserRephrase => 2,
            EditClass::UserRewrite  => 1,
        }
    }
    let dominant_from_labels = || -> Option<EditClass> {
        let mut best: Option<EditClass> = None;
        for l in &raw.labels {
            let Some(c) = EditClass::parse(&l.class) else { continue };
            if best.map(priority).unwrap_or(0) < priority(c) {
                best = Some(c);
            }
        }
        best
    };
    let overall = EditClass::parse(&raw.overall).or_else(dominant_from_labels)?;

    let mut labelled: Vec<LabelledHunk> = Vec::with_capacity(hunks.len());
    for (i, hunk) in hunks.iter().enumerate() {
        let lbl = raw.labels.iter().find(|l| l.hunk_index == i);
        let class = lbl
            .and_then(|l| EditClass::parse(&l.class))
            .unwrap_or(EditClass::UserRephrase);
        let confidence = lbl.map(|l| l.confidence.clamp(0.0, 1.0)).unwrap_or(0.0);
        labelled.push(LabelledHunk { hunk: hunk.clone(), class, confidence });
    }

    let mean_conf = if labelled.is_empty() {
        0.0
    } else {
        labelled.iter().map(|l| l.confidence).sum::<f64>() / labelled.len() as f64
    };

    Some(ClassifyResult {
        class:      overall,
        reason:     raw.reason,
        candidates: labelled,
        confidence: mean_conf,
    })
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::edit_diff::Hunk;

    fn hunk(t: &str, p: &str, k: &str) -> Hunk {
        Hunk { transcript_window: t.into(), polish_window: p.into(), kept_window: k.into() }
    }

    #[test]
    fn parse_stt_error_label_is_paired_with_hunk() {
        let hunks = vec![hunk("written", "written", "n8n")];
        let json = r#"{"overall":"STT_ERROR","reason":"n8n is jargon",
                       "labels":[{"hunk_index":0,"class":"STT_ERROR","confidence":0.9}]}"#;
        let r = parse_label_response(json, &hunks).unwrap();
        assert_eq!(r.class, EditClass::SttError);
        assert_eq!(r.candidates.len(), 1);
        assert_eq!(r.candidates[0].correct_form(), "n8n");
        assert_eq!(r.candidates[0].class, EditClass::SttError);
        assert!(r.class.is_learnable());
    }

    #[test]
    fn parse_polish_error_label() {
        let hunks = vec![hunk("please", "kindly", "please")];
        let json = r#"{"overall":"POLISH_ERROR","reason":"AI swap",
                       "labels":[{"hunk_index":0,"class":"POLISH_ERROR","confidence":0.85}]}"#;
        let r = parse_label_response(json, &hunks).unwrap();
        assert_eq!(r.class, EditClass::PolishError);
        assert!(r.class.is_learnable());
    }

    #[test]
    fn parse_rephrase_no_promotion() {
        let hunks = vec![hunk("good", "good", "great")];
        let json = r#"{"overall":"USER_REPHRASE","reason":"style","labels":[
                        {"hunk_index":0,"class":"USER_REPHRASE","confidence":0.6}]}"#;
        let r = parse_label_response(json, &hunks).unwrap();
        assert_eq!(r.class, EditClass::UserRephrase);
        assert!(!r.class.is_learnable());
    }

    #[test]
    fn parse_rewrite() {
        let hunks = vec![hunk("", "", "[link](mailto:x@y)")];
        let json = r#"{"overall":"USER_REWRITE","reason":"prefix added","labels":[
                        {"hunk_index":0,"class":"USER_REWRITE","confidence":0.95}]}"#;
        let r = parse_label_response(json, &hunks).unwrap();
        assert_eq!(r.class, EditClass::UserRewrite);
        assert!(!r.class.is_learnable());
    }

    #[test]
    fn parse_missing_label_defaults_to_rephrase() {
        // LLM returned only one label but there are two hunks — the missing
        // hunk gets safe-default USER_REPHRASE so we don't promote it.
        let hunks = vec![
            hunk("written", "written", "n8n"),
            hunk("good",    "good",    "great"),
        ];
        let json = r#"{"overall":"STT_ERROR","reason":"only first labelled",
                       "labels":[{"hunk_index":0,"class":"STT_ERROR","confidence":0.9}]}"#;
        let r = parse_label_response(json, &hunks).unwrap();
        assert_eq!(r.candidates.len(), 2);
        assert_eq!(r.candidates[1].class, EditClass::UserRephrase);
    }

    #[test]
    fn parse_overall_inferred_from_labels_when_missing() {
        // No `overall` field — must derive from labels (priority: STT_ERROR > others).
        let hunks = vec![hunk("a", "a", "b"), hunk("c", "c", "d")];
        let json = r#"{"reason":"","labels":[
                        {"hunk_index":0,"class":"USER_REPHRASE","confidence":0.5},
                        {"hunk_index":1,"class":"STT_ERROR","confidence":0.9}]}"#;
        let r = parse_label_response(json, &hunks).unwrap();
        assert_eq!(r.class, EditClass::SttError);
    }

    #[test]
    fn parse_rejects_unknown_overall_with_no_labels() {
        let hunks = vec![hunk("a", "a", "b")];
        let json = r#"{"overall":"MAYBE","reason":"","labels":[]}"#;
        assert!(parse_label_response(json, &hunks).is_none());
    }

    #[test]
    fn parse_accepts_lowercase_class() {
        let hunks = vec![hunk("written", "written", "n8n")];
        let json = r#"{"overall":"stt_error","labels":[
                        {"hunk_index":0,"class":"stt_error","confidence":0.7}]}"#;
        let r = parse_label_response(json, &hunks).unwrap();
        assert_eq!(r.class, EditClass::SttError);
    }

    #[test]
    fn parse_confidence_clamped() {
        let hunks = vec![hunk("a", "a", "b")];
        let json = r#"{"overall":"STT_ERROR","labels":[
                        {"hunk_index":0,"class":"STT_ERROR","confidence":2.5}]}"#;
        let r = parse_label_response(json, &hunks).unwrap();
        assert!(r.candidates[0].confidence <= 1.0);
        assert!(r.confidence <= 1.0);
    }
}

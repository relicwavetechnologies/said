//! Stage 1 of the learning pipeline — deterministic pre-filter.
//!
//! Catches edit shapes whose class is decidable WITHOUT an LLM call:
//!
//!   • `Drop`         — polish == user_kept (no real edit)
//!   • `UserRewrite`  — large length delta or polish ⊂ user_kept + significant
//!                      added content (markdown link, signature, full rewrite)
//!   • `UserRephrase` — script of user_kept differs from output_language pref
//!                      (user typing in unrelated script — not a learnable
//!                      jargon/polish issue)
//!
//! Returns `PreFilter::Pass` to escalate to the diff+LLM stage; otherwise
//! returns the early decision and the route can answer immediately.
//!
//! This stage is what makes the pipeline cheap and predictable: the most
//! common "false positive" shapes never hit the classifier.

use super::edit_diff;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreFilter {
    /// No edit happened (polish == user_kept).
    Drop,
    /// Edit shape is a rewrite — bypass the LLM.
    EarlyClass(EarlyDecision),
    /// Edit warrants real classification — proceed to diff + LLM.
    Pass,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EarlyDecision {
    pub class:  &'static str,
    pub reason: &'static str,
}

/// Run the pre-filter.
///
/// `output_language` is the user's preference (`english | hinglish | hindi |
/// custom`).  Used for the script-mismatch check.
pub fn run(
    polish:          &str,
    user_kept:       &str,
    output_language: &str,
) -> PreFilter {
    let polish_t = polish.trim();
    let kept_t   = user_kept.trim();

    if polish_t == kept_t {
        return PreFilter::Drop;
    }

    let p_chars = polish_t.chars().count();
    let k_chars = kept_t.chars().count();

    // Empty polish but non-empty kept → user wrote everything → REWRITE
    if p_chars == 0 {
        return PreFilter::EarlyClass(EarlyDecision {
            class:  "USER_REWRITE",
            reason: "no polish text — user wrote everything from scratch",
        });
    }

    // Big proportional growth → REWRITE shape (added significant content).
    if k_chars as f64 > (p_chars as f64) * 1.4 {
        return PreFilter::EarlyClass(EarlyDecision {
            class:  "USER_REWRITE",
            reason: "user_kept length > 1.4× polish — large additive change",
        });
    }

    // Big absolute growth (markdown link, signature) → REWRITE shape.
    if k_chars > p_chars + 30 {
        return PreFilter::EarlyClass(EarlyDecision {
            class:  "USER_REWRITE",
            reason: "user_kept exceeds polish by >30 chars — likely added prefix/suffix",
        });
    }

    // Polish appears verbatim with substantial wrapping → REWRITE.
    if let Some(idx) = kept_t.find(polish_t) {
        let extra_chars = k_chars - p_chars;
        let polish_kept_verbatim = idx > 0 || idx + p_chars < kept_t.len();
        if polish_kept_verbatim && extra_chars > 10 {
            return PreFilter::EarlyClass(EarlyDecision {
                class:  "USER_REWRITE",
                reason: "polish kept verbatim with prefix/suffix added (e.g. markdown link)",
            });
        }
    }

    // Script-mismatch check: if user_kept is in a script that doesn't match
    // their output language preference, treat as REPHRASE — they're actively
    // overriding our polish into their preferred script, not correcting an
    // STT/polish error.
    if !script_consistent(kept_t, output_language) {
        return PreFilter::EarlyClass(EarlyDecision {
            class:  "USER_REPHRASE",
            reason: "user_kept script does not match output_language preference",
        });
    }

    // Token-level diff: if the diff is vacuous (only whitespace/punct
    // differences), skip — there's nothing structural to learn.
    if edit_diff::diff(polish_t, polish_t, kept_t).is_empty() {
        return PreFilter::Drop;
    }

    PreFilter::Pass
}

/// True if the user_kept script is broadly compatible with their output
/// language preference.  A few Devanagari chars in a Hinglish text are fine
/// (idioms, names) but a *majority* of Devanagari content in `hinglish` mode
/// is the user overriding the polish into their preferred script.
fn script_consistent(text: &str, output_language: &str) -> bool {
    let alphabetic_chars: Vec<char> = text.chars().filter(|c| c.is_alphabetic()).collect();
    if alphabetic_chars.is_empty() {
        return true;
    }
    let total = alphabetic_chars.len() as f64;
    let devanagari = alphabetic_chars
        .iter()
        .filter(|c| {
            let code = **c as u32;
            (0x0900..=0x097F).contains(&code) || (0xA8E0..=0xA8FF).contains(&code)
        })
        .count() as f64;
    let dev_ratio = devanagari / total;

    match output_language {
        // In Roman-output modes, more than 30% Devanagari means the user is
        // overriding the script — not a learnable correction.
        "english" | "hinglish" => dev_ratio < 0.30,
        // Hindi mode is permissive: Roman jargon is normal.
        "hindi" => true,
        // Custom / unknown → permissive.
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_texts_drop() {
        assert_eq!(run("hello", "hello", "english"), PreFilter::Drop);
    }

    #[test]
    fn whitespace_only_diff_drops() {
        assert_eq!(run("hello world", "  hello  world  ", "english"), PreFilter::Drop);
    }

    #[test]
    fn email_link_prefix_caught_as_rewrite() {
        // The exact failure case from production logs.
        let polish = "Anish at Gmail dot com ka zara batana kaun sa mail ID par bhejna hai";
        let kept   = "[anish@gmail.com](mailto:anish@gmail.com) Anish at Gmail dot com ka zara batana kaun sa mail ID par bhejna hai";
        let result = run(polish, kept, "hinglish");
        match result {
            PreFilter::EarlyClass(d) => assert_eq!(d.class, "USER_REWRITE"),
            other => panic!("expected USER_REWRITE early-class, got {other:?}"),
        }
    }

    #[test]
    fn small_in_place_correction_passes_to_classifier() {
        // n8n case must reach the classifier.
        assert_eq!(
            run("I use written for automation",
                "I use n8n for automation",
                "hinglish"),
            PreFilter::Pass,
        );
    }

    #[test]
    fn devanagari_majority_in_hinglish_mode_is_rephrase() {
        // User overrode polish into Devanagari while their pref is hinglish.
        let polish = "main kal jaunga";
        let kept   = "मैं कल जाऊंगा";
        let result = run(polish, kept, "hinglish");
        match result {
            PreFilter::EarlyClass(d) => assert_eq!(d.class, "USER_REPHRASE"),
            other => panic!("expected USER_REPHRASE, got {other:?}"),
        }
    }

    #[test]
    fn devanagari_in_hindi_mode_is_pass() {
        let polish = "मैं कल";
        let kept   = "मैं आज";
        assert_eq!(run(polish, kept, "hindi"), PreFilter::Pass);
    }

    #[test]
    fn full_rewrite_is_caught() {
        let polish = "short";
        let kept   = "completely different sentence with much more content than the original short";
        match run(polish, kept, "english") {
            PreFilter::EarlyClass(d) => assert_eq!(d.class, "USER_REWRITE"),
            other => panic!("expected USER_REWRITE, got {other:?}"),
        }
    }

    #[test]
    fn typo_fix_passes() {
        assert_eq!(run("the meeting was good", "the meeting was great", "english"), PreFilter::Pass);
    }
}

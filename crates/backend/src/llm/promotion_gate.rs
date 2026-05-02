//! Validation gates for STT_ERROR auto-promotion.
//!
//! The classifier (Groq llama-3.1-8b) sometimes hallucinates corrections that
//! don't actually exist in the user's edit — most notoriously, proposing
//! Devanagari "ground truth" when the user is dictating Hinglish in romanized
//! script.  Without these gates a single bad classification poisons the
//! vocabulary table with garbage rules.
//!
//! Three gates, each a hard bool — a candidate must pass ALL of them:
//!
//!   1. `appears_in_user_kept`   — the supposed "correct form" must actually
//!                                  appear in user_kept.  If the LLM made it
//!                                  up out of thin air, this catches it.
//!   2. `script_matches`          — the candidate's script must match the
//!                                  user's chosen output language (e.g. don't
//!                                  promote Devanagari into a Hinglish-mode
//!                                  user's vocab).
//!   3. `not_user_added_content`  — the candidate must not appear in
//!                                  user_kept *only* because the user added a
//!                                  large prefix/suffix that wasn't in polish
//!                                  (markdown links, signatures, brackets).

/// True if `term` appears as a whole-word match in `text` (case-insensitive
/// for ASCII; exact for non-ASCII like Devanagari).
pub fn appears_in_user_kept(term: &str, user_kept: &str) -> bool {
    let term = term.trim();
    if term.is_empty() {
        return false;
    }
    // Whole-word containment.  We split on Unicode whitespace + punctuation
    // boundaries and check each token.
    user_kept
        .split(|c: char| c.is_whitespace() || (c.is_ascii_punctuation() && c != '_' && c != '-'))
        .any(|tok| {
            let tok = tok.trim();
            !tok.is_empty()
                && (tok == term
                    || (tok.is_ascii() && term.is_ascii() && tok.eq_ignore_ascii_case(term)))
        })
}

/// True if `term`'s script matches `output_language`.
///   `english` / `hinglish`  → must be ASCII (Roman script)
///   `hindi`                 → may contain Devanagari
///   anything else (custom)  → permissive (allow either)
///
/// "ASCII script" here means: the alphabetic characters in `term` are all
/// ASCII letters.  Digits, punctuation, and underscores are neutral.
pub fn script_matches(term: &str, output_language: &str) -> bool {
    let alphabetic_chars: Vec<char> = term.chars().filter(|c| c.is_alphabetic()).collect();
    if alphabetic_chars.is_empty() {
        return true;
    }
    let all_ascii = alphabetic_chars.iter().all(|c| c.is_ascii());
    let any_devanagari = alphabetic_chars.iter().any(|c| {
        let code = *c as u32;
        // Devanagari block: U+0900..=U+097F + Devanagari Extended U+A8E0..=U+A8FF
        (0x0900..=0x097F).contains(&code) || (0xA8E0..=0xA8FF).contains(&code)
    });

    match output_language {
        "english" | "hinglish" => all_ascii,
        "hindi"                => any_devanagari || all_ascii, // Hindi mode tolerates either
        _                      => true,                        // custom / unknown — allow
    }
}

/// True if `kept` looks like an insertion-without-deletion mishap rather
/// than a clean correction — i.e. `kept` literally contains `polish` as a
/// PREFIX or SUFFIX, with substantial extra characters attached.
///
/// The canonical case: cursor was at column 0 in the target field, user
/// typed "EMIAC" intending to *replace* "MAAR", but the polish wasn't
/// selected — they ended up with "EMIACMAAR" (concatenation).  Here "MAAR"
/// is a SUFFIX of "EMIACMAAR" with a 5-char prefix added.  Refuse to learn.
///
/// We require the addition to be ≥ 3 chars to distinguish concatenation from
/// typo extension ("Anis" → "anish" is a 1-char tail; that's a real STT fix,
/// not a concatenation).  We also require polish to sit at the boundary
/// (start or end of kept) — interior containment is rare in practice and
/// usually a coincidence rather than a missed-deletion.
///
/// Returns false when:
///   • `polish` is empty
///   • `polish` is < 3 chars (containment is too noisy at that length)
///   • `kept == polish` (no diff)
///   • polish appears in kept but not as prefix or suffix
///   • the extra-char count is < 3 (typo / minor extension, not concatenation)
pub fn is_concatenation_pattern(polish: &str, kept: &str) -> bool {
    let polish = polish.trim();
    let kept   = kept.trim();
    if polish.is_empty() || kept.is_empty() || polish == kept {
        return false;
    }
    let polish_chars = polish.chars().count();
    let kept_chars   = kept.chars().count();
    if polish_chars < 3 || kept_chars <= polish_chars {
        return false;
    }
    let extra = kept_chars - polish_chars;
    if extra < 3 {
        // Likely typo extension (e.g. "Anis" → "anish") — let the normal
        // gates handle it.
        return false;
    }

    // Boundary check — polish must be a prefix OR suffix of kept.
    // Case-insensitive for ASCII, exact for non-ASCII.
    let (polish_cmp, kept_cmp): (String, String) = if polish.is_ascii() && kept.is_ascii() {
        (polish.to_ascii_lowercase(), kept.to_ascii_lowercase())
    } else {
        (polish.to_string(), kept.to_string())
    };
    kept_cmp.starts_with(&polish_cmp) || kept_cmp.ends_with(&polish_cmp)
}

/// True if `user_kept` looks like a USER_REWRITE rather than a small in-place
/// correction.  Heuristic — any of:
///   • user_kept length > 1.4× polish length (added substantial content)
///   • user_kept length > polish length + 30 chars (added a markdown link,
///     signature, etc.)
///   • polish appears as a contiguous substring of user_kept (i.e. user kept
///     polish verbatim and added a prefix/suffix), with extra content > 10 chars
///
/// This catches the email-link-prefix bug: user added `[anish@gmail.com]
/// (mailto:anish@gmail.com) ` before the otherwise-unchanged polish, and the
/// classifier hallucinated word-level "corrections".
pub fn looks_like_user_addition(polish: &str, user_kept: &str) -> bool {
    let polish_trim = polish.trim();
    let kept_trim   = user_kept.trim();

    let p_len = polish_trim.chars().count();
    let k_len = kept_trim.chars().count();

    if p_len == 0 || k_len == 0 {
        return false;
    }

    // Big proportional growth → REWRITE shape.
    if k_len as f64 > (p_len as f64) * 1.4 {
        return true;
    }
    // Big absolute growth → REWRITE shape.
    if k_len > p_len + 30 {
        return true;
    }
    // Polish kept verbatim + significant extra content wrapping it.
    if let Some(idx) = kept_trim.find(polish_trim) {
        let extra = (k_len - p_len).saturating_sub(0);
        let is_pure_addition = idx > 0 || (idx + p_len < k_len);
        if is_pure_addition && extra > 10 {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── appears_in_user_kept ──────────────────────────────────────────────────
    #[test]
    fn appears_finds_simple_match() {
        assert!(appears_in_user_kept("n8n", "I use n8n daily"));
        assert!(appears_in_user_kept("N8N", "i use n8n daily"));
    }

    #[test]
    fn appears_misses_substring_only() {
        // "ai" is inside "aiden" but not a whole word
        assert!(!appears_in_user_kept("ai", "aiden saw rain"));
    }

    #[test]
    fn appears_handles_punctuation_boundaries() {
        assert!(appears_in_user_kept("n8n", "Use [n8n](https://n8n.io) today."));
    }

    #[test]
    fn appears_rejects_hallucinated_devanagari() {
        // The bug case: the LLM proposed "अनीष" but user_kept is all Roman.
        assert!(!appears_in_user_kept("अनीष", "Anish at Gmail dot com"));
        assert!(!appears_in_user_kept("का",   "Anish at Gmail dot com ka zara batana"));
    }

    // ── script_matches ────────────────────────────────────────────────────────
    #[test]
    fn script_blocks_devanagari_in_hinglish_mode() {
        assert!(!script_matches("अनीष",  "hinglish"));
        assert!(!script_matches("का",    "hinglish"));
        assert!(!script_matches("ज़रा",  "english"));
    }

    #[test]
    fn script_allows_roman_in_hinglish_mode() {
        assert!(script_matches("n8n",     "hinglish"));
        assert!(script_matches("Vipassana","hinglish"));
        assert!(script_matches("kaam",    "hinglish"));
    }

    #[test]
    fn script_allows_devanagari_in_hindi_mode() {
        assert!(script_matches("अनीष", "hindi"));
        assert!(script_matches("kaam", "hindi"));   // Hindi mode tolerates Roman jargon
    }

    #[test]
    fn script_neutral_for_pure_digits() {
        assert!(script_matches("123",    "hinglish"));
        assert!(script_matches("v2.0",   "english"));
    }

    // ── looks_like_user_addition ──────────────────────────────────────────────
    #[test]
    fn user_addition_catches_markdown_link_prefix() {
        let polish    = "Anish at Gmail dot com ka zara batana kaun sa mail ID par bhejna hai";
        let user_kept = "[anish@gmail.com](mailto:anish@gmail.com) Anish at Gmail dot com ka zara batana kaun sa mail ID par bhejna hai";
        assert!(looks_like_user_addition(polish, user_kept),
                "markdown link prefix must be detected as user addition");
    }

    #[test]
    fn user_addition_ignores_in_place_word_swap() {
        // Same length, words just swapped — a real correction.
        let polish    = "I use written for automation";
        let user_kept = "I use n8n     for automation";
        assert!(!looks_like_user_addition(polish, user_kept));
    }

    #[test]
    fn user_addition_catches_full_rewrite() {
        let polish    = "short";
        let user_kept = "this is a totally different sentence rewritten from scratch";
        assert!(looks_like_user_addition(polish, user_kept));
    }

    #[test]
    fn user_addition_tolerates_small_typo_fix() {
        let polish    = "the meeting was good";
        let user_kept = "the meeting was great";
        assert!(!looks_like_user_addition(polish, user_kept));
    }

    // ── is_concatenation_pattern ──────────────────────────────────────────────

    #[test]
    fn concatenation_caught_for_emiac_maar_case() {
        // The exact production case: user wanted to replace "MAAR" with "EMIAC"
        // but cursor went to start of field, ended up with "EMIACMAAR".
        assert!(is_concatenation_pattern("MAAR", "EMIACMAAR"));
        assert!(is_concatenation_pattern("maar", "emiacmaar"));
    }

    #[test]
    fn concatenation_caught_for_prefix_and_suffix() {
        // polish at start, with a substantial suffix added
        assert!(is_concatenation_pattern("hello", "helloWORLD"));
        // polish at end, with a substantial prefix added (the EMIAC case shape)
        assert!(is_concatenation_pattern("server", "MYBIGserver"));
    }

    #[test]
    fn interior_containment_is_not_flagged() {
        // polish in the middle of kept (e.g. "server" inside "myserver-prod")
        // is rare and ambiguous — let normal gates handle it, don't block here.
        assert!(!is_concatenation_pattern("server", "myserver-prod"));
    }

    #[test]
    fn concatenation_ignored_for_clean_swap() {
        // Genuine STT correction — n8n is NOT a substring of written.
        assert!(!is_concatenation_pattern("written", "n8n"));
        assert!(!is_concatenation_pattern("Anis",    "anish"));
    }

    #[test]
    fn concatenation_ignored_when_polish_too_short() {
        // 1-2 char polish forms make false positives trivial — skip.
        assert!(!is_concatenation_pattern("ai", "aiden"));
        assert!(!is_concatenation_pattern("a",  "anish"));
    }

    #[test]
    fn concatenation_ignored_when_equal() {
        assert!(!is_concatenation_pattern("hello", "hello"));
        assert!(!is_concatenation_pattern("",      ""));
    }

    #[test]
    fn concatenation_ignored_when_polish_empty() {
        assert!(!is_concatenation_pattern("", "anything"));
    }

    #[test]
    fn concatenation_handles_non_ascii_prefix() {
        // Devanagari concatenation case (the polish-at-start variant).
        // We need ≥ 3 extra chars, so use a longer suffix.
        assert!(is_concatenation_pattern("नमस्ते", "नमस्तेजीसाहब"));
    }

    #[test]
    fn concatenation_unrelated_devanagari_not_flagged() {
        assert!(!is_concatenation_pattern("नमस्ते", "खुशी"));
    }
}

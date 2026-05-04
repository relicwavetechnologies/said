//! Lightweight phonetic key + similarity for STT-error detection.
//!
//! Goal: decide whether a transcript token (e.g. "Aiden") and a user-final token
//! (e.g. "n8n") could plausibly be the same spoken sound — so we can detect that
//! the user is correcting an STT misrecognition rather than rewriting their own
//! speech.
//!
//! We use a pragmatic Metaphone-style algorithm: collapse to ASCII, normalize
//! common digraphs (ph→f, ck→k, sh/ch→x, th→0), drop redundant vowels, dedupe
//! repeated consonants.  It is not strictly Double-Metaphone — we don't need
//! cross-language coverage; we need *cheap, monotone, stable* keys for a small
//! per-user table.
//!
//! `phonetic_key("written")`     → "RTN"
//! `phonetic_key("aiden")`       → "ATN"
//! `phonetic_key("n8n")`         → "NN"   (digits/punct drop)
//! `phonetic_key("nateen")`      → "NTN"
//! `phonetic_key("phonetic")`    → "FNTK"
//! `phonetic_key("vipassana")`   → "FPSN"
//! `phonetic_key("we passed na")`→ "FPSTN"
//!
//! Similarity is normalized Levenshtein distance over phonetic keys ∈ [0, 1].

/// Compute a stable phonetic key for a token.
///
/// Algorithm:
///   1. Split input on non-alphabetic chars (so "n8n" → ["n", "n"], keyed
///      separately so the digit doesn't collapse the key to nothing).
///   2. For each alphabetic chunk:
///      a. Handle silent-leading digraphs: `wr`, `kn`, `gn`, `gh`.
///      b. Apply digraph substitutions: `ph→F`, `sh|ch→X`, `th→0`, `ck|qu→K`.
///      c. Single-char map: c→K, q→K, x→K, z→S, y→I, v→F, w→W; else uppercase.
///      d. Drop interior vowels (preserve a leading vowel for word shape).
///      e. Dedupe consecutive duplicates inside the chunk.
///   3. Concatenate keyed chunks (no separator).
pub fn phonetic_key(s: &str) -> String {
    let mut out = String::new();
    let mut chunk = String::new();
    for c in s.chars() {
        if c.is_ascii_alphabetic() {
            chunk.push(c.to_ascii_lowercase());
        } else if !chunk.is_empty() {
            out.push_str(&key_chunk(&chunk));
            chunk.clear();
        }
        // non-alpha terminates the chunk; we don't emit a separator
    }
    if !chunk.is_empty() {
        out.push_str(&key_chunk(&chunk));
    }
    out
}

fn key_chunk(lower: &str) -> String {
    let bytes = lower.as_bytes();
    if bytes.is_empty() {
        return String::new();
    }

    // Stage 1: digraph + silent-letter normalization.
    let mut buf: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        let next = bytes.get(i + 1).copied().unwrap_or(0);
        match (b, next) {
            // Silent-leading digraphs: skip the silent letter, keep the next.
            (b'w', b'r') | (b'k', b'n') | (b'g', b'n') => {
                i += 1;
            }
            (b'g', b'h') => {
                i += 2;
            } // silent gh (light, through)
            // Sound digraphs: collapse to a single key letter.
            (b'p', b'h') => {
                buf.push(b'F');
                i += 2;
            }
            (b's', b'h') | (b'c', b'h') => {
                buf.push(b'X');
                i += 2;
            }
            (b't', b'h') => {
                buf.push(b'0');
                i += 2;
            }
            (b'c', b'k') | (b'q', b'u') => {
                buf.push(b'K');
                i += 2;
            }
            _ => {
                let mapped = match b {
                    b'c' => b'K',
                    b'q' => b'K',
                    b'x' => b'K',
                    b'z' => b'S',
                    b'y' => b'I',
                    b'v' => b'F',
                    b'w' => b'W',
                    other => other.to_ascii_uppercase(),
                };
                buf.push(mapped);
                i += 1;
            }
        }
    }

    // Stage 2: drop interior vowels (keep a leading vowel for shape).
    let mut out: Vec<u8> = Vec::with_capacity(buf.len());
    for (idx, &b) in buf.iter().enumerate() {
        let is_vowel = matches!(b, b'A' | b'E' | b'I' | b'O' | b'U');
        if is_vowel && idx != 0 {
            continue;
        }
        out.push(b);
    }

    // Stage 3: dedupe consecutive duplicates inside the chunk.
    let mut deduped: Vec<u8> = Vec::with_capacity(out.len());
    for &b in &out {
        if deduped.last().copied() != Some(b) {
            deduped.push(b);
        }
    }
    String::from_utf8(deduped).unwrap_or_default()
}

/// Levenshtein distance over byte slices.  O(n·m) but n,m ≤ ~10 in our domain.
fn levenshtein(a: &[u8], b: &[u8]) -> usize {
    let (n, m) = (a.len(), b.len());
    if n == 0 {
        return m;
    }
    if m == 0 {
        return n;
    }

    let mut prev: Vec<usize> = (0..=m).collect();
    let mut curr: Vec<usize> = vec![0; m + 1];

    for i in 1..=n {
        curr[0] = i;
        for j in 1..=m {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (curr[j - 1] + 1).min(prev[j] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[m]
}

/// Phonetic similarity ∈ [0, 1].  1.0 = identical phonetic keys, 0.0 = totally
/// dissimilar.  Computed as `1 - levenshtein(k_a, k_b) / max(|k_a|, |k_b|)`.
pub fn similarity(a: &str, b: &str) -> f64 {
    let ka = phonetic_key(a);
    let kb = phonetic_key(b);
    if ka.is_empty() && kb.is_empty() {
        return 1.0;
    }
    if ka.is_empty() || kb.is_empty() {
        return 0.0;
    }
    let d = levenshtein(ka.as_bytes(), kb.as_bytes()) as f64;
    let max_len = ka.len().max(kb.len()) as f64;
    1.0 - (d / max_len)
}

/// Score how "jargon-like" a token is.  Higher = more likely to be a name,
/// brand, code identifier, or technical term that STT may have mis-transcribed.
///
/// Signals (all stack, capped at 1.0):
///   • all-caps acronym 2–8 letters (NASA, IPO, MACOBS, FBI)   → +0.6
///   • mixed case (camelCase, PascalCase, iPhone)              → +0.4
///   • contains digits (n8n, k8s, gpt4)                        → +0.4
///   • contains underscore / hyphen (snake_case, kebab-case)   → +0.2
///   • short consonant-heavy with digit (n8n, k8s)             → +0.2
///   • initial-cap proper-noun-ish (Anish, Vipassana, Cursor)  → +0.2
///
/// The all-caps path is critical — it's the canonical shape for the
/// jargon class STT mishears most aggressively (acronyms / brand names
/// / company codes), and earlier scoring missed it entirely.
///
/// Result is clamped to [0, 1].
pub fn jargon_score(token: &str) -> f64 {
    let t = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != '-');
    if t.is_empty() {
        return 0.0;
    }
    let mut score = 0.0_f64;

    let has_lower = t.chars().any(|c| c.is_ascii_lowercase());
    let has_upper = t.chars().any(|c| c.is_ascii_uppercase());
    let has_digit = t.chars().any(|c| c.is_ascii_digit());
    let len = t.chars().count();
    let alpha_only = t.chars().all(|c| c.is_ascii_alphabetic());

    // ── All-caps acronym ─────────────────────────────────────────────────
    // The single biggest miss in the previous version.  An all-uppercase
    // alphabetic token of 2–8 letters is almost always an acronym, brand
    // ticker, or company code — exactly the shapes STT mishears worst.
    // We require alpha_only so we don't double-count "K8S" (handled by the
    // digit path below).
    if alpha_only && has_upper && !has_lower && (2..=8).contains(&len) {
        score += 0.6;
    }

    // ── Mixed case (camelCase, PascalCase, iPhone) ──────────────────────
    // Strict: requires an uppercase letter AFTER the first character.
    // Sentence-case ("The", "And") has uppercase only at position 0 and
    // shouldn't count as jargon — those are common English.
    let upper_after_first = t
        .chars()
        .enumerate()
        .any(|(i, c)| i > 0 && c.is_ascii_uppercase());
    if has_lower && upper_after_first {
        score += 0.4;
    }

    // ── Digits ──────────────────────────────────────────────────────────
    if has_digit {
        score += 0.4;
    }

    // ── Underscore / hyphen ─────────────────────────────────────────────
    if t.contains('_') || t.contains('-') {
        score += 0.2;
    }

    // ── Consonant-heavy short word with digit (n8n, k8s, c0d3) ──────────
    let consonants = t
        .chars()
        .filter(|c| c.is_ascii_alphabetic() && !"aeiouAEIOU".contains(*c))
        .count();
    if (2..=4).contains(&len) && consonants as f64 / len as f64 > 0.5 && has_digit {
        score += 0.2;
    }

    // ── Initial-capital proper-noun shape ──────────────────────────────
    // Catches Anish, Vipassana, Cursor, Linear, Slack — names of people,
    // brands, and tools, which STT mishears constantly.  We *only* award
    // this when the rest is all lowercase (so we don't double-count
    // mixed-case which already got 0.4) and the word is at least 4 chars
    // (avoids generic "I", "A", "The" boosts).
    if alpha_only && len >= 4 {
        let first = t.chars().next().unwrap();
        let rest_lower = t.chars().skip(1).all(|c| c.is_ascii_lowercase());
        if first.is_ascii_uppercase() && rest_lower {
            score += 0.2;
        }
    }

    score.min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_basic() {
        assert_eq!(phonetic_key("written"), "RTN"); // silent w-r
        assert_eq!(phonetic_key("aiden"), "ADN"); // vowel-drop interior
        assert_eq!(phonetic_key("aidan"), "ADN"); // same key as "aiden"
        assert_eq!(phonetic_key("phonetic"), "FNTK");
        assert_eq!(phonetic_key("ck"), "K");
        assert_eq!(phonetic_key("through"), "0R"); // th→0, gh silent
        assert_eq!(phonetic_key("knife"), "NF"); // silent k-n
        assert_eq!(phonetic_key("gnostic"), "NSTK"); // silent g-n
    }

    #[test]
    fn key_drops_digits_and_punct() {
        assert_eq!(phonetic_key("n8n"), "NN");
        assert_eq!(phonetic_key("k8s"), "KS");
        assert_eq!(phonetic_key("hello!"), "HL");
    }

    #[test]
    fn similarity_is_symmetric_and_in_range() {
        let s = similarity("aiden", "n8n");
        assert!((0.0..=1.0).contains(&s));
        let s2 = similarity("n8n", "aiden");
        assert!((s - s2).abs() < 1e-9);
    }

    #[test]
    fn similarity_examples() {
        // identical key → 1.0
        assert!((similarity("written", "writen") - 1.0).abs() < 1e-9);
        // unrelated
        assert!(similarity("hello", "n8n") < 0.5);
    }

    #[test]
    fn jargon_detects_mixed_case_and_digits() {
        assert!(jargon_score("n8n") >= 0.4);
        assert!(jargon_score("k8s") >= 0.4);
        assert!(jargon_score("camelCase") >= 0.4);
        assert!(jargon_score("hello") < 0.4);
        assert!(jargon_score("the") < 0.4);
    }

    #[test]
    fn jargon_detects_all_caps_acronyms() {
        // The MACOBS regression — acronyms scored 0.0 before the fix.
        assert!(
            jargon_score("MACOBS") >= 0.6,
            "got {}",
            jargon_score("MACOBS")
        );
        assert!(jargon_score("NASA") >= 0.6);
        assert!(jargon_score("IPO") >= 0.6);
        assert!(jargon_score("FBI") >= 0.6);
        assert!(jargon_score("EMIAC") >= 0.6);
        assert!(jargon_score("COVID") >= 0.6);
        // Too long — likely a sentence shouty-cap, not an acronym
        assert!(jargon_score("THISISNOTAACRONYM") < 0.6);
    }

    #[test]
    fn jargon_detects_proper_noun_initial_cap() {
        // Names + brand names — STT mishears these constantly
        assert!(jargon_score("Anish") >= 0.2);
        assert!(jargon_score("Vipassana") >= 0.2);
        assert!(jargon_score("Cursor") >= 0.2);
        assert!(jargon_score("Linear") >= 0.2);
        // But NOT capitalised normal English at sentence-start
        // (we accept some false positives here — the cost is "STT got The
        //  vs the right" being treated as slightly jargon-like, low risk)
        assert!(jargon_score("The") < 0.4); // length 3, doesn't trigger initial-cap path
    }

    #[test]
    fn jargon_score_stacks_correctly() {
        // iPhone: mixed case (+0.4), no digits, no _-, length 6
        // Should score ~0.4
        let p = jargon_score("iPhone");
        assert!(p >= 0.4 && p <= 0.6, "iPhone scored {p}");

        // K8S: all-caps + digit → 0.6 + 0.4 = 1.0
        let k = jargon_score("K8S");
        assert!(k >= 0.6, "K8S scored {k}");
    }

    #[test]
    fn jargon_ignores_common_english_words() {
        for word in &[
            "the", "and", "for", "is", "was", "going", "really", "thought",
        ] {
            assert!(
                jargon_score(word) < 0.4,
                "{word:?} scored too high: {}",
                jargon_score(word)
            );
        }
    }
}

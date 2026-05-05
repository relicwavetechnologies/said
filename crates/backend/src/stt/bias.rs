use std::collections::{HashMap, HashSet};

use voice_polish_core::deepgram::{BiasPackage, ReplacementRule, resolve_stt_mode};

use crate::{
    llm::phonetics,
    store::{DbPool, stt_replacements, vocabulary},
};

fn is_precise_keyterm(term: &vocabulary::VocabTerm) -> bool {
    matches!(term.source.as_str(), "manual" | "starred")
        || matches!(
            term.term_type.as_deref(),
            Some("acronym" | "proper_noun" | "brand" | "code_identifier" | "phrase")
        )
        || term.use_count > 1
        || term.weight >= 1.5
}

fn replacement_threshold(term: Option<&vocabulary::VocabTerm>, correct_form: &str) -> i64 {
    let jargon_like = term
        .and_then(|t| t.term_type.as_deref())
        .map(|kind| {
            matches!(
                kind,
                "acronym" | "proper_noun" | "brand" | "code_identifier" | "phrase"
            )
        })
        .unwrap_or(false)
        || phonetics::jargon_score(correct_form) >= 0.4;
    if jargon_like { 2 } else { 3 }
}

pub fn build_bias_package(
    pool: &DbPool,
    user_id: &str,
    transcription_language: &str,
    output_language: &str,
) -> BiasPackage {
    let stt_mode = resolve_stt_mode(transcription_language);
    let vocab_terms = vocabulary::top_terms_for_language(pool, user_id, output_language, 200);

    let mut seen_terms = HashSet::new();
    let mut keyterms = Vec::new();
    for term in vocab_terms.iter().filter(|t| is_precise_keyterm(t)) {
        let lowered = term.term.to_ascii_lowercase();
        if seen_terms.insert(lowered) {
            keyterms.push(term.term.clone());
        }
        if keyterms.len() >= voice_polish_core::deepgram::MAX_KEYTERMS {
            break;
        }
    }

    let vocab_by_term: HashMap<String, vocabulary::VocabTerm> = vocab_terms
        .into_iter()
        .map(|term| (term.term.to_ascii_lowercase(), term))
        .collect();

    let mut seen_replacements = HashSet::new();
    let mut replacements = Vec::new();
    for rule in stt_replacements::load_for_language(pool, user_id, output_language) {
        let find = rule.transcript_form.trim();
        let replace = rule.correct_form.trim();
        if find.is_empty() || replace.is_empty() {
            continue;
        }
        if find.eq_ignore_ascii_case(replace) {
            continue;
        }

        let Some(canonical) = vocab_by_term.get(&replace.to_ascii_lowercase()) else {
            continue;
        };
        let threshold = replacement_threshold(Some(canonical), replace);
        if rule.use_count < threshold {
            continue;
        }

        let dedupe_key = format!(
            "{}=>{}",
            find.to_ascii_lowercase(),
            replace.to_ascii_lowercase()
        );
        if !seen_replacements.insert(dedupe_key) {
            continue;
        }
        replacements.push(ReplacementRule {
            find: find.to_ascii_lowercase(),
            replace: Some(replace.to_string()),
        });
        if replacements.len() >= voice_polish_core::deepgram::MAX_REPLACEMENTS {
            break;
        }
    }

    BiasPackage {
        stt_mode,
        keyterms,
        replacements,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{DbPool, stt_replacements, vocabulary};
    use r2d2_sqlite::SqliteConnectionManager;

    fn mem_pool() -> DbPool {
        let mgr = SqliteConnectionManager::memory();
        let pool = r2d2::Pool::builder().max_size(1).build(mgr).unwrap();
        let conn = pool.get().unwrap();
        conn.execute_batch(
            "CREATE TABLE local_user (id TEXT PRIMARY KEY);
             INSERT INTO local_user(id) VALUES ('u1');
             CREATE TABLE vocabulary (
                 user_id TEXT NOT NULL,
                 term TEXT NOT NULL,
                 weight REAL NOT NULL,
                 use_count INTEGER NOT NULL,
                 last_used INTEGER NOT NULL,
                 source TEXT NOT NULL,
                 language TEXT,
                 example_context TEXT,
                 term_type TEXT,
                 meaning TEXT,
                 meaning_updated_at INTEGER,
                 examples_since_meaning INTEGER NOT NULL DEFAULT 0,
                 UNIQUE(user_id, term)
             );
             CREATE TABLE stt_replacements (
                 user_id TEXT NOT NULL,
                 transcript_form TEXT NOT NULL,
                 correct_form TEXT NOT NULL,
                 phonetic_key TEXT NOT NULL,
                 weight REAL NOT NULL,
                 use_count INTEGER NOT NULL,
                 last_used INTEGER NOT NULL,
                 language TEXT,
                 UNIQUE(user_id, transcript_form, correct_form)
             );",
        )
        .unwrap();
        pool
    }

    #[test]
    fn builds_multi_bias_and_filters_low_trust_replacements() {
        let pool = mem_pool();
        vocabulary::upsert_for_language_with_context(
            &pool,
            "u1",
            "EMIAC",
            2.0,
            "manual",
            "hinglish",
            Some("EMIAC technology ke baare mein"),
        );
        vocabulary::upsert_for_language_with_context(
            &pool,
            "u1",
            "return",
            1.0,
            "auto",
            "hinglish",
            Some("return ka automation"),
        );
        stt_replacements::upsert_aliases_for_language(
            &pool, "u1", "emi", "emi", "EMIAC", 1.0, "hinglish",
        );
        stt_replacements::upsert_aliases_for_language(
            &pool, "u1", "emi", "emi", "EMIAC", 1.0, "hinglish",
        );
        stt_replacements::upsert_aliases_for_language(
            &pool, "u1", "return", "return", "return", 1.0, "hinglish",
        );
        stt_replacements::upsert_aliases_for_language(
            &pool, "u1", "return", "return", "return", 1.0, "hinglish",
        );

        let bias = build_bias_package(&pool, "u1", "auto", "hinglish");
        assert_eq!(bias.stt_mode, "multi");
        assert!(bias.keyterms.contains(&"EMIAC".to_string()));
        assert!(
            bias.replacements
                .iter()
                .any(|r| r.find == "emi" && r.replace.as_deref() == Some("EMIAC"))
        );
        assert!(
            !bias
                .replacements
                .iter()
                .any(|r| r.find == "return" && r.replace.as_deref() == Some("return"))
        );
    }
}

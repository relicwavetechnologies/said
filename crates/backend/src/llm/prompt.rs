//! RACC prompt builder.
//!
//! Structure (injection-safe: transcript always last, tag-wrapped):
//!
//! ```text
//! <output_language> … enforced script rule … </output_language>
//! <role> … persona … </role>
//! <tone> … tone preset … </tone>
//! <preferences>
//!   (optional RAG examples of user edits)
//! </preferences>
//! <task> … instructions … </task>
//! <transcript> {transcript} </transcript>
//! ```

use crate::store::{corrections::Correction, prefs::Preferences, vocabulary::VocabTerm};

/// Render a single vocab entry for the polish prompt. Output shape:
///   `  MACOBS [acronym]`
///   `    means: indian SME stock acronym used in market-cap discussions`
///   `    example: "MACOBS ka IPO ka 12 hazaar batana"`
///
/// Three layers of structured signal in one entry:
///   • The bracketed type tag drives type-aware reasoning (an acronym entry
///     must not match a single common English word).
///   • The `means:` line carries the LLM-distilled semantic description,
///     refined over time. The polish LLM can semantic-align the transcript
///     context against this instead of inferring from one example.
///   • The `example:` line preserves a concrete usage shape for the cases
///     where a semantically-noisy meaning still needs a literal anchor.
///
/// All three lines are optional — entries without context, type, or meaning
/// degrade gracefully (just the term, just the type, etc.).
fn format_vocab_entry(e: &VocabEntry) -> String {
    let type_label: String = match e.term_type.as_deref() {
        Some("acronym") => " [acronym]".into(),
        Some("proper_noun") => " [proper noun]".into(),
        Some("brand") => " [brand]".into(),
        Some("code_identifier") => " [code identifier]".into(),
        Some("phrase") => " [phrase]".into(),
        Some("other") | None => String::new(), // no signal — render bare
        Some(other) => format!(" [{other}]"),
    };
    let mut out = format!("  {}{type_label}", e.term);
    if let Some(m) = &e.meaning {
        let m = m.trim();
        if !m.is_empty() {
            out.push_str(&format!("\n    means: {m}"));
        }
    }
    if let Some(ctx) = &e.context {
        let ctx = ctx.trim();
        if !ctx.is_empty() {
            out.push_str(&format!("\n    example: \"{ctx}\""));
        }
    }
    out
}

pub struct RagExample {
    pub ai_output: String,
    pub user_kept: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VocabResolution {
    Candidate,
    Resolved,
}

/// One vocabulary entry as fed to the polish prompt. Carries the canonical
/// term and (optionally) an example sentence the term was first observed
/// in. The example is what enables context-aware recognition of unseen STT
/// mishearings: when polish sees "main course ka IPO" but the vocab has
/// `term="MACOBS"` with `context="MACOBS ka IPO ka 12 hazaar batana"`,
/// the LLM can match the *context shape* and output MACOBS even though
/// the literal "main course" isn't a stored alias.
#[derive(Clone)]
pub struct VocabEntry {
    pub term: String,
    pub context: Option<String>,
    pub resolution: VocabResolution,
    /// Lexical-shape classification ("acronym" / "proper_noun" / "brand" /
    /// "code_identifier" / "phrase" / "other"). Used by the polish prompt
    /// to render structured, type-aware entries so the LLM can reason from
    /// signals (an acronym entry should not match a common single word)
    /// instead of needing hardcoded exception lists.
    pub term_type: Option<String>,
    /// LLM-distilled 1-2 sentence description of what the term refers to
    /// and the contexts it appears in, refined over time as more examples
    /// accumulate. When present, the polish prompt surfaces it so the LLM
    /// can do semantic alignment (does the transcript context match this
    /// term's meaning?) instead of inferring from a single example. None
    /// when meaning hasn't been generated yet — entry still renders, just
    /// without the meaning line.
    pub meaning: Option<String>,
}

impl VocabEntry {
    pub fn from_term(term: impl Into<String>) -> Self {
        Self {
            term: term.into(),
            context: None,
            resolution: VocabResolution::Candidate,
            term_type: None,
            meaning: None,
        }
    }
}

pub fn vocab_terms_to_entries(terms: Vec<VocabTerm>) -> Vec<VocabEntry> {
    terms
        .into_iter()
        .map(|v| VocabEntry {
            term: v.term,
            context: v.example_context,
            resolution: VocabResolution::Candidate,
            term_type: v.term_type,
            meaning: v.meaning,
        })
        .collect()
}

pub fn resolved_vocab_terms_to_entries(terms: Vec<VocabTerm>) -> Vec<VocabEntry> {
    terms
        .into_iter()
        .map(|v| VocabEntry {
            term: v.term,
            context: v.example_context,
            resolution: VocabResolution::Resolved,
            term_type: v.term_type,
            meaning: v.meaning,
        })
        .collect()
}

/// Build the full system-prompt string.
///
/// `corrections` are LLM-polish substitutions learned from past POLISH_ERRORs.
/// They are applied *contextually* (not mandatorily) — the LLM is told to
/// prefer the right-hand form when the left-hand form would otherwise appear,
/// but is allowed to skip when context makes the substitution unnatural. This
/// is intentional: a hard always-replace rule on a common English word would
/// corrupt unrelated sentences.
///
/// `vocabulary` is the user's personal STT-bias vocabulary.  We pass it into
/// the polish prompt as well, so the LLM is told: "if you see any of these
/// terms in the transcript, KEEP THEM VERBATIM."  This stops the polish step
/// from helpfully "fixing" learned jargon back into a wrong common word.
///
/// `rag_examples` are embedding-based similar past edits (contextual).
pub fn build_system_prompt(
    prefs: &Preferences,
    rag_examples: &[RagExample],
    corrections: &[Correction],
) -> String {
    build_system_prompt_with_vocab(prefs, rag_examples, corrections, &[])
}

/// Backwards-compatible wrapper — wraps bare term strings into VocabEntry
/// values with no context. Prefer `build_system_prompt_with_vocab_entries`
/// for new code so contexts can flow through.
pub fn build_system_prompt_with_vocab(
    prefs: &Preferences,
    rag_examples: &[RagExample],
    corrections: &[Correction],
    vocabulary_terms: &[String],
) -> String {
    let entries: Vec<VocabEntry> = vocabulary_terms
        .iter()
        .map(|t| VocabEntry::from_term(t.clone()))
        .collect();
    build_system_prompt_with_vocab_entries(prefs, rag_examples, corrections, &entries)
}

/// Full builder with context-aware vocabulary. Each `VocabEntry` may carry
/// an example sentence the term was observed in; the polish prompt surfaces
/// these so the LLM can do context-aware recognition of mishearings.
pub fn build_system_prompt_with_vocab_entries(
    prefs: &Preferences,
    rag_examples: &[RagExample],
    corrections: &[Correction],
    vocabulary_entries: &[VocabEntry],
) -> String {
    let lang_rule = language_rule(&prefs.output_language);
    let persona = persona_block(prefs);
    let tone = tone_description(&prefs.tone_preset);
    let script_check = script_final_check(&prefs.output_language);

    // Vocabulary block — compact form. Each entry shows canonical + type tag +
    // example_context. The type tag carries the foundational signal (an
    // acronym entry is type-incompatible with a single common word). We
    // intentionally avoid verbose explanations that would expand the
    // prompt with decision-style language ("you may emit", "candidates")
    // — that framing pushes the LLM into evaluate-multiple-options mode
    // and was the root cause of duplicate-output regressions.
    //
    // Two-line instruction = enough. The model uses the type+example
    // signals to decide; the global single-output rule (in <task>) keeps
    // it from emitting multiple variants.
    let vocab_block = if vocabulary_entries.is_empty() {
        String::new()
    } else {
        let resolved = vocabulary_entries
            .iter()
            .filter(|e| e.resolution == VocabResolution::Resolved)
            .map(format_vocab_entry)
            .collect::<Vec<_>>()
            .join("\n");
        let candidates = vocabulary_entries
            .iter()
            .filter(|e| e.resolution == VocabResolution::Candidate)
            .map(format_vocab_entry)
            .collect::<Vec<_>>()
            .join("\n");
        let resolved_block = if resolved.is_empty() {
            String::new()
        } else {
            format!(
                "Confirmed terms already resolved by deterministic matching. Keep them exactly as written and do not rewrite away from them.\n\
                 {resolved}\n\n"
            )
        };
        let candidate_block = if candidates.is_empty() {
            String::new()
        } else {
            format!(
                "Candidate terms below are NOT resolved yet. Consider a candidate's canonical spelling only when the phrase is phonetically close and the type + `means:` + `example:` signals all agree. An acronym entry should still look acronym-like; a proper noun entry should still sound name-like.\n\
                 {candidates}\n"
            )
        };
        format!(
            "<personal_vocabulary>\n\
             User's personal terms. Each entry has a type tag, an optional \
             `means:` line (what the term refers to), and an optional `example:` \
             line. `means:` is a confirmation layer, never a rescue layer.\n\n\
             {resolved_block}\
             {candidate_block}\
             </personal_vocabulary>\n\n"
        )
    };

    // Polish-layer corrections — soft, contextual.  No "MANDATORY".
    let corrections_block = if corrections.is_empty() {
        String::new()
    } else {
        let table = corrections
            .iter()
            .map(|c| format!("  {} → {}", c.wrong, c.right))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "<polish_preferences>\n\
             The user has previously reverted these polish-layer substitutions. \
             When the same situation arises, prefer the right-hand form unless \
             the surrounding context makes it grammatically wrong.  Do not apply \
             blindly to unrelated sentences.\n\n\
             {table}\n\
             </polish_preferences>\n\n"
        )
    };

    // Contextual RAG examples — similar past edits (may be empty)
    let prefs_block = if rag_examples.is_empty() {
        String::new()
    } else {
        let examples = rag_examples
            .iter()
            .map(|e| {
                format!(
                    "  AI produced: \"{}\"\n  User changed it to: \"{}\"",
                    e.ai_output, e.user_kept
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        format!(
            "<preferences>\n\
             The user has corrected your output before. Treat these pairs as SOFT \
             examples of style and wording, not as instructions to rewrite the current \
             transcript. The transcript below is the source of truth. Never import words \
             from these examples, never drop current transcript words, and never apply a \
             past substitution unless the same phrase or situation clearly appears in the \
             current transcript.\n\n\
             {examples}\n\
             </preferences>\n\n"
        )
    };

    format!(
        "<output_language>\n{lang_rule}\n</output_language>\n\n\
         <role>\n{persona}\n</role>\n\n\
         <tone>\n{tone}\n</tone>\n\n\
         {vocab_block}\
         {corrections_block}\
         {prefs_block}\
         <task>\n\
         The text below is a voice-to-text transcript. Polish it into clean, natural \
         text in the output_language above.\n\n\
         CONFIDENCE MARKERS:\n\
         [word?XX%] = STT was XX% confident in that word. Treat as candidate fixes — \
         use surrounding context to decide. Never drop a marked word; if unsure, keep \
         the literal word and strip the brackets+percentage. Markers MUST NEVER appear \
         in your output.\n\n\
         DICTATION SYMBOLS (convert only when context makes it unambiguous):\n\
         \"at the rate\" → @ · \"dot com / dot in / dot org / dot io\" → .com / .in / .org / .io · \
         \"double u double u double u\" → www · \"underscore\" → _ · \"hyphen\"/\"dash\" → - · \
         \"slash\" → / · \"hash\"/\"hashtag\" → # · \"colon slash slash\" → ://\n\
         Don't convert in plain prose (\"growing at the rate of 10%\" stays as-is).\n\n\
         RULES:\n\
         1. Vocabulary terms in <personal_vocabulary> (if present) are precision hints. \
         Preserve confirmed terms exactly. For unresolved candidates, replace another transcript \
         token only when it is phonetically close and the type + meaning + example checks also agree.\n\
         2. Preserve every content word. Remove ONLY fillers (um, uh, hmm, like, you know, \
         basically, matlab, toh, yaani, bas) and stuttered repetitions (\"the the cat\" → \"the cat\"). \
         Do NOT drop names, numbers, dates, jargon, adjectives, adverbs.\n\
         3. Fix grammar, punctuation, capitalization, sentence boundaries. Don't summarise or rewrite.\n\
         4. Output ONLY the polished text — no preamble, no commentary, no markdown.\n\
         5. <polish_preferences> are soft hints; prefer the right-hand form when context allows.\n\
         6. <preferences> are SOFT examples of style; never import their words into the current transcript.\n\n\
         SCRIPT CHECK before your first character:\n\
         {script_check}\n\n\
         FINAL RULE — SINGLE OUTPUT:\n\
         Write the polished text ONCE and stop. No repetition, no paraphrasing, no \"cleaner version\". \
         Even on ambiguous input, commit to one version.\n\
         </task>"
    )
}

/// Build a system prompt for the tray "Polish my message" feature.
///
/// Output language is always English (it is baked into the preset label).
/// For "custom" the caller passes the user's stored custom_prompt as `tone_preset`.
/// No RAG — this is a one-shot, context-free polish.
pub fn build_tray_system_prompt(tone_preset: &str) -> String {
    let lang_rule = "ABSOLUTE RULE — OUTPUT LANGUAGE: English only.\n\
                     Every word must be in English. If the text contains Hindi or any \
                     other language, translate it to natural English. \
                     Do NOT output Devanagari, Roman Hindi, or any non-English script.";

    let tone = tone_description(tone_preset);

    format!(
        "<output_language>\n{lang_rule}\n</output_language>\n\n\
         <tone>\n{tone}\n</tone>\n\n\
         <task>\n\
         Polish the text below into clean, natural English.\n\
         Output ONLY the polished text — no preamble, no commentary, no markdown.\n\
         The output_language rule above is ABSOLUTE.\n\
         Remove disfluencies (um, uh, like, basically, you know).\n\
         Honour the tone above.\n\
         </task>"
    )
}

/// Build the user message (transcript wrapped in tags — injection-safe).
///
/// `output_language` drives a one-line script reminder prepended to the
/// message — right before the transcript, closest to where the model
/// starts generating.  This counters the tendency to echo the script of
/// the transcript itself on the very first word.
pub fn build_user_message(transcript: &str, output_language: &str) -> String {
    let reminder = match output_language {
        "hindi" => "Output in Devanagari script only.\n",
        "english" => "Output in English only — no Devanagari, no Roman Hindi.\n",
        // hinglish / default
        _ => {
            "Output in Roman script (no Devanagari) AND preserve Hindi words as Hindi — \
              never translate them to English. \"kaam\" stays \"kaam\", not \"work\". \
              \"bahut\" stays \"bahut\", not \"a lot\".\n"
        }
    };
    format!("{reminder}<transcript>\n{transcript}\n</transcript>")
}

/// A sharp per-language script reminder injected at the BOTTOM of the
/// `<task>` block — closest context before the model starts writing.
fn script_final_check(output_language: &str) -> &'static str {
    match output_language {
        "hindi" => "Your entire output must be Devanagari. No Roman script.\n",
        "english" => "Your entire output must be English. No Devanagari, no Roman Hindi.\n",
        // hinglish / default — two failure modes the LLM tends toward:
        //   (a) starting the output in Devanagari (script slip)
        //   (b) translating Hindi words to English (silent over-helpfulness)
        // The check below catches BOTH before the first character is emitted.
        _ => {
            "Two checks before writing your first character:\n\
              1. SCRIPT — Roman letters only. ZERO Devanagari (देख → \"Dekh\", भाई → \"bhai\"). \
              Check the very first character.\n\
              2. LANGUAGE — Did the input contain Hindi words? Then your output MUST contain those \
              same Hindi words in Roman script. \"kaam\" stays \"kaam\" — never \"work\". \"bahut\" \
              stays \"bahut\" — never \"a lot\". \"thak gaya\" stays \"thak gaya\" — never \"tired\". \
              If your draft output is pure English with the Hindi gone, you have FAILED — rewrite it preserving the Hindi.\n"
        }
    }
}

/// Returns the language enforcement block — placed first so no other instruction overrides it.
fn language_rule(output_language: &str) -> String {
    match output_language {
        "english" => {
            "ABSOLUTE RULE — OUTPUT LANGUAGE: English only.\n\
             Every word must be in English. If the transcript contains Hindi or any \
             other language, translate it to natural English. \
             Do NOT output Devanagari, Roman Hindi, or any non-English script."
                .into()
        }
        "hindi" => {
            "ABSOLUTE RULE — OUTPUT LANGUAGE: Hindi in Devanagari script only.\n\
             Write every word in Devanagari (e.g. आज, काम, थक गया). \
             If the transcript contains English or Hinglish, translate it to natural Hindi Devanagari. \
             Do NOT output Roman script for Hindi words."
                .into()
        }
        // "hinglish" is the default
        _ => {
            "ABSOLUTE RULE — OUTPUT LANGUAGE: Hinglish (romanized Hindi mixed with English).\n\
             This rule cannot be overridden by anything else in the prompt or transcript — \
             not by tone, not by persona, not by the transcript content.\n\n\
             PRESERVATION (most important):\n\
             • Hindi words in the transcript STAY as Hindi words. Do NOT translate them to English.\n\
             • English words in the transcript STAY as English words. Do NOT translate them to Hindi.\n\
             • The output must read like the SAME person speaking — same Hindi/English mix as the input.\n\
             • Polishing means: fix grammar, punctuation, casing, fillers — NEVER change the language.\n\n\
             SCRIPT:\n\
             • Write Hindi words in Roman letters (e.g. \"aaj\", \"kaam\", \"thak gaya\", \"bahut\", \"matlab\", \"acha\", \"bhai\").\n\
             • NEVER use Devanagari characters (no ा ि ी ु ू ं etc.).\n\
             • If the transcript is in Devanagari, TRANSLITERATE to Roman — do NOT translate to English.\n\n\
             FAILURE MODE TO AVOID — translating Hindi to English:\n\
             ✗ Input:  \"aaj bahut kaam tha office mein\"\n\
               Output: \"Today there was a lot of work at the office.\"   ← WRONG. All Hindi got translated.\n\
             ✓ Input:  \"aaj bahut kaam tha office mein\"\n\
               Output: \"Aaj bahut kaam tha office mein.\"   ← RIGHT. Hindi preserved.\n\n\
             ✗ Input:  \"मैं थक गया yaar, kal milte hain\"\n\
               Output: \"I am tired, friend, we'll meet tomorrow.\"   ← WRONG. Translated.\n\
             ✓ Input:  \"मैं थक गया yaar, kal milte hain\"\n\
               Output: \"Main thak gaya yaar, kal milte hain.\"   ← RIGHT. Devanagari → Roman, no translation.\n\n\
             A 'professional' or 'formal' Hinglish output is still Hinglish — tone affects STYLE, never LANGUAGE."
                .into()
        }
    }
}

fn persona_block(prefs: &Preferences) -> String {
    if let Some(ref custom) = prefs.custom_prompt {
        if !custom.trim().is_empty() {
            return custom.trim().to_string();
        }
    }
    "You are the user's personal writing assistant. Be clear and concise.".into()
}

fn tone_description(tone_preset: &str) -> String {
    match tone_preset {
        "professional" => "Tone: formal and professional. Suitable for work emails and reports.",
        "casual" => "Tone: friendly and conversational. Light and easy to read.",
        "assertive" => "Tone: direct and confident. Clear calls-to-action.",
        "concise" => "Tone: minimal words. Remove every unnecessary word.",
        "neutral" => "Tone: neutral and clear. No strong stylistic lean.",
        _ => "Tone: neutral and clear.",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{corrections::Correction, prefs::Preferences};

    fn prefs() -> Preferences {
        Preferences {
            user_id: "u1".into(),
            selected_model: "smart".into(),
            tone_preset: "neutral".into(),
            custom_prompt: None,
            language: "auto".into(),
            output_language: "english".into(),
            auto_paste: true,
            edit_capture: true,
            polish_text_hotkey: "cmd+shift+p".into(),
            deepgram_api_key: None,
            gemini_api_key: None,
            gateway_api_key: None,
            groq_api_key: None,
            llm_provider: "gateway".into(),
            updated_at: 0,
        }
    }

    #[test]
    fn vocab_block_appears_when_terms_present() {
        let p = prefs();
        let prompt =
            build_system_prompt_with_vocab(&p, &[], &[], &["n8n".into(), "Vipassana".into()]);
        assert!(
            prompt.contains("<personal_vocabulary>"),
            "vocab block should be emitted"
        );
        assert!(prompt.contains("n8n"));
        assert!(prompt.contains("Vipassana"));
        // Compact form: the canonical-spelling rule appears (now phrased as
        // "Replace ... with the canonical spelling ONLY when ALL THREE align"
        // since the three-layer matching upgrade — lexical + type + meaning).
        assert!(
            prompt.contains("canonical spelling"),
            "canonical-spelling instruction should appear in compact form"
        );
        // The vocab block should NOT contain the verbose multi-rule form
        // that caused duplicate-output regressions.
        assert!(
            !prompt.contains("**Verbatim match**"),
            "verbose numbered-rule form must be removed"
        );
        assert!(
            !prompt.contains("**Mishearing recognition**"),
            "verbose numbered-rule form must be removed"
        );
    }

    #[test]
    fn vocab_block_absent_when_no_terms() {
        let p = prefs();
        let prompt = build_system_prompt_with_vocab(&p, &[], &[], &[]);
        assert!(
            !prompt.contains("<personal_vocabulary>\n"),
            "expected no vocabulary block when terms are empty"
        );
        assert!(
            !prompt.contains("KEEP the canonical"),
            "vocab instructions should be gated on having terms"
        );
    }

    #[test]
    fn vocab_block_compact_form_no_verbose_rules() {
        // FOUNDATIONAL: the previous prompt had a 40+ line verbose vocab
        // block with numbered rules + sub-bullets + Q&A-style examples.
        // That framing pushed the LLM into "evaluate multiple candidates"
        // mode and caused duplicate-output regressions (LLM would emit
        // its first version, then a paraphrased "alternative").
        // The compact form keeps the type+example signals and a 1-line
        // rule — no Q&A examples, no decision-style language.
        let p = prefs();
        let entries = vec![VocabEntry {
            term: "MACOBS".into(),
            context: Some("MACOBS ka IPO".into()),
            resolution: VocabResolution::Candidate,
            term_type: Some("acronym".into()),
            meaning: None,
        }];
        let prompt = build_system_prompt_with_vocab_entries(&p, &[], &[], &entries);

        // Old verbose markers must all be GONE.
        assert!(
            !prompt.contains("COMMON-WORD SAFEGUARD"),
            "stopword safeguard heading must be removed"
        );
        assert!(
            !prompt.contains("\"the\", \"a\", \"is\""),
            "enumerated stopword list must be removed"
        );
        assert!(
            !prompt.contains("type-compatible"),
            "verbose 'type-compatible' explainer is gone (kept implicit in 1-line rule)"
        );
        assert!(
            !prompt.contains("Each entry below is a CANDIDATE"),
            "decision-style 'CANDIDATE' framing must be gone"
        );
        // The new compact form must contain the type-shape rule inline.
        assert!(
            prompt.contains("acronym entry"),
            "compact rule mentions acronym type as the canonical example"
        );
        assert!(
            prompt.contains("never a rescue layer"),
            "semantic meaning must be constrained as confirmation, not expansion"
        );
    }

    #[test]
    fn resolved_terms_render_in_preserve_only_section() {
        let p = prefs();
        let entries = vec![VocabEntry {
            term: "MACOBS".into(),
            context: Some("MACOBS ka IPO".into()),
            resolution: VocabResolution::Resolved,
            term_type: Some("acronym".into()),
            meaning: Some("Indian SME stock acronym.".into()),
        }];
        let prompt = build_system_prompt_with_vocab_entries(&p, &[], &[], &entries);
        assert!(prompt.contains("Confirmed terms already resolved by deterministic matching"));
        assert!(prompt.contains("Keep them exactly as written"));
        assert!(prompt.contains("MACOBS [acronym]"));
        assert!(!prompt.contains("Candidate terms below are NOT resolved yet.\n  MACOBS"));
    }

    #[test]
    fn candidate_terms_render_in_confirm_only_section() {
        let p = prefs();
        let entries = vec![VocabEntry {
            term: "n8n".into(),
            context: Some("I run n8n for automations".into()),
            resolution: VocabResolution::Candidate,
            term_type: Some("code_identifier".into()),
            meaning: Some("Workflow automation tool.".into()),
        }];
        let prompt = build_system_prompt_with_vocab_entries(&p, &[], &[], &entries);
        assert!(prompt.contains("Candidate terms below are NOT resolved yet"));
        assert!(prompt.contains("canonical spelling only when the phrase is phonetically close"));
        assert!(prompt.contains("n8n [code identifier]"));
    }

    #[test]
    fn task_block_ends_with_single_output_enforcement() {
        // FOUNDATIONAL: the very last instruction the LLM sees before
        // generation must be the single-output enforcement. End-of-prompt
        // attention is strongest; placing the rule earlier (as a bullet
        // in the middle of a numbered list) wasn't holding up against
        // verbose vocab-block changes that pushed the LLM into
        // multiple-output mode. Locking placement here is the regression
        // test for the duplicate-polish bug.
        let p = prefs();
        let prompt = build_system_prompt_with_vocab(&p, &[], &[], &[]);

        // Must contain the final rule heading (compact form).
        assert!(
            prompt.contains("FINAL RULE — SINGLE OUTPUT"),
            "single-output rule must be present"
        );
        // Must explicitly forbid repetition (the failure mode).
        assert!(
            prompt.contains("No repetition") || prompt.contains("no repetition"),
            "single-output rule must explicitly forbid repetition"
        );
        // The FINAL rule must come AFTER all other task rules — verify by
        // checking position relative to a known earlier rule.
        let pos_output_only = prompt.find("Output ONLY the polished text").unwrap();
        let pos_final = prompt.find("FINAL RULE").unwrap();
        assert!(
            pos_final > pos_output_only,
            "FINAL RULE must come AFTER the output-only rule (be the LAST instruction)"
        );
        // The final rule must be near the </task> closer for end-of-prompt
        // attention to fire on it.
        let pos_close = prompt.find("</task>").unwrap();
        assert!(
            pos_close - pos_final < 1500,
            "FINAL RULE must be near </task> closer ({}+ chars away — should be < 1500)",
            pos_close - pos_final
        );
    }

    #[test]
    fn vocab_block_renders_type_tag_per_entry() {
        let p = prefs();
        let entries = vec![
            VocabEntry {
                term: "MACOBS".into(),
                context: Some("MACOBS ka IPO".into()),
                resolution: VocabResolution::Candidate,
                term_type: Some("acronym".into()),
                meaning: None,
            },
            VocabEntry {
                term: "Anish".into(),
                context: None,
                resolution: VocabResolution::Candidate,
                term_type: Some("proper_noun".into()),
                meaning: None,
            },
            VocabEntry {
                term: "n8n".into(),
                context: Some("I run n8n".into()),
                resolution: VocabResolution::Candidate,
                term_type: Some("code_identifier".into()),
                meaning: None,
            },
            VocabEntry {
                term: "ClaudeCode".into(),
                context: None,
                resolution: VocabResolution::Candidate,
                term_type: Some("brand".into()),
                meaning: None,
            },
            VocabEntry {
                term: "Cloud Code".into(),
                context: None,
                resolution: VocabResolution::Candidate,
                term_type: Some("phrase".into()),
                meaning: None,
            },
            VocabEntry {
                term: "weird".into(),
                context: None,
                resolution: VocabResolution::Candidate,
                term_type: Some("other".into()),
                meaning: None,
            },
        ];
        let prompt = build_system_prompt_with_vocab_entries(&p, &[], &[], &entries);
        // Multi-line entry shape: "  TERM [type]\n    example: \"...\""
        assert!(prompt.contains("MACOBS [acronym]"));
        assert!(prompt.contains("example: \"MACOBS ka IPO\""));
        assert!(prompt.contains("Anish [proper noun]"));
        assert!(prompt.contains("n8n [code identifier]"));
        assert!(prompt.contains("example: \"I run n8n\""));
        assert!(prompt.contains("ClaudeCode [brand]"));
        assert!(prompt.contains("Cloud Code [phrase]"));
        // "other" type means no signal — render bare without a tag.
        assert!(prompt.contains("  weird\n"));
        assert!(!prompt.contains("weird [other]"));
    }

    #[test]
    fn vocab_entries_with_context_render_inline() {
        // Backward-compat for the earlier context-only test. Type tag is
        // omitted when entry.term_type is None — the LLM still has the
        // example signal to work with.
        let p = prefs();
        let entries = vec![
            VocabEntry {
                term: "MACOBS".into(),
                context: Some("MACOBS ka IPO ka 12 hazaar batana".into()),
                resolution: VocabResolution::Candidate,
                term_type: None,
                meaning: None,
            },
            VocabEntry {
                term: "n8n".into(),
                context: None,
                resolution: VocabResolution::Candidate,
                term_type: None,
                meaning: None,
            },
        ];
        let prompt = build_system_prompt_with_vocab_entries(&p, &[], &[], &entries);
        // No type tag, with context — `  TERM\n    example: "..."`
        assert!(
            prompt.contains("  MACOBS\n    example: \"MACOBS ka IPO ka 12 hazaar batana\""),
            "entry without type tag should still render context on its own line"
        );
        assert!(
            prompt.contains("  n8n\n"),
            "bare entry should render just the term"
        );
    }

    #[test]
    fn vocab_entry_renders_meaning_line_when_present() {
        // Foundational: when the term has a stored meaning, the polish prompt
        // must surface it as a `means:` line so the LLM can do semantic
        // alignment between the transcript context and the term's distilled
        // description. This is the third matching layer (alongside lexical
        // gate + type signal) — without it we'd be back to inferring meaning
        // from one example each call.
        let p = prefs();
        let entries = vec![VocabEntry {
            term: "MACOBS".into(),
            context: Some("MACOBS ka IPO".into()),
            resolution: VocabResolution::Candidate,
            term_type: Some("acronym".into()),
            meaning: Some("Indian SME stock acronym used in market-cap discussions.".into()),
        }];
        let prompt = build_system_prompt_with_vocab_entries(&p, &[], &[], &entries);
        assert!(
            prompt.contains("MACOBS [acronym]"),
            "term + type tag still render"
        );
        assert!(
            prompt.contains("means: Indian SME stock acronym used in market-cap discussions."),
            "meaning surfaces as a `means:` line",
        );
        assert!(
            prompt.contains("example: \"MACOBS ka IPO\""),
            "example still renders alongside meaning"
        );
        // The block-level instruction must mention semantic alignment, not
        // just type compatibility — that's the upgrade.
        assert!(
            prompt.contains("means:"),
            "vocab block instructions reference the means: layer"
        );
    }

    #[test]
    fn vocab_entry_omits_meaning_when_absent() {
        // When meaning is None the entry must still render cleanly — the
        // `means:` line is suppressed (we never emit `means:` followed by
        // empty content) and the rest of the entry is unchanged.
        let p = prefs();
        let entries = vec![VocabEntry {
            term: "Anish".into(),
            context: None,
            resolution: VocabResolution::Candidate,
            term_type: Some("proper_noun".into()),
            meaning: None,
        }];
        let prompt = build_system_prompt_with_vocab_entries(&p, &[], &[], &entries);
        assert!(prompt.contains("Anish [proper noun]"));
        // No phantom `means:` line for entries without one.
        let count_means = prompt.matches("means:").count();
        // The block-level instructions reference `means:` exactly twice (the
        // structural rule) — but no per-entry rendering.
        assert!(
            count_means <= 3,
            "no per-entry `means:` line should be emitted when meaning is None ({count_means} found)"
        );
    }

    #[test]
    fn hinglish_prompt_explicitly_blocks_translation_to_english() {
        // FOUNDATIONAL: ~2/10 of Hinglish polish runs were dropping Hindi
        // entirely and emitting pure English ("aaj bahut kaam tha" →
        // "Today there was a lot of work"). The original rule only forbade
        // Devanagari, which pure English satisfies — so the LLM thought it
        // was complying. The fix adds explicit "preserve Hindi, do not
        // translate" language at three positions: language_rule (top of
        // system prompt), script_final_check (last thing in <task>), and
        // build_user_message reminder (right before the transcript).
        //
        // This test pins those three positions so a future "shorten the
        // prompt" refactor can't quietly remove them.
        let mut p = prefs();
        p.output_language = "hinglish".into();

        let sys = build_system_prompt_with_vocab(&p, &[], &[], &[]);
        // language_rule must contain anti-translation language.
        assert!(
            sys.contains("PRESERVATION"),
            "Hinglish language_rule must have a PRESERVATION block"
        );
        assert!(
            sys.contains("Do NOT translate them to English")
                || sys.contains("never translate Hindi"),
            "Hinglish language_rule must explicitly forbid Hindi→English translation"
        );
        // Concrete failure-mode example must be present (the LLM learns from
        // ✗/✓ pairs much more reliably than from abstract rules).
        assert!(
            sys.contains("Today there was a lot of work"),
            "Hinglish language_rule must include the canonical failure-mode example"
        );

        // script_final_check must call out language preservation, not just script.
        assert!(
            sys.contains("LANGUAGE —") || sys.contains("LANGUAGE BALANCE"),
            "script_final_check must include a LANGUAGE/preservation check"
        );
        assert!(
            sys.contains("kaam") && sys.contains("never \"work\""),
            "script_final_check must show kaam→work as the canonical wrong move"
        );

        // user_message reminder must mention preservation.
        let user = build_user_message("aaj bahut kaam tha", "hinglish");
        assert!(
            user.contains("preserve Hindi") || user.contains("never translate"),
            "user_message reminder must mention Hindi preservation"
        );
    }

    #[test]
    fn polish_corrections_block_is_soft_not_mandatory() {
        let p = prefs();
        let corr = vec![Correction {
            wrong: "kindly".into(),
            right: "please".into(),
            count: 1,
        }];
        let prompt = build_system_prompt_with_vocab(&p, &[], &corr, &[]);
        assert!(prompt.contains("<polish_preferences>"));
        // The old MANDATORY language must be gone — that was the semantic bug.
        assert!(!prompt.contains("MANDATORY"));
        assert!(!prompt.contains("No exceptions"));
    }

    #[test]
    fn rag_examples_are_soft_and_cannot_drop_transcript_words() {
        let p = prefs();
        let rag = vec![RagExample {
            ai_output: "Please check the deployment logs.".into(),
            user_kept: "Check deploy logs.".into(),
        }];
        let prompt = build_system_prompt_with_vocab(&p, &rag, &[], &[]);
        assert!(prompt.contains("<preferences>"));
        assert!(prompt.contains("SOFT examples"));
        assert!(prompt.contains("transcript below is the source of truth"));
        assert!(prompt.contains("Never import words"));
        assert!(prompt.contains("never drop current transcript words"));
        assert!(!prompt.contains("carry the same style and word choices"));
    }
}

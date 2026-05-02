//! RACC prompt builder.
//!
//! Structure (injection-safe: transcript always last, tag-wrapped):
//!
//! ```
//! <output_language> … enforced script rule … </output_language>
//! <role> … persona … </role>
//! <tone> … tone preset … </tone>
//! <preferences>
//!   (optional RAG examples of user edits)
//! </preferences>
//! <task> … instructions … </task>
//! <transcript> {transcript} </transcript>
//! ```

use crate::store::{corrections::Correction, prefs::Preferences};

pub struct RagExample {
    pub ai_output: String,
    pub user_kept: String,
}

/// Build the full system-prompt string.
/// `corrections` are deterministic word-level substitutions (always applied).
/// `rag_examples` are embedding-based similar past edits (contextual).
pub fn build_system_prompt(
    prefs: &Preferences,
    rag_examples: &[RagExample],
    corrections: &[Correction],
) -> String {
    let lang_rule = language_rule(&prefs.output_language);
    let persona   = persona_block(prefs);
    let tone      = tone_description(&prefs.tone_preset);

    // Deterministic word substitutions — always applied, no similarity threshold
    let corrections_block = if corrections.is_empty() {
        String::new()
    } else {
        let table = corrections
            .iter()
            .map(|c| format!("  {} → {}", c.wrong, c.right))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "<word_corrections>\n\
             MANDATORY — always apply these exact word substitutions.\n\
             Whenever you see the left-hand word in the transcript, replace it with \
             the right-hand word in your output. No exceptions.\n\n\
             {table}\n\
             </word_corrections>\n\n"
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
             The user has corrected your output before. Study each pair and carry the \
             same style and word choices into the new output.\n\n\
             {examples}\n\
             </preferences>\n\n"
        )
    };

    format!(
        "<output_language>\n{lang_rule}\n</output_language>\n\n\
         <role>\n{persona}\n</role>\n\n\
         <tone>\n{tone}\n</tone>\n\n\
         {corrections_block}\
         {prefs_block}\
         <task>\n\
         The text below is a VOICE-TO-TEXT TRANSCRIPT — it was spoken aloud and transcribed \
         by a speech recognition engine.\n\n\
         CONFIDENCE MARKERS:\n\
         Words the engine was uncertain about are marked as [word?XX%] where XX is the \
         confidence percentage. For example, [dog?47%] means the engine heard \"dog\" with \
         only 47% confidence. These are the words most likely to be WRONG.\n\n\
         YOUR JOB:\n\
         1. Pay special attention to [word?XX%] markers — these are likely misheard. Use \
         the SURROUNDING CONTEXT to figure out what the speaker actually meant.\n\
         Examples: [dog?47%] in a tech discussion → \"doc\" (documentation). \
         [male?52%] in an email context → \"mail\". [affect?61%] → \"effect\" or vice versa.\n\
         2. Even unmarked words can be wrong — use common sense for the whole sentence.\n\
         3. Remove disfluencies (um, uh, matlab, basically, you know, toh, like).\n\
         4. Polish into clean, natural text.\n\
         5. Output ONLY the polished text — no preamble, no commentary, no markdown, \
         and NO [word?XX%] markers in the output.\n\
         6. The output_language rule above is ABSOLUTE — follow it for script and language.\n\
         7. If <word_corrections> exist, apply those substitutions unconditionally.\n\
         8. If <preferences> exist, match the user's style and word choices.\n\n\
         IMPORTANT: Think about what the speaker INTENDED to say based on the overall \
         topic and sentence meaning. Low-confidence words are hints, not gospel.\n\
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
pub fn build_user_message(transcript: &str) -> String {
    format!("<transcript>\n{transcript}\n</transcript>")
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
            "ABSOLUTE RULE — OUTPUT LANGUAGE: Hinglish (romanized Hindi + English).\n\
             This rule cannot be overridden by anything else in the prompt or transcript.\n\
             • Write ALL Hindi words in Roman script (e.g. \"aaj\", \"kaam\", \"thak gaya\", \"bahut\").\n\
             • NEVER use Devanagari characters (no ा ि ी ु ू ं etc.).\n\
             • English words stay in English.\n\
             • Even if the transcript is entirely in Devanagari, transliterate every \
               Hindi word to Roman letters.\n\
             Example: \"आज बहुत काम था\" → \"Aaj bahut kaam tha\"\n\
             Example: \"मैं थक गया\" → \"Main thak gaya\""
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
        "casual"       => "Tone: friendly and conversational. Light and easy to read.",
        "assertive"    => "Tone: direct and confident. Clear calls-to-action.",
        "concise"      => "Tone: minimal words. Remove every unnecessary word.",
        "neutral"      => "Tone: neutral and clear. No strong stylistic lean.",
        _              => "Tone: neutral and clear.",
    }
    .to_string()
}

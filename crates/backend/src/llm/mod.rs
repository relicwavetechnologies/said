pub mod classifier;
pub mod edit_diff;
pub mod gateway;
pub mod gemini_direct;
pub mod groq;
pub mod openai_codex;
pub mod phonetics;
pub mod pre_filter;
pub mod promotion_gate;
pub mod prompt;

/// Shared result type returned by all LLM streaming clients.
pub struct PolishResult {
    pub polished:  String,
    pub polish_ms: u64,
}

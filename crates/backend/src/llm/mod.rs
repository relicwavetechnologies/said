pub mod gateway;
pub mod gemini_direct;
pub mod groq;
pub mod openai_codex;
pub mod prompt;

/// Shared result type returned by all LLM streaming clients.
pub struct PolishResult {
    pub polished:  String,
    pub polish_ms: u64,
}

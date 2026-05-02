-- Migration 010: Add groq_api_key column to preferences
-- Enables Groq as a standalone LLM provider (OpenAI-compatible API).
ALTER TABLE preferences ADD COLUMN groq_api_key TEXT;

-- Migration 005: Add llm_provider column to preferences
-- Allowed values: 'gateway' | 'gemini_direct'
ALTER TABLE preferences ADD COLUMN llm_provider TEXT NOT NULL DEFAULT 'gateway';

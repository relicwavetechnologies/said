-- Migration 018 — vocabulary.meaning: LLM-distilled semantic descriptions per term.
--
-- Foundational addition: each vocab entry gains a stored MEANING (1-2 sentences)
-- describing what the term refers to and the contexts it applies in. Generated
-- by Groq from observed example sentences and refined as more examples
-- accumulate. The polish prompt then includes the meaning per entry, enabling
-- the LLM to do SEMANTIC alignment (does the transcript context match the
-- term's meaning?) instead of inferring meaning from one example each call.
--
-- This is the third matching layer alongside:
--   1. Lexical gate (BM25)        — cheap inclusion filter
--   2. Type signal                — structural compatibility (acronym/proper-noun/etc)
--   3. Semantic alignment (NEW)   — meaning-vs-context match (this layer)
--
-- All three together: skip irrelevant entries cheaply, enforce structural
-- compatibility, then let the LLM make the smart final decision with full
-- semantic context.
--
-- Refresh policy:
--   • On first promotion → generate meaning from the 1 example we have.
--   • After K=3 new examples accumulate → regenerate from all examples.
--   • Tracked via examples_since_meaning_update counter.
--
-- Schema additions to vocabulary:
ALTER TABLE vocabulary ADD COLUMN meaning TEXT;
-- 1-2 sentence LLM-distilled description; NULL = not generated yet.

ALTER TABLE vocabulary ADD COLUMN meaning_updated_at INTEGER;
-- Epoch ms of last meaning generation. NULL = never generated.

ALTER TABLE vocabulary ADD COLUMN examples_since_meaning INTEGER NOT NULL DEFAULT 0;
-- Counter of new examples observed since last meaning refresh. When this
-- crosses K=3, the next learning event triggers a meaning regeneration
-- (async, fire-and-forget via Groq).

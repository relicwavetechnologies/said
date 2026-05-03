-- Migration 014 — example context for vocabulary terms.
--
-- Each learned vocabulary term gets an optional `example_context` — the
-- sentence (or short snippet) the term was first observed in. This is
-- the foundational data that lets the polish LLM do context-aware
-- recognition of unseen STT mishearings:
--
--   Without context:
--     vocabulary["MACOBS"] → just a bare token
--     polish LLM sees "main course ka IPO" — no idea this is MACOBS
--
--   With context:
--     vocabulary["MACOBS"] = { example: "MACOBS ka IPO ka 12 hazaar batana" }
--     polish LLM sees "main course ka IPO" → recognises the IPO context
--     matches the example → outputs MACOBS
--
-- Nullable so old rows pre-014 keep working; new rows store the snippet
-- captured at learning time.

ALTER TABLE vocabulary ADD COLUMN example_context TEXT;

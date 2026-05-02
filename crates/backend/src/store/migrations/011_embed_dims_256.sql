-- Migration 011: Switch embedding dimensions from 768 → 256.
--
-- The preference_vectors BLOB and embedding_cache BLOB are incompatible
-- between 768-dim and 256-dim representations, so we clear both tables.
-- preference_vectors will be re-populated naturally as users continue
-- making edits. embedding_cache will be rebuilt on next embed() call.
DELETE FROM preference_vectors;
DELETE FROM embedding_cache;

-- Add output_language preference (hinglish | hindi | english)
ALTER TABLE preferences ADD COLUMN output_language TEXT NOT NULL DEFAULT 'hinglish';

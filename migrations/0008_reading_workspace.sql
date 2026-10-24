-- Existing articles, content and reading/starred state are preserved.
ALTER TABLE entries ADD COLUMN is_archived INTEGER NOT NULL DEFAULT 0 CHECK (is_archived IN (0, 1));
ALTER TABLE feeds ADD COLUMN custom_title TEXT;

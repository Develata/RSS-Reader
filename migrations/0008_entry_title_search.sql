-- Title folded for search (Unicode lowercase, computed by the application: SQLite lower()
-- only folds ASCII). Rows written before this migration are filled in after migrating.
ALTER TABLE entries ADD COLUMN title_search TEXT;

-- Rows still waiting for a folded title, so the backfill check at every start stays cheap.
CREATE INDEX idx_entries_title_search_missing ON entries(id) WHERE title_search IS NULL;

-- An update that changes the title without a new folded title (for example an older version
-- of the app still running against this database) clears the stale value; the application
-- folds it again (the current version right away, otherwise at the next start).
CREATE TRIGGER entries_title_search_stale
AFTER UPDATE OF title ON entries
WHEN NEW.title IS NOT OLD.title AND NEW.title_search IS OLD.title_search
BEGIN
    UPDATE entries SET title_search = NULL WHERE id = NEW.id;
END;

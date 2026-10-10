-- Title folded for search (Unicode lowercase, computed by the application: SQLite lower()
-- only folds ASCII). Rows written before this migration are filled in after migrating.
ALTER TABLE entries ADD COLUMN title_search TEXT;

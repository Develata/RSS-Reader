-- v0.2.0's baseline is immutable. Existing articles remain unarchived on upgrade.
ALTER TABLE entries ADD COLUMN is_archived INTEGER NOT NULL DEFAULT 0
    CHECK (is_archived IN (0, 1));

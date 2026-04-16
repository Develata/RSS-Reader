CREATE TABLE IF NOT EXISTS entry_contents (
    entry_id INTEGER PRIMARY KEY,
    feed_id INTEGER NOT NULL,
    content_html TEXT,
    content_text TEXT,
    content_hash TEXT,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_entry_contents_feed_id
    ON entry_contents(feed_id);

CREATE INDEX IF NOT EXISTS idx_entry_contents_updated_at
    ON entry_contents(updated_at DESC);

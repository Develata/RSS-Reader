PRAGMA foreign_keys=OFF;

ALTER TABLE entries RENAME TO entries_old;

CREATE TABLE entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    feed_id INTEGER NOT NULL,
    external_id TEXT NOT NULL,
    dedup_key TEXT NOT NULL,
    url TEXT,
    title TEXT NOT NULL,
    author TEXT,
    summary TEXT,
    published_at TEXT,
    updated_at_source TEXT,
    first_seen_at TEXT NOT NULL,
    has_content INTEGER NOT NULL DEFAULT 0,
    is_read INTEGER NOT NULL DEFAULT 0,
    is_starred INTEGER NOT NULL DEFAULT 0,
    read_at TEXT,
    starred_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(feed_id) REFERENCES feeds(id)
);

INSERT INTO entries (
    id, feed_id, external_id, dedup_key, url, title, author, summary, published_at,
    updated_at_source, first_seen_at, has_content, is_read, is_starred, read_at,
    starred_at, created_at, updated_at
)
SELECT
    id,
    feed_id,
    external_id,
    dedup_key,
    url,
    title,
    author,
    summary,
    published_at,
    updated_at_source,
    first_seen_at,
    CASE
        WHEN content_html IS NOT NULL OR content_text IS NOT NULL THEN 1
        ELSE 0
    END,
    is_read,
    is_starred,
    read_at,
    starred_at,
    created_at,
    updated_at
FROM entries_old;

DROP TABLE entries_old;

CREATE UNIQUE INDEX idx_entries_feed_external
    ON entries(feed_id, external_id);

CREATE UNIQUE INDEX idx_entries_feed_dedup
    ON entries(feed_id, dedup_key);

CREATE INDEX idx_entries_feed_published
    ON entries(feed_id, published_at DESC);

CREATE INDEX idx_entries_published
    ON entries(published_at DESC);

CREATE INDEX idx_entries_is_read_published
    ON entries(is_read, published_at DESC);

CREATE INDEX idx_entries_is_starred_published
    ON entries(is_starred, published_at DESC);

CREATE INDEX idx_entries_title
    ON entries(title);

PRAGMA foreign_keys=ON;

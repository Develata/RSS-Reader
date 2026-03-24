CREATE TABLE IF NOT EXISTS feeds (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    url TEXT NOT NULL UNIQUE,
    title TEXT,
    site_url TEXT,
    description TEXT,
    icon_url TEXT,
    etag TEXT,
    last_modified TEXT,
    last_fetched_at TEXT,
    last_success_at TEXT,
    fetch_error TEXT,
    is_deleted INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_feeds_updated_at ON feeds(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_feeds_is_deleted ON feeds(is_deleted);

CREATE TABLE IF NOT EXISTS entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    feed_id INTEGER NOT NULL,
    external_id TEXT NOT NULL,
    dedup_key TEXT NOT NULL,
    url TEXT,
    title TEXT NOT NULL,
    author TEXT,
    summary TEXT,
    content_html TEXT,
    content_text TEXT,
    published_at TEXT,
    updated_at_source TEXT,
    first_seen_at TEXT NOT NULL,
    content_hash TEXT,
    is_read INTEGER NOT NULL DEFAULT 0,
    is_starred INTEGER NOT NULL DEFAULT 0,
    read_at TEXT,
    starred_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(feed_id) REFERENCES feeds(id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_entries_feed_external
    ON entries(feed_id, external_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_entries_feed_dedup
    ON entries(feed_id, dedup_key);

CREATE INDEX IF NOT EXISTS idx_entries_feed_published
    ON entries(feed_id, published_at DESC);

CREATE INDEX IF NOT EXISTS idx_entries_published
    ON entries(published_at DESC);

CREATE INDEX IF NOT EXISTS idx_entries_is_read_published
    ON entries(is_read, published_at DESC);

CREATE INDEX IF NOT EXISTS idx_entries_is_starred_published
    ON entries(is_starred, published_at DESC);

CREATE INDEX IF NOT EXISTS idx_entries_title
    ON entries(title);

CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sync_sources (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    endpoint TEXT,
    username TEXT,
    remote_path TEXT,
    last_synced_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

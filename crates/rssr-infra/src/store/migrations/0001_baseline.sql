-- v0.2 library baseline: one file holds subscriptions, entries, bodies, settings and app state.
--
-- Derived from v0.1.x index migrations 0001-0009 plus content migration 0001, with the
-- changes decided for v0.2 (docs/design/v0.2-architecture.md):
--   * `entries.title_search` is NOT NULL; the stale-title trigger, the partial index and the
--     start-up backfill that only served old writers are gone.
--   * `entry_contents` lives next to `entries` and is removed with its entry (foreign key with
--     ON DELETE CASCADE), so the cross-database GC queue and its `feed_id` copy are gone.
--   * `feeds.refresh_*` hold the persistent refresh tickets I/P/D (0 <= P <= D <= I).
--   * the unused `sync_sources` table and `idx_entry_contents_updated_at` are not carried over.
-- Until v0.2.0 is released this file may still change; afterwards it is frozen.

CREATE TABLE feeds (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    url TEXT NOT NULL UNIQUE,
    -- Name chosen by the user or carried by an import; readers show
    -- COALESCE(custom_title, title, url).
    custom_title TEXT,
    -- Title the feed itself declares; refreshes update it.
    title TEXT,
    site_url TEXT,
    description TEXT,
    icon_url TEXT,
    folder TEXT,
    etag TEXT,
    last_modified TEXT,
    last_fetched_at TEXT,
    last_success_at TEXT,
    fetch_error TEXT,
    -- Where fetch_error came from: fetch, parse or storage; NULL for a diagnostic imported
    -- from v0.1.x (shown generically).
    fetch_error_kind TEXT CHECK (fetch_error_kind IN ('fetch', 'parse', 'storage')),
    is_deleted INTEGER NOT NULL DEFAULT 0,
    -- Bumped when a removed subscription is added again, so work started for the old
    -- subscription cannot write into the new one.
    generation INTEGER NOT NULL DEFAULT 0,
    -- Refresh tickets: issued (I), published (P), diagnosed (D).
    refresh_issued INTEGER NOT NULL DEFAULT 0,
    refresh_published INTEGER NOT NULL DEFAULT 0,
    refresh_diagnosed INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK (
        0 <= refresh_published
        AND refresh_published <= refresh_diagnosed
        AND refresh_diagnosed <= refresh_issued
    )
);

CREATE INDEX idx_feeds_updated_at ON feeds(updated_at DESC);
CREATE INDEX idx_feeds_is_deleted ON feeds(is_deleted);

CREATE TABLE entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    feed_id INTEGER NOT NULL REFERENCES feeds(id),
    external_id TEXT NOT NULL,
    dedup_key TEXT NOT NULL,
    url TEXT,
    title TEXT NOT NULL,
    -- `title` folded for search (`rssr_domain::fold_for_search`); written with every title.
    title_search TEXT NOT NULL,
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
    updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX idx_entries_feed_external ON entries(feed_id, external_id);
CREATE UNIQUE INDEX idx_entries_feed_dedup ON entries(feed_id, dedup_key);
CREATE INDEX idx_entries_feed_published ON entries(feed_id, published_at DESC);
CREATE INDEX idx_entries_published ON entries(published_at DESC);
CREATE INDEX idx_entries_is_read_published ON entries(is_read, published_at DESC);
CREATE INDEX idx_entries_is_starred_published ON entries(is_starred, published_at DESC);
CREATE INDEX idx_entries_title ON entries(title);
-- Lists and reader navigation sort by COALESCE(published_at, created_at) DESC, id DESC
-- (see v0.1.x migrations 0003-0005 for the query plans behind these three).
CREATE INDEX idx_entries_sort_key
    ON entries(COALESCE(published_at, created_at) DESC, id DESC);
CREATE INDEX idx_entries_feed_sort_key
    ON entries(feed_id, COALESCE(published_at, created_at) DESC, id DESC);
CREATE INDEX idx_entries_unread_sort_key
    ON entries(is_read, COALESCE(published_at, created_at) DESC, id DESC);

-- Bodies stay out of `entries` so list queries never read them.
CREATE TABLE entry_contents (
    entry_id INTEGER PRIMARY KEY REFERENCES entries(id) ON DELETE CASCADE,
    content_html TEXT,
    content_text TEXT,
    -- Identifies the source body; a localized rewrite keeps it.
    content_hash TEXT,
    updated_at TEXT NOT NULL
);

-- Key/value documents: user settings and app state.
CREATE TABLE app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

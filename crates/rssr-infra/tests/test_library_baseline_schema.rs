#![cfg(not(target_arch = "wasm32"))]

//! The v0.2 baseline equals the v0.1.x schema (index migrations 0001-0009 plus content
//! migration 0001) except for the changes listed here, which are the ones decided in
//! docs/design/v0.2-architecture.md. The v0.1.x schema is built from the frozen migration files.

#[path = "support/v01_schema.rs"]
mod v01_schema;

use std::collections::BTreeSet;

use rssr_infra::store::LibraryStore;
use rusqlite::Connection;

const COLUMNS: &str = "
    SELECT m.name || '.' || p.name || ' ' || p.type || ' notnull=' || p.\"notnull\"
        || ' default=' || COALESCE(p.dflt_value, '-') || ' pk=' || p.pk
    FROM sqlite_schema AS m JOIN pragma_table_info(m.name) AS p
    WHERE m.type = 'table' AND m.name NOT LIKE 'sqlite_%'
      AND m.name NOT IN ('_sqlx_migrations', 'schema_migrations')";

const FOREIGN_KEYS: &str = "
    SELECT m.name || '.' || f.\"from\" || ' -> ' || f.\"table\" || '.' || COALESCE(f.\"to\", '?')
        || ' on_delete=' || f.on_delete
    FROM sqlite_schema AS m JOIN pragma_foreign_key_list(m.name) AS f
    WHERE m.type = 'table'";

const INDEXES: &str = "
    SELECT type || ' ' || name || ' ON ' || tbl_name FROM sqlite_schema
    WHERE type IN ('index', 'trigger') AND sql IS NOT NULL";

/// Index SQL normalized for comparison (whitespace and `IF NOT EXISTS` do not matter).
const INDEX_SQL: &str = "
    SELECT name, sql FROM sqlite_schema WHERE type = 'index' AND sql IS NOT NULL";

fn v01_index() -> Connection {
    let index = Connection::open_in_memory().unwrap();
    v01_schema::apply_index(&index, v01_schema::INDEX.len());
    index
}

async fn v01_schema(query: &str) -> BTreeSet<String> {
    let content = Connection::open_in_memory().unwrap();
    v01_schema::apply_content(&content);
    let mut rows = BTreeSet::new();
    for conn in [&v01_index(), &content] {
        let mut statement = conn.prepare(query).unwrap();
        for row in statement.query_map([], |row| row.get::<_, String>(0)).unwrap() {
            rows.insert(row.unwrap());
        }
    }
    rows
}

async fn baseline_schema(query: &'static str) -> BTreeSet<String> {
    let store = LibraryStore::open_in_memory().await.unwrap();
    store
        .read(move |conn| {
            let mut statement = conn.prepare(query)?;
            Ok(statement.query_map([], |row| row.get(0))?.collect::<Result<_, _>>()?)
        })
        .await
        .unwrap()
}

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|item| item.to_string()).collect()
}

fn assert_delta(
    v01: &BTreeSet<String>,
    baseline: &BTreeSet<String>,
    removed: &[&str],
    added: &[&str],
) {
    let actually_removed: BTreeSet<_> = v01.difference(baseline).cloned().collect();
    let actually_added: BTreeSet<_> = baseline.difference(v01).cloned().collect();
    assert_eq!(actually_removed, set(removed), "only in v0.1.x");
    assert_eq!(actually_added, set(added), "only in the baseline");
}

#[tokio::test(flavor = "multi_thread")]
async fn columns_differ_only_by_the_decided_changes() {
    assert_delta(
        &v01_schema(COLUMNS).await,
        &baseline_schema(COLUMNS).await,
        &[
            "content_gc_queue.entry_id INTEGER notnull=0 default=- pk=1",
            "content_gc_queue.queued_at TEXT notnull=1 default=- pk=0",
            "content_gc_state.id INTEGER notnull=0 default=- pk=1",
            "content_gc_state.legacy_reconciled INTEGER notnull=1 default=0 pk=0",
            "entries.title_search TEXT notnull=0 default=- pk=0",
            "entry_contents.feed_id INTEGER notnull=1 default=- pk=0",
            "sync_sources.created_at TEXT notnull=1 default=- pk=0",
            "sync_sources.endpoint TEXT notnull=0 default=- pk=0",
            "sync_sources.id INTEGER notnull=0 default=- pk=1",
            "sync_sources.kind TEXT notnull=1 default=- pk=0",
            "sync_sources.last_synced_at TEXT notnull=0 default=- pk=0",
            "sync_sources.remote_path TEXT notnull=0 default=- pk=0",
            "sync_sources.updated_at TEXT notnull=1 default=- pk=0",
            "sync_sources.username TEXT notnull=0 default=- pk=0",
        ],
        &[
            "entries.title_search TEXT notnull=1 default=- pk=0",
            "feeds.refresh_diagnosed INTEGER notnull=1 default=0 pk=0",
            "feeds.refresh_issued INTEGER notnull=1 default=0 pk=0",
            "feeds.refresh_published INTEGER notnull=1 default=0 pk=0",
        ],
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn foreign_keys_differ_only_by_the_body_cascade() {
    assert_delta(
        &v01_schema(FOREIGN_KEYS).await,
        &baseline_schema(FOREIGN_KEYS).await,
        &[],
        &["entry_contents.entry_id -> entries.id on_delete=CASCADE"],
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn indexes_and_triggers_differ_only_by_the_decided_changes() {
    assert_delta(
        &v01_schema(INDEXES).await,
        &baseline_schema(INDEXES).await,
        &[
            "index idx_entries_title_search_missing ON entries",
            "index idx_entry_contents_feed_id ON entry_contents",
            "index idx_entry_contents_updated_at ON entry_contents",
            "trigger entries_title_search_stale ON entries",
        ],
        &[],
    );
}

/// Indexes kept from v0.1.x cover the same keys.
#[tokio::test(flavor = "multi_thread")]
async fn kept_indexes_have_the_same_definition() {
    fn normalize(sql: &str) -> String {
        sql.replace("IF NOT EXISTS ", "").split_whitespace().collect::<Vec<_>>().join(" ")
    }
    let index = v01_index();
    let v01: Vec<(String, String)> = index
        .prepare(INDEX_SQL)
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let baseline: Vec<(String, String)> = LibraryStore::open_in_memory()
        .await
        .unwrap()
        .read(|conn| {
            let mut statement = conn.prepare(INDEX_SQL)?;
            Ok(statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<Result<_, _>>()?)
        })
        .await
        .unwrap();
    let mut compared = 0;
    for (name, sql) in &v01 {
        if let Some((_, kept)) = baseline.iter().find(|(kept, _)| kept == name) {
            assert_eq!(normalize(kept), normalize(sql), "{name}");
            compared += 1;
        }
    }
    assert_eq!(compared, 12, "kept index count: 2 on feeds, 10 on entries");
}

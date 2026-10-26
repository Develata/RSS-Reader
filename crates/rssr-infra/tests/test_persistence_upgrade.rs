#![cfg(not(target_arch = "wasm32"))]

use std::time::{SystemTime, UNIX_EPOCH};

use rssr_domain::{
    AppStateRepository, AppStateSnapshot, ArchiveFilter, EntryBatchAction, EntryBatchRequest,
    EntryIndexRepository, EntryQuery, FeedEditRequest, FeedRepository, ListDensity,
    SettingsRepository, StartupView, ThemeMode, UserSettings,
};
use rssr_infra::db::{
    CONTENT_MIGRATOR, MIGRATOR, app_state_repository::SqliteAppStateRepository,
    entry_repository::SqliteEntryRepository, feed_repository::SqliteFeedRepository,
    settings_repository::SqliteSettingsRepository, sqlite_native::NativeSqliteBackend,
    storage_backend::StorageBackend,
};
use sqlx::Row;

/// Exercise the actual pre-workspace schema, then close every connection between
/// upgrade/reopen phases. An in-memory round trip cannot catch lost on-disk data.
#[tokio::test]
async fn pre_workspace_database_survives_upgrade_and_reopen() {
    for newlines in ["lf", "crlf", "mixed"] {
        upgrade_and_reopen(newlines).await;
    }
}

async fn upgrade_and_reopen(newlines: &str) {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let directory =
        std::env::temp_dir().join(format!("rssr-upgrade-{}-{nonce}", std::process::id()));
    let backend = NativeSqliteBackend::with_path(directory.join("rss-reader.db"));
    let pool = backend.connect().await.unwrap();
    let content_pool = backend.connect_content().await.unwrap();
    let legacy = legacy_migrator(&MIGRATOR, 7, newlines);
    legacy.run(&pool).await.unwrap();
    legacy_migrator(&CONTENT_MIGRATOR, 1, newlines).run(&content_pool).await.unwrap();
    let old_history = migration_history(&pool).await;
    let old_content_history = migration_history(&content_pool).await;
    sqlx::raw_sql(
        r#"
        INSERT INTO feeds (id, url, title, folder, created_at, updated_at)
        VALUES (42, 'https://example.com/old.xml', 'Original source', 'Saved folder',
                '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
        INSERT INTO entries (id, feed_id, external_id, dedup_key, title, first_seen_at,
            has_content, is_read, is_starred, read_at, starred_at, created_at, updated_at)
        VALUES (100, 42, 'old-100', 'old-100', 'Saved article', '2026-01-01T00:00:00Z',
            1, 1, 1, '2026-01-02T00:00:00Z', '2026-01-03T00:00:00Z',
            '2026-01-01T00:00:00Z', '2026-01-03T00:00:00Z');
        INSERT INTO entries (id, feed_id, external_id, dedup_key, title, first_seen_at,
            has_content, is_read, is_starred, created_at, updated_at)
        VALUES (101, 42, 'old-101', 'old-101', 'Unread article', '2026-01-01T00:00:00Z',
            0, 0, 0, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
        INSERT INTO sync_sources (kind, endpoint, username, remote_path, created_at, updated_at)
        VALUES ('webdav', 'https://example.com/dav', 'saved-user', '/rssr.json',
                '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO entry_contents (entry_id, feed_id, content_html, content_text, content_hash, updated_at)
         VALUES (100, 42, '<p>Saved body</p>', 'Saved body', 'saved-hash', '2026-01-01T00:00:00Z')",
    )
    .execute(&content_pool)
    .await
    .unwrap();
    let settings = UserSettings {
        theme: ThemeMode::Dark,
        list_density: ListDensity::Compact,
        startup_view: StartupView::LastFeed,
        refresh_interval_minutes: 45,
        archive_after_months: 6,
        entries_page_size: 50,
        reader_font_scale: 1.2,
        custom_css: "[data-page=\"feeds\"] { opacity: 0.95; }".into(),
    };
    let mut state: AppStateSnapshot = serde_json::from_str(
        r#"{"last_opened_feed_id":42,"entries_workspace":{"grouping_mode":"source",
        "show_archived":true,"read_filter":"read_only","starred_filter":"starred_only",
        "selected_feed_urls":["https://example.com/old.xml"]}}"#,
    )
    .unwrap();
    SqliteSettingsRepository::new(pool.clone()).save(&settings).await.unwrap();
    SqliteAppStateRepository::new(pool.clone()).save(&state).await.unwrap();
    pool.close().await;
    content_pool.close().await;

    let pool = backend.connect().await.unwrap();
    let content_pool = backend.connect_content().await.unwrap();
    backend.migrate(&pool).await.unwrap();
    backend.migrate_content(&content_pool).await.unwrap();
    assert_eq!(migration_history(&pool).await[..old_history.len()], old_history);
    assert_eq!(migration_history(&content_pool).await, old_content_history);
    let entries = SqliteEntryRepository::new_with_content_pool(pool.clone(), content_pool.clone());
    let feeds = SqliteFeedRepository::new(pool.clone());
    let saved_entry = entries.get_entry(100).await.unwrap().unwrap();
    assert!(saved_entry.is_read && saved_entry.is_starred);
    assert_eq!(saved_entry.content_html.as_deref(), Some("<p>Saved body</p>"));
    assert_eq!(saved_entry.content_text.as_deref(), Some("Saved body"));
    assert_eq!(saved_entry.content_hash.as_deref(), Some("saved-hash"));
    assert_eq!(saved_entry.read_at.unwrap().day(), 2);
    assert_eq!(saved_entry.starred_at.unwrap().day(), 3);
    assert_eq!(entries.list_entries(&EntryQuery::default()).await.unwrap().len(), 2);
    assert_eq!(SqliteSettingsRepository::new(pool.clone()).load().await.unwrap(), settings);
    assert_eq!(SqliteAppStateRepository::new(pool.clone()).load().await.unwrap(), state);
    let feed = feeds.get_feed(42).await.unwrap().unwrap();
    assert_eq!(feed.title.as_deref(), Some("Original source"));
    assert_eq!(feed.folder.as_deref(), Some("Saved folder"));
    assert_eq!(feeds.edit_details(42).await.unwrap().custom_title, None);
    let archived: i64 =
        sqlx::query_scalar("SELECT SUM(is_archived) FROM entries").fetch_one(&pool).await.unwrap();
    assert_eq!(archived, 0, "upgrade must not silently archive existing articles");

    feeds
        .update_subscription(&FeedEditRequest {
            feed_id: 42,
            url: "https://example.com/new.xml".into(),
            custom_title: Some("Custom source".into()),
        })
        .await
        .unwrap();
    entries
        .apply_entry_batch(&EntryBatchRequest {
            entry_ids: vec![101],
            action: EntryBatchAction::Archive,
        })
        .await
        .unwrap();
    state.replace_feed_url("https://example.com/old.xml", "https://example.com/new.xml");
    pool.close().await;
    content_pool.close().await;

    let pool = backend.connect().await.unwrap();
    let content_pool = backend.connect_content().await.unwrap();
    backend.migrate(&pool).await.unwrap();
    backend.migrate_content(&content_pool).await.unwrap();
    assert_eq!(migration_history(&pool).await[..old_history.len()], old_history);
    assert_eq!(migration_history(&content_pool).await, old_content_history);
    let entries = SqliteEntryRepository::new_with_content_pool(pool.clone(), content_pool.clone());
    let feeds = SqliteFeedRepository::new(pool.clone());
    assert_eq!(entries.get_entry(100).await.unwrap().unwrap(), saved_entry);
    assert_eq!(SqliteSettingsRepository::new(pool.clone()).load().await.unwrap(), settings);
    assert_eq!(SqliteAppStateRepository::new(pool.clone()).load().await.unwrap(), state);
    let details = feeds.edit_details(42).await.unwrap();
    assert_eq!(details.url, "https://example.com/new.xml");
    assert_eq!(details.original_title.as_deref(), Some("Original source"));
    assert_eq!(details.custom_title.as_deref(), Some("Custom source"));
    let archived = entries
        .list_entries(&EntryQuery {
            archive_filter: ArchiveFilter::OnlyManual,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0].id, 101);
    assert!(!archived[0].is_read && !archived[0].is_starred);
    let sync = sqlx::query("SELECT kind, endpoint, username, remote_path FROM sync_sources")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(sync.get::<String, _>("kind"), "webdav");
    assert_eq!(sync.get::<String, _>("endpoint"), "https://example.com/dav");
    assert_eq!(sync.get::<String, _>("username"), "saved-user");
    assert_eq!(sync.get::<String, _>("remote_path"), "/rssr.json");
    pool.close().await;
    content_pool.close().await;
    // Windows may briefly retain SQLite worker file handles after pool close.
    // Retry only sharing/lock violations; other cleanup errors still fail loudly.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        match std::fs::remove_dir_all(&directory) {
            Ok(()) => break,
            Err(error)
                if cfg!(windows)
                    && matches!(error.raw_os_error(), Some(32 | 33))
                    && std::time::Instant::now() < deadline =>
            {
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
            Err(error) => panic!("failed to remove persistence fixture: {error}"),
        }
    }
}

fn legacy_migrator(
    source: &sqlx::migrate::Migrator,
    last: i64,
    newlines: &str,
) -> sqlx::migrate::Migrator {
    sqlx::migrate::Migrator {
        migrations: source
            .iter()
            .filter(|m| m.version <= last)
            .map(|m| {
                let lf = m.sql.replace("\r\n", "\n");
                let sql = if newlines == "crlf" || (newlines == "mixed" && m.version % 2 == 1) {
                    lf.replace('\n', "\r\n")
                } else {
                    lf
                };
                sqlx::migrate::Migration::new(
                    m.version,
                    m.description.clone(),
                    m.migration_type,
                    sql.into(),
                    m.no_tx,
                )
            })
            .collect(),
        ..sqlx::migrate::Migrator::DEFAULT
    }
}

async fn migration_history(
    pool: &rssr_infra::db::SqlitePool,
) -> Vec<(i64, String, String, bool, Vec<u8>, i64)> {
    sqlx::query_as("SELECT version, description, installed_on, success, checksum, execution_time FROM _sqlx_migrations ORDER BY version")
        .fetch_all(pool).await.unwrap()
}

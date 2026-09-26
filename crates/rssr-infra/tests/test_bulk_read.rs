#![cfg(not(target_arch = "wasm32"))]
#[path = "support/bulk_read_cases.rs"]
mod cases;
use rssr_domain::{
    EntryIndexRepository, EntryQuery, FeedRepository, MarkReadOutcome, NewFeedSubscription,
};
use rssr_infra::{
    db::{
        entry_repository::SqliteEntryRepository, feed_repository::SqliteFeedRepository, migrate,
        sqlite_native::NativeSqliteBackend, storage_backend::StorageBackend,
    },
    parser::feed_parser::ParsedEntry,
};
use url::Url;

#[tokio::test]
async fn sqlite_bulk_scope_confirmation_atomic_failure_and_zero() {
    let backend = NativeSqliteBackend::new("sqlite::memory:");
    let pool = backend.connect().await.unwrap();
    migrate(&pool).await.unwrap();
    let feeds = SqliteFeedRepository::new(pool.clone());
    let entries = SqliteEntryRepository::new(pool.clone());
    for feed_id in [1, 2] {
        feeds
            .upsert_subscription(&NewFeedSubscription {
                url: Url::parse(&format!("https://example.com/{feed_id}")).unwrap(),
                title: None,
                site_url: None,
                folder: None,
            })
            .await
            .unwrap();
    }
    for &(id, feed_id, title, old, starred, read) in cases::ROWS {
        entries
            .upsert_entries(
                feed_id,
                &[ParsedEntry {
                    external_id: id.to_string(),
                    dedup_key: id.to_string(),
                    url: None,
                    title: title.into(),
                    author: None,
                    summary: None,
                    content_html: None,
                    content_text: None,
                    published_at: cases::published(id, old),
                    updated_at_source: None,
                }],
            )
            .await
            .unwrap();
        entries.set_starred(id, starred).await.unwrap();
        entries.set_read(id, read).await.unwrap();
    }
    for (query, expected) in cases::cases() {
        let preview = entries.preview_mark_read(&query).await.unwrap();
        assert_eq!(preview.unread_entry_ids, expected, "{query:?}");
        assert_eq!(preview.query.limit, None);
    }
    let original = entries.preview_mark_read(&EntryQuery::default()).await.unwrap();
    entries.set_read(1, true).await.unwrap();
    entries.set_read(5, false).await.unwrap();
    assert!(matches!(
        entries.mark_read_if_unchanged(&original).await.unwrap(),
        MarkReadOutcome::SelectionChanged { .. }
    ));
    assert!(!entries.get_entry_record(2).await.unwrap().unwrap().is_read);
    let fresh = entries.preview_mark_read(&EntryQuery::default()).await.unwrap();
    sqlx::query("CREATE TRIGGER reject_batch BEFORE UPDATE OF is_read ON entries WHEN NEW.id = 4 BEGIN SELECT RAISE(ABORT, 'test failure'); END").execute(&pool).await.unwrap();
    assert!(entries.mark_read_if_unchanged(&fresh).await.is_err());
    assert_eq!(entries.preview_mark_read(&EntryQuery::default()).await.unwrap(), fresh);
    sqlx::query("DROP TRIGGER reject_batch").execute(&pool).await.unwrap();
    assert_eq!(
        entries.mark_read_if_unchanged(&fresh).await.unwrap(),
        MarkReadOutcome::Applied { changed_count: 5 }
    );
    for id in fresh.unread_entry_ids {
        assert!(entries.get_entry_record(id).await.unwrap().unwrap().read_at.is_some());
    }
    let empty = entries.preview_mark_read(&EntryQuery::default()).await.unwrap();
    assert_eq!(
        entries.mark_read_if_unchanged(&empty).await.unwrap(),
        MarkReadOutcome::Applied { changed_count: 0 }
    );
    assert!(feeds.list_summaries().await.unwrap().iter().all(|f| f.unread_count == 0));
}

#[tokio::test]
async fn bulk_read_large_dataset_measurement() {
    let backend = NativeSqliteBackend::new("sqlite::memory:");
    let pool = backend.connect().await.unwrap();
    migrate(&pool).await.unwrap();
    sqlx::query("INSERT INTO feeds(id,url,created_at,updated_at) VALUES(1,'https://example.com','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')").execute(&pool).await.unwrap();
    sqlx::query("WITH RECURSIVE seq(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM seq WHERE x<50000) INSERT INTO entries(feed_id,external_id,dedup_key,title,first_seen_at,created_at,updated_at) SELECT 1,CAST(x AS TEXT),CAST(x AS TEXT),'Bulk measurement','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z' FROM seq").execute(&pool).await.unwrap();
    let entries = SqliteEntryRepository::new(pool);
    let started = std::time::Instant::now();
    let preview = entries.preview_mark_read(&EntryQuery::default()).await.unwrap();
    let preview_ms = started.elapsed().as_millis();
    let started = std::time::Instant::now();
    assert_eq!(
        entries.mark_read_if_unchanged(&preview).await.unwrap(),
        MarkReadOutcome::Applied { changed_count: 50000 }
    );
    eprintln!(
        "bulk_read rows=50000 preview_ms={preview_ms} apply_ms={}",
        started.elapsed().as_millis()
    );
}

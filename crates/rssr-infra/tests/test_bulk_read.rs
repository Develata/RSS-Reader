#![cfg(not(target_arch = "wasm32"))]
#[path = "support/bulk_read_cases.rs"]
mod cases;
use rssr_domain::{
    EntryIndexRepository, EntryQuery, FeedRepository, MarkReadOutcome, NewFeedSubscription,
};
use sqlx::{QueryBuilder, Row, Sqlite};
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

#[tokio::test]
async fn sqlite_entry_query_accepts_more_feed_ids_than_sqlite_bind_limit() {
    let backend = NativeSqliteBackend::new("sqlite::memory:");
    let pool = backend.connect().await.unwrap();
    migrate(&pool).await.unwrap();

    let feeds = SqliteFeedRepository::new(pool.clone());
    let feed = feeds
        .upsert_subscription(&NewFeedSubscription {
            url: Url::parse("https://example.com/selected").unwrap(),
            title: Some("Selected".into()),
            site_url: None,
            folder: None,
        })
        .await
        .unwrap();
    let entries = SqliteEntryRepository::new(pool);
    entries
        .upsert_entries(
            feed.id,
            &[ParsedEntry {
                external_id: "selected-entry".into(),
                dedup_key: "selected-entry".into(),
                url: None,
                title: "Selected entry".into(),
                author: None,
                summary: None,
                content_html: None,
                content_text: None,
                published_at: None,
                updated_at_source: None,
            }],
        )
        .await
        .unwrap();

    // Modern SQLite commonly caps bound variables at 32766. This scope deliberately exceeds that
    // limit while containing only one real feed id; all repository read/bulk paths must still work.
    let query = EntryQuery { feed_ids: (1_i64..=40_000).collect(), ..EntryQuery::default() };

    assert_eq!(entries.count_entries(&query).await.unwrap(), 1);
    assert_eq!(entries.list_entries(&query).await.unwrap().len(), 1);

    let preview = entries.preview_mark_read(&query).await.unwrap();
    assert_eq!(preview.unread_entry_ids.len(), 1);
    assert_eq!(
        entries.mark_read_if_unchanged(&preview).await.unwrap(),
        MarkReadOutcome::Applied { changed_count: 1 }
    );
}


#[tokio::test]
async fn measure_large_feed_scope_literal_vs_json() {
    let backend = NativeSqliteBackend::new("sqlite::memory:");
    let pool = backend.connect().await.unwrap();
    migrate(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO feeds(id,url,created_at,updated_at) VALUES(1,'https://example.com','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO entries(feed_id,external_id,dedup_key,title,first_seen_at,created_at,updated_at) VALUES(1,'entry','entry','Entry','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let feed_ids = (1_i64..=40_000).collect::<Vec<_>>();
    let probe = serde_json::to_string(&feed_ids).unwrap();
    let json_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM json_each(?1)")
        .bind(&probe)
        .fetch_one(&pool)
        .await
        .expect("bundled SQLite must provide json_each");
    assert_eq!(json_count, 40_000);

    let mut literal_total = std::time::Duration::ZERO;
    let mut json_total = std::time::Duration::ZERO;
    let repetitions = 12_u32;

    for _ in 0..repetitions {
        let started = std::time::Instant::now();
        let mut literal =
            QueryBuilder::<Sqlite>::new("SELECT COUNT(*) AS count FROM entries WHERE feed_id IN (");
        let mut separated = literal.separated(", ");
        for feed_id in &feed_ids {
            separated.push(feed_id.to_string());
        }
        literal.push(")");
        let row = literal
            .build()
            .persistent(false)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(row.get::<i64, _>("count"), 1);
        literal_total += started.elapsed();

        let started = std::time::Instant::now();
        let encoded = serde_json::to_string(&feed_ids).unwrap();
        let row = sqlx::query(
            "SELECT COUNT(*) AS count FROM entries WHERE feed_id IN (SELECT value FROM json_each(?1))",
        )
        .bind(encoded)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.get::<i64, _>("count"), 1);
        json_total += started.elapsed();
    }

    panic!(
        "MEASURE_ONLY repetitions={repetitions} literal_total_us={} json_total_us={} literal_avg_us={} json_avg_us={}",
        literal_total.as_micros(),
        json_total.as_micros(),
        literal_total.as_micros() / repetitions as u128,
        json_total.as_micros() / repetitions as u128,
    );
}

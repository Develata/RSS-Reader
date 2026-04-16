#![cfg(not(target_arch = "wasm32"))]

use std::time::Instant;

use rssr_domain::{EntryQuery, FeedRepository, NewFeedSubscription, ReadFilter, StarredFilter};
use rssr_infra::db::{
    entry_repository::SqliteEntryRepository, feed_repository::SqliteFeedRepository, migrate,
    migrate_content, sqlite_native::NativeSqliteBackend, storage_backend::StorageBackend,
};
use sqlx::QueryBuilder;
use time::{Duration, OffsetDateTime};
use url::Url;

#[tokio::test]
async fn entry_repository_handles_large_dataset_queries() {
    let backend = NativeSqliteBackend::new("sqlite::memory:");
    let index_pool = backend.connect().await.expect("connect sqlite memory");
    migrate(&index_pool).await.expect("run migrations");
    let content_pool = backend.connect_content().await.expect("connect sqlite content memory");
    migrate_content(&content_pool).await.expect("run content migrations");

    let feed_repository = SqliteFeedRepository::new(index_pool.clone());
    let entry_repository =
        SqliteEntryRepository::new_with_content_pool(index_pool.clone(), content_pool.clone());

    let feed = feed_repository
        .upsert_subscription(&NewFeedSubscription {
            url: Url::parse("https://example.com/feed.xml").expect("valid url"),
            title: Some("Perf Feed".to_string()),
            folder: Some("Perf".to_string()),
        })
        .await
        .expect("create feed");

    let now = OffsetDateTime::now_utc();
    for batch in (0..10_000_i64).collect::<Vec<_>>().chunks(250) {
        let mut builder = QueryBuilder::new(
            "INSERT INTO entries (feed_id, external_id, dedup_key, url, title, author, summary, \
             published_at, updated_at_source, first_seen_at, has_content, is_read, is_starred, \
             read_at, starred_at, created_at, updated_at) ",
        );
        builder.push_values(batch.iter().copied(), |mut row, idx| {
            let stamp = (now - Duration::minutes(idx))
                .format(&time::format_description::well_known::Rfc3339)
                .expect("format timestamp");
            row.push_bind(feed.id)
                .push_bind(format!("entry-{idx}"))
                .push_bind(format!("entry-{idx}"))
                .push_bind(format!("https://example.com/articles/{idx}"))
                .push_bind(format!("Performance Article {idx}"))
                .push_bind(Some("Perf Bot"))
                .push_bind(Some(format!("Summary {idx}")))
                .push_bind(Some(stamp.clone()))
                .push_bind(Option::<String>::None)
                .push_bind(stamp.clone())
                .push_bind(1_i64)
                .push_bind(if idx % 3 == 0 { 1_i64 } else { 0_i64 })
                .push_bind(if idx % 5 == 0 { 1_i64 } else { 0_i64 })
                .push_bind(Option::<String>::None)
                .push_bind(Option::<String>::None)
                .push_bind(stamp.clone())
                .push_bind(stamp);
        });
        builder.build().execute(&index_pool).await.expect("seed entry batch");
    }

    for batch in (0..10_000_i64).collect::<Vec<_>>().chunks(250) {
        let mut builder = QueryBuilder::new(
            "INSERT INTO entry_contents (entry_id, feed_id, content_html, content_text, \
             content_hash, updated_at) ",
        );
        builder.push_values(batch.iter().copied(), |mut row, idx| {
            let entry_id = idx + 1;
            let stamp = (now - Duration::minutes(idx))
                .format(&time::format_description::well_known::Rfc3339)
                .expect("format timestamp");
            row.push_bind(entry_id)
                .push_bind(feed.id)
                .push_bind(Some(format!("<p>content {idx}</p>")))
                .push_bind(Some(format!("content {idx}")))
                .push_bind(Some(format!("hash-{idx}")))
                .push_bind(stamp);
        });
        builder.build().execute(&content_pool).await.expect("seed content batch");
    }

    let start = Instant::now();
    let all_entries =
        entry_repository.list_entries(&EntryQuery::default()).await.expect("list entries");
    let list_all_ms = start.elapsed().as_millis();
    assert_eq!(all_entries.len(), 10_000);

    let target_ids: Vec<_> = all_entries.iter().take(10).map(|entry| entry.id).collect();

    let start = Instant::now();
    for id in &target_ids {
        entry_repository.set_read(*id, true).await.expect("set read");
    }
    let toggle_read_ms = start.elapsed().as_millis();

    let start = Instant::now();
    for id in &target_ids {
        entry_repository.set_starred(*id, true).await.expect("set starred");
    }
    let toggle_starred_ms = start.elapsed().as_millis();

    let start = Instant::now();
    let unread_entries = entry_repository
        .list_entries(&EntryQuery { read_filter: ReadFilter::UnreadOnly, ..EntryQuery::default() })
        .await
        .expect("list unread");
    let unread_ms = start.elapsed().as_millis();
    assert!(!unread_entries.is_empty());

    let start = Instant::now();
    let starred_entries = entry_repository
        .list_entries(&EntryQuery {
            starred_filter: StarredFilter::StarredOnly,
            ..EntryQuery::default()
        })
        .await
        .expect("list starred");
    let starred_ms = start.elapsed().as_millis();
    assert!(!starred_entries.is_empty());

    let start = Instant::now();
    let search_hits = entry_repository
        .list_entries(&EntryQuery {
            search_title: Some("Performance Article 99".to_string()),
            ..EntryQuery::default()
        })
        .await
        .expect("search hits");
    let search_hit_ms = start.elapsed().as_millis();
    assert!(!search_hits.is_empty());

    let start = Instant::now();
    let search_miss = entry_repository
        .list_entries(&EntryQuery {
            search_title: Some("no-such-title-token".to_string()),
            ..EntryQuery::default()
        })
        .await
        .expect("search miss");
    let search_miss_ms = start.elapsed().as_millis();
    assert!(search_miss.is_empty());

    println!(
        "PERF_METRICS list_all_ms={list_all_ms} toggle_read_10_ms={toggle_read_ms} \
         toggle_starred_10_ms={toggle_starred_ms} unread_filter_ms={unread_ms} \
         starred_filter_ms={starred_ms} search_hit_ms={search_hit_ms} \
         search_miss_ms={search_miss_ms}"
    );

    assert!(list_all_ms < 2_000, "list all took too long: {list_all_ms}ms");
    assert!(toggle_read_ms < 500, "toggle read took too long: {toggle_read_ms}ms");
    assert!(toggle_starred_ms < 500, "toggle starred took too long: {toggle_starred_ms}ms");
    assert!(unread_ms < 1_500, "unread filter took too long: {unread_ms}ms");
    assert!(starred_ms < 1_500, "starred filter took too long: {starred_ms}ms");
    assert!(search_hit_ms < 1_500, "search hit took too long: {search_hit_ms}ms");
    assert!(search_miss_ms < 1_500, "search miss took too long: {search_miss_ms}ms");
}

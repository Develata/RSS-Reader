#![cfg(not(target_arch = "wasm32"))]

//! List, flag and search timings on a 10_000-entry library (port of
//! `test_entry_large_dataset_performance`). The rows are inserted with raw SQL on purpose, to
//! control `is_read` / `is_starred` / timestamps; with one library file the bodies go into
//! `entry_contents` in the same transaction (no `feed_id` copy any more).

#[path = "support/library.rs"]
mod library;

use std::time::Instant;

use library::Library;
use rssr_domain::{
    EntryContentRepository, EntryIndexRepository, EntryQuery, ReadFilter, StarredFilter,
};
use time::{Duration, OffsetDateTime};

#[tokio::test]
async fn entry_repository_handles_large_dataset_queries() {
    let library = Library::memory().await;
    let feed = library.add_feed("https://example.com/feed.xml").await;
    let feed_id = feed.id;

    let now = OffsetDateTime::now_utc();
    library
        .store
        .write(move |tx| {
            let mut insert_entry = tx.prepare(
                "INSERT INTO entries (feed_id, external_id, dedup_key, url, title, title_search, \
                 author, summary, published_at, updated_at_source, first_seen_at, has_content, \
                 is_read, is_starred, read_at, starred_at, created_at, updated_at) \
                 VALUES (?1, ?2, ?2, ?3, ?4, ?5, 'Perf Bot', ?6, ?7, NULL, ?7, 1, ?8, ?9, NULL, \
                 NULL, ?7, ?7)",
            )?;
            let mut insert_content = tx.prepare(
                "INSERT INTO entry_contents (entry_id, content_html, content_text, \
                 content_hash, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for idx in 0..10_000_i64 {
                let stamp = (now - Duration::minutes(idx))
                    .format(&time::format_description::well_known::Rfc3339)
                    .expect("format timestamp");
                let title = format!("Performance Article {idx}");
                insert_entry.execute(rusqlite::params![
                    feed_id,
                    format!("entry-{idx}"),
                    format!("https://example.com/articles/{idx}"),
                    title,
                    rssr_domain::fold_for_search(&title),
                    format!("Summary {idx}"),
                    stamp,
                    idx % 3 == 0,
                    idx % 5 == 0,
                ])?;
                // Entry ids are assigned in insertion order, starting at 1.
                insert_content.execute(rusqlite::params![
                    idx + 1,
                    format!("<p>content {idx}</p>"),
                    format!("content {idx}"),
                    format!("hash-{idx}"),
                    stamp,
                ])?;
            }
            Ok(())
        })
        .await
        .expect("seed entries and contents");
    assert_eq!(library.scalar("SELECT COUNT(*) FROM entry_contents").await, 10_000);
    let entry_repository = &library.entries;
    // The bodies seeded above are the ones the reader resolves.
    assert_eq!(
        entry_repository.get_content(1).await.unwrap().unwrap().content_text.as_deref(),
        Some("content 0")
    );

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

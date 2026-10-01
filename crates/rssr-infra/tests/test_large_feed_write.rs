#![cfg(not(target_arch = "wasm32"))]

use rssr_domain::{EntryContentRepository, FeedRepository, NewFeedSubscription};
use rssr_infra::{
    db::{
        entry_repository::SqliteEntryRepository, feed_repository::SqliteFeedRepository, migrate,
        migrate_content, sqlite_native::NativeSqliteBackend, storage_backend::StorageBackend,
    },
    parser::ParsedEntry,
};
use time::OffsetDateTime;
use url::Url;

const ENTRY_COUNT: usize = 33_000;

#[tokio::test]
async fn thirty_three_thousand_entry_feed_writes_index_and_content_completely() {
    let backend = NativeSqliteBackend::new("sqlite::memory:");
    let index_pool = backend.connect().await.expect("connect index");
    migrate(&index_pool).await.expect("migrate index");
    let content_pool = backend.connect_content().await.expect("connect content");
    migrate_content(&content_pool).await.expect("migrate content");

    let feeds = SqliteFeedRepository::new(index_pool.clone());
    let entries =
        SqliteEntryRepository::new_with_content_pool(index_pool.clone(), content_pool.clone());
    let feed = feeds
        .upsert_subscription(&NewFeedSubscription {
            site_url: None,
            url: Url::parse("https://example.com/huge.xml").unwrap(),
            title: Some("Huge feed".into()),
            folder: None,
        })
        .await
        .unwrap();

    let parsed = (0..ENTRY_COUNT)
        .map(|index| ParsedEntry {
            external_id: format!("entry-{index}"),
            dedup_key: format!("entry-{index}"),
            url: Some(Url::parse(&format!("https://example.com/articles/{index}")).unwrap()),
            title: format!("Entry {index}"),
            author: None,
            summary: Some(format!("Summary {index}")),
            content_html: Some(format!("<p>Body {index}</p>")),
            content_text: Some(format!("Body {index}")),
            published_at: Some(OffsetDateTime::UNIX_EPOCH),
            updated_at_source: None,
        })
        .collect::<Vec<_>>();

    entries.upsert_entries(feed.id, &parsed).await.expect("write huge feed");

    let index_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM entries WHERE feed_id = ?1")
            .bind(feed.id)
            .fetch_one(&index_pool)
            .await
            .unwrap();
    let content_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM entry_contents WHERE feed_id = ?1")
            .bind(feed.id)
            .fetch_one(&content_pool)
            .await
            .unwrap();
    let marked_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM entries WHERE feed_id = ?1 AND has_content = 1",
    )
    .bind(feed.id)
    .fetch_one(&index_pool)
    .await
    .unwrap();

    assert_eq!(index_count, ENTRY_COUNT as i64);
    assert_eq!(content_count, ENTRY_COUNT as i64);
    assert_eq!(marked_count, ENTRY_COUNT as i64);

    let entry_ids = (1..=ENTRY_COUNT as i64).collect::<Vec<_>>();
    EntryContentRepository::delete_for_entry_ids(&entries, &entry_ids)
        .await
        .expect("chunked content delete");
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM entry_contents")
        .fetch_one(&content_pool)
        .await
        .unwrap();
    assert_eq!(remaining, 0);
}

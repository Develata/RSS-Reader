#![cfg(not(target_arch = "wasm32"))]

use rssr_domain::{FeedRepository, NewFeedSubscription};
use rssr_infra::{
    db::{
        entry_repository::SqliteEntryRepository, feed_repository::SqliteFeedRepository, migrate,
        migrate_content, sqlite_native::NativeSqliteBackend, storage_backend::StorageBackend,
    },
    parser::ParsedEntry,
};
use time::OffsetDateTime;
use url::Url;

const GUID: &str = "0123456789abcdef0123456789abcdef";
const ARTICLE_URL: &str = "https://example.com/article";

fn parsed(external_id: &str, dedup_key: &str, body: &str) -> ParsedEntry {
    ParsedEntry {
        external_id: external_id.into(),
        dedup_key: dedup_key.into(),
        url: Some(Url::parse(ARTICLE_URL).unwrap()),
        title: "Stable article".into(),
        author: None,
        summary: Some(body.into()),
        content_html: Some(format!("<p>{body}</p>")),
        content_text: Some(body.into()),
        published_at: Some(OffsetDateTime::UNIX_EPOCH),
        updated_at_source: None,
    }
}

#[tokio::test]
async fn legacy_url_identity_is_promoted_in_place_when_hex_guid_becomes_visible() {
    let backend = NativeSqliteBackend::new("sqlite::memory:");
    let index_pool = backend.connect().await.unwrap();
    migrate(&index_pool).await.unwrap();
    let content_pool = backend.connect_content().await.unwrap();
    migrate_content(&content_pool).await.unwrap();

    let feeds = SqliteFeedRepository::new(index_pool.clone());
    let entries =
        SqliteEntryRepository::new_with_content_pool(index_pool.clone(), content_pool.clone());
    let feed = feeds
        .upsert_subscription(&NewFeedSubscription {
            site_url: None,
            url: Url::parse("https://example.com/feed.xml").unwrap(),
            title: Some("Example".into()),
            folder: None,
        })
        .await
        .unwrap();

    entries
        .upsert_entries(feed.id, &[parsed(ARTICLE_URL, ARTICLE_URL, "legacy body")])
        .await
        .unwrap();
    let original_id: i64 = sqlx::query_scalar("SELECT id FROM entries WHERE feed_id = ?1")
        .bind(feed.id)
        .fetch_one(&index_pool)
        .await
        .unwrap();
    entries.set_read(original_id, true).await.unwrap();
    entries.set_starred(original_id, true).await.unwrap();

    entries.upsert_entries(feed.id, &[parsed(GUID, GUID, "new body")]).await.unwrap();

    let rows: Vec<(i64, String, String, i64, i64)> = sqlx::query_as(
        "SELECT id, external_id, dedup_key, is_read, is_starred FROM entries WHERE feed_id = ?1",
    )
    .bind(feed.id)
    .fetch_all(&index_pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, original_id);
    assert_eq!(rows[0].1, GUID);
    assert_eq!(rows[0].2, GUID);
    assert_eq!(rows[0].3, 1);
    assert_eq!(rows[0].4, 1);

    let content = entries.get_content(original_id).await.unwrap().unwrap();
    assert_eq!(content.content_text.as_deref(), Some("new body"));
}

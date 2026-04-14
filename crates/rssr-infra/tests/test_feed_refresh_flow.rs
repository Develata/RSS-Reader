#![cfg(not(target_arch = "wasm32"))]

use rssr_domain::{EntryQuery, EntryRepository, FeedRepository, NewFeedSubscription};
use rssr_infra::{
    db::{
        entry_repository::{
            LocalizedEntryUpdate, SqliteEntryRepository, compute_entry_content_hash,
        },
        feed_repository::SqliteFeedRepository,
        migrate,
        sqlite_native::NativeSqliteBackend,
        storage_backend::StorageBackend,
    },
    parser::FeedParser,
};
use url::Url;

const SAMPLE_FEED: &str = r#"
<rss version="2.0">
  <channel>
    <title>Example Feed</title>
    <link>https://example.com</link>
    <description>Example description</description>
    <item>
      <guid>entry-1</guid>
      <title>Hello World</title>
      <link>https://example.com/hello</link>
      <description>First summary</description>
      <pubDate>Tue, 25 Mar 2026 10:00:00 GMT</pubDate>
    </item>
    <item>
      <guid>entry-1</guid>
      <title>Hello World Updated</title>
      <link>https://example.com/hello</link>
      <description>Updated summary</description>
      <pubDate>Tue, 25 Mar 2026 10:00:00 GMT</pubDate>
    </item>
  </channel>
</rss>
"#;

#[tokio::test]
async fn refresh_flow_stores_feed_and_deduplicated_entries() {
    let backend = NativeSqliteBackend::new("sqlite::memory:");
    let pool = backend.connect().await.expect("connect sqlite memory");
    migrate(&pool).await.expect("run migrations");

    let feed_repository = SqliteFeedRepository::new(pool.clone());
    let entry_repository = SqliteEntryRepository::new(pool.clone());
    let parser = FeedParser::new();

    let feed = feed_repository
        .upsert_subscription(&NewFeedSubscription {
            url: Url::parse("https://example.com/feed.xml").expect("valid url"),
            title: Some("Example Feed".to_string()),
            folder: None,
        })
        .await
        .expect("create feed");

    let parsed = parser.parse(SAMPLE_FEED).expect("parse sample feed");
    let changed = entry_repository
        .upsert_entries(feed.id, &parsed.entries)
        .await
        .expect("upsert parsed entries");

    assert_eq!(changed, 2);

    let feeds = feed_repository.list_summaries().await.expect("list feed summaries");
    assert_eq!(feeds.len(), 1);
    assert_eq!(feeds[0].unread_count, 1);

    feed_repository
        .update_feed_metadata(feed.id, &parsed)
        .await
        .expect("persist parsed feed metadata");

    let stored_feed = feed_repository
        .get_feed(feed.id)
        .await
        .expect("read stored feed")
        .expect("feed must exist");
    assert_eq!(stored_feed.title.as_deref(), Some("Example Feed"));
    assert_eq!(stored_feed.description.as_deref(), Some("Example description"));
    assert_eq!(stored_feed.site_url.as_ref().map(|url| url.as_str()), Some("https://example.com/"));

    let entries =
        entry_repository.list_entries(&EntryQuery::default()).await.expect("list entries");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].title, "Hello World Updated");
}

#[tokio::test]
async fn localized_writeback_does_not_override_newer_refresh_content() {
    let backend = NativeSqliteBackend::new("sqlite::memory:");
    let pool = backend.connect().await.expect("connect sqlite memory");
    migrate(&pool).await.expect("run migrations");

    let feed_repository = SqliteFeedRepository::new(pool.clone());
    let entry_repository = SqliteEntryRepository::new(pool.clone());

    let feed = feed_repository
        .upsert_subscription(&NewFeedSubscription {
            url: Url::parse("https://example.com/feed.xml").expect("valid url"),
            title: Some("Example Feed".to_string()),
            folder: None,
        })
        .await
        .expect("create feed");

    let first_html = "<p>old</p>";
    let first_hash =
        compute_entry_content_hash(Some(first_html), Some("summary"), Some("Entry")).unwrap();
    entry_repository
        .upsert_entries(
            feed.id,
            &[rssr_infra::parser::feed_parser::ParsedEntry {
                external_id: "entry-1".to_string(),
                dedup_key: "entry-1".to_string(),
                url: Some(Url::parse("https://example.com/entry-1").unwrap()),
                title: "Entry".to_string(),
                author: None,
                summary: Some("summary".to_string()),
                content_html: Some(first_html.to_string()),
                content_text: Some("summary".to_string()),
                published_at: None,
                updated_at_source: None,
            }],
        )
        .await
        .expect("insert first revision");

    let newer_html = "<p>new</p>";
    entry_repository
        .upsert_entries(
            feed.id,
            &[rssr_infra::parser::feed_parser::ParsedEntry {
                external_id: "entry-1".to_string(),
                dedup_key: "entry-1".to_string(),
                url: Some(Url::parse("https://example.com/entry-1").unwrap()),
                title: "Entry".to_string(),
                author: None,
                summary: Some("summary".to_string()),
                content_html: Some(newer_html.to_string()),
                content_text: Some("summary".to_string()),
                published_at: None,
                updated_at_source: None,
            }],
        )
        .await
        .expect("insert newer revision");

    let localized_old_html = "<p>old<img src=\"data:image/png;base64,xxx\"></p>";
    let localized_old_hash =
        compute_entry_content_hash(Some(localized_old_html), Some("summary"), Some("Entry"))
            .unwrap();
    let updated = entry_repository
        .update_localized_html_if_hash_matches(
            feed.id,
            &LocalizedEntryUpdate {
                dedup_key: "entry-1",
                expected_content_hash: &first_hash,
                localized_html: localized_old_html,
                localized_content_hash: &localized_old_hash,
            },
        )
        .await
        .expect("attempt localized writeback");

    assert!(!updated, "stale localization should not overwrite newer content");

    let stored = entry_repository.get_entry(1).await.expect("load entry").expect("entry exists");
    assert_eq!(stored.content_html.as_deref(), Some(newer_html));
}

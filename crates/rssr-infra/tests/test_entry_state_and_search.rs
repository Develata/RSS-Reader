use rssr_domain::{EntryQuery, EntryRepository, FeedRepository, NewFeedSubscription};
use rssr_infra::{
    db::{
        entry_repository::SqliteEntryRepository, feed_repository::SqliteFeedRepository, migrate,
        sqlite_native::NativeSqliteBackend, storage_backend::StorageBackend,
    },
    parser::FeedParser,
};
use url::Url;

const SAMPLE_FEED: &str = r#"
<rss version="2.0">
  <channel>
    <title>Example Feed</title>
    <item>
      <guid>entry-1</guid>
      <title>Hello World</title>
      <link>https://example.com/hello</link>
      <description>First summary</description>
      <pubDate>Tue, 25 Mar 2026 10:00:00 GMT</pubDate>
    </item>
    <item>
      <guid>entry-2</guid>
      <title>Rust News</title>
      <link>https://example.com/rust</link>
      <description>Rust summary</description>
      <pubDate>Tue, 25 Mar 2026 11:00:00 GMT</pubDate>
    </item>
  </channel>
</rss>
"#;

#[tokio::test]
async fn entry_repository_updates_state_and_supports_search() {
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
    entry_repository.upsert_entries(feed.id, &parsed.entries).await.expect("insert entries");

    let all_entries =
        entry_repository.list_entries(&EntryQuery::default()).await.expect("list entries");
    assert_eq!(all_entries.len(), 2);

    let entry_id = all_entries[0].id;
    entry_repository.set_read(entry_id, true).await.expect("set read");
    entry_repository.set_starred(entry_id, true).await.expect("set starred");

    let unread = entry_repository
        .list_entries(&EntryQuery { unread_only: true, ..EntryQuery::default() })
        .await
        .expect("list unread");
    assert_eq!(unread.len(), 1);

    let starred = entry_repository
        .list_entries(&EntryQuery { starred_only: true, ..EntryQuery::default() })
        .await
        .expect("list starred");
    assert_eq!(starred.len(), 1);

    let searched = entry_repository
        .list_entries(&EntryQuery {
            search_title: Some("Rust".to_string()),
            ..EntryQuery::default()
        })
        .await
        .expect("search title");
    assert_eq!(searched.len(), 1);
    assert_eq!(searched[0].title, "Rust News");

    let searched_case_insensitive = entry_repository
        .list_entries(&EntryQuery {
            search_title: Some("rust".to_string()),
            ..EntryQuery::default()
        })
        .await
        .expect("search title case insensitive");
    assert_eq!(searched_case_insensitive.len(), 1);
    assert_eq!(searched_case_insensitive[0].title, "Rust News");
}

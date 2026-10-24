#![cfg(not(target_arch = "wasm32"))]

use rssr_domain::{
    ArchiveFilter, EntryQuery, FeedRepository, NewFeedSubscription, ReadFilter, StarredFilter,
};
use rssr_infra::{
    db::{
        entry_repository::SqliteEntryRepository, feed_repository::SqliteFeedRepository, migrate,
        sqlite_native::NativeSqliteBackend, storage_backend::StorageBackend,
    },
    parser::{FeedParser, feed_parser::ParsedEntry},
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
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
            site_url: None,
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
        .list_entries(&EntryQuery { read_filter: ReadFilter::UnreadOnly, ..EntryQuery::default() })
        .await
        .expect("list unread");
    assert_eq!(unread.len(), 1);

    let read = entry_repository
        .list_entries(&EntryQuery { read_filter: ReadFilter::ReadOnly, ..EntryQuery::default() })
        .await
        .expect("list read");
    assert_eq!(read.len(), 1);

    let starred = entry_repository
        .list_entries(&EntryQuery {
            starred_filter: StarredFilter::StarredOnly,
            ..EntryQuery::default()
        })
        .await
        .expect("list starred");
    assert_eq!(starred.len(), 1);

    let unstarred = entry_repository
        .list_entries(&EntryQuery {
            starred_filter: StarredFilter::UnstarredOnly,
            ..EntryQuery::default()
        })
        .await
        .expect("list unstarred");
    assert_eq!(unstarred.len(), 1);

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

/// 归档筛选下沉到存储层后的契约：
/// 未归档集合与已归档计数必须互补，且没有 `published_at` 的条目永远算未归档。
#[tokio::test]
async fn entry_repository_applies_archive_filter_in_query() {
    let backend = NativeSqliteBackend::new("sqlite::memory:");
    let pool = backend.connect().await.expect("connect sqlite memory");
    migrate(&pool).await.expect("run migrations");

    let feed_repository = SqliteFeedRepository::new(pool.clone());
    let entry_repository = SqliteEntryRepository::new(pool.clone());

    let feed = feed_repository
        .upsert_subscription(&NewFeedSubscription {
            site_url: None,
            url: Url::parse("https://example.com/archive-feed.xml").expect("valid url"),
            title: Some("Archive Feed".to_string()),
            folder: None,
        })
        .await
        .expect("create feed");

    let parse = |raw: &str| OffsetDateTime::parse(raw, &Rfc3339).expect("parse timestamp");
    let entry = |id: &str, published_at: Option<OffsetDateTime>| ParsedEntry {
        external_id: id.to_string(),
        dedup_key: id.to_string(),
        url: Some(Url::parse(&format!("https://example.com/{id}")).expect("valid url")),
        title: format!("Entry {id}"),
        author: None,
        summary: Some("summary".to_string()),
        content_html: None,
        content_text: Some("summary".to_string()),
        published_at,
        updated_at_source: None,
    };

    entry_repository
        .upsert_entries(
            feed.id,
            &[
                entry("old", Some(parse("2020-01-01T00:00:00Z"))),
                entry("recent", Some(parse("2026-07-01T00:00:00Z"))),
                entry("undated", None),
            ],
        )
        .await
        .expect("insert entries");

    let cutoff = parse("2026-04-01T00:00:00Z");

    let visible = entry_repository
        .list_entries(&EntryQuery {
            archive_filter: ArchiveFilter::ExcludeArchived { cutoff },
            ..EntryQuery::default()
        })
        .await
        .expect("list non-archived");
    let mut visible_titles = visible.iter().map(|entry| entry.title.as_str()).collect::<Vec<_>>();
    visible_titles.sort_unstable();
    assert_eq!(visible_titles, vec!["Entry recent", "Entry undated"]);

    let archived_count = entry_repository
        .count_entries(&EntryQuery {
            archive_filter: ArchiveFilter::OnlyArchived { cutoff },
            ..EntryQuery::default()
        })
        .await
        .expect("count archived");
    assert_eq!(archived_count, 1);

    let unfiltered =
        entry_repository.count_entries(&EntryQuery::default()).await.expect("count all");
    assert_eq!(unfiltered, visible.len() as u64 + archived_count);
}

#[tokio::test]
async fn entry_repository_resolves_content_after_batch_upsert() {
    let backend = NativeSqliteBackend::new("sqlite::memory:");
    let pool = backend.connect().await.expect("connect sqlite memory");
    migrate(&pool).await.expect("run migrations");

    let feed_repository = SqliteFeedRepository::new(pool.clone());
    let entry_repository = SqliteEntryRepository::new(pool.clone());

    let feed = feed_repository
        .upsert_subscription(&NewFeedSubscription {
            site_url: None,
            url: Url::parse("https://example.com/content-feed.xml").expect("valid url"),
            title: Some("Content Feed".to_string()),
            folder: None,
        })
        .await
        .expect("create feed");

    let entries = vec![
        ParsedEntry {
            external_id: "entry-1".to_string(),
            dedup_key: "entry-1".to_string(),
            url: Some(Url::parse("https://example.com/entry-1").expect("valid url")),
            title: "Entry 1".to_string(),
            author: None,
            summary: Some("Summary 1".to_string()),
            content_html: Some("<p>Body 1</p>".to_string()),
            content_text: Some("Body 1".to_string()),
            published_at: None,
            updated_at_source: None,
        },
        ParsedEntry {
            external_id: "entry-2".to_string(),
            dedup_key: "entry-2".to_string(),
            url: Some(Url::parse("https://example.com/entry-2").expect("valid url")),
            title: "Entry 2".to_string(),
            author: None,
            summary: Some("Summary 2".to_string()),
            content_html: Some("<p>Body 2</p>".to_string()),
            content_text: Some("Body 2".to_string()),
            published_at: None,
            updated_at_source: None,
        },
    ];

    let resolved = entry_repository
        .upsert_entries_and_resolve_contents(feed.id, &entries)
        .await
        .expect("upsert entries and resolve contents");
    assert_eq!(resolved.len(), 2);
    assert_ne!(resolved[0].entry_id, resolved[1].entry_id);

    entry_repository.upsert_contents(feed.id, &resolved).await.expect("upsert contents");

    let stored = entry_repository
        .get_content(resolved[1].entry_id)
        .await
        .expect("get content")
        .expect("content exists");
    assert_eq!(stored.content_html.as_deref(), Some("<p>Body 2</p>"));
    assert_eq!(stored.content_text.as_deref(), Some("Body 2"));
}

#[tokio::test]
async fn unread_summaries_are_global_idempotent_and_unchanged_on_failure() {
    let backend = NativeSqliteBackend::new("sqlite::memory:");
    let pool = backend.connect().await.unwrap();
    migrate(&pool).await.unwrap();
    let feeds = std::sync::Arc::new(SqliteFeedRepository::new(pool.clone()));
    let entries = std::sync::Arc::new(SqliteEntryRepository::new(pool));
    let feed = feeds
        .upsert_subscription(&NewFeedSubscription {
            site_url: None,
            url: Url::parse("https://example.com/feed.xml").unwrap(),
            title: Some("Example Feed".into()),
            folder: None,
        })
        .await
        .unwrap();
    let parsed = FeedParser::new().parse(SAMPLE_FEED).unwrap();
    entries.upsert_entries(feed.id, &parsed.entries).await.unwrap();
    let all = entries.list_entries(&EntryQuery::default()).await.unwrap();
    let reader =
        rssr_application::ReaderService::new(entries.clone(), entries.clone(), feeds.clone());
    let snapshot = reader.load_entry(all[0].id).await.unwrap();
    assert_eq!(snapshot.feed_title.as_deref(), Some("Example Feed"));
    assert_eq!(snapshot.entry.unwrap().id, all[0].id);
    assert_eq!(feeds.list_summaries().await.unwrap()[0].unread_count, 2);
    entries.set_read(all[0].id, true).await.unwrap();
    entries.set_read(all[0].id, true).await.unwrap();
    assert_eq!(feeds.list_summaries().await.unwrap()[0].unread_count, 1);
    entries
        .list_entries(&EntryQuery {
            search_title: Some("no match".into()),
            archive_filter: ArchiveFilter::All,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(feeds.list_summaries().await.unwrap()[0].unread_count, 1);
    assert!(entries.set_read(i64::MAX, true).await.is_err());
    assert_eq!(feeds.list_summaries().await.unwrap()[0].unread_count, 1);
    entries.set_read(all[1].id, true).await.unwrap();
    assert_eq!(feeds.list_summaries().await.unwrap()[0].unread_count, 0);
    entries.set_read(all[0].id, false).await.unwrap();
    assert_eq!(feeds.list_summaries().await.unwrap()[0].unread_count, 1);
}

#[tokio::test]
async fn selected_batches_are_explicit_atomic_and_manual_archive_preserves_flags() {
    use rssr_domain::{EntryBatchAction, EntryBatchRequest, EntryIndexRepository};
    let pool = NativeSqliteBackend::new("sqlite::memory:").connect().await.unwrap();
    migrate(&pool).await.unwrap();
    let feeds = SqliteFeedRepository::new(pool.clone());
    let entries = SqliteEntryRepository::new(pool.clone());
    let feed = feeds
        .upsert_subscription(&NewFeedSubscription {
            site_url: None,
            url: Url::parse("https://example.com/selected").unwrap(),
            title: Some("Original".into()),
            folder: None,
        })
        .await
        .unwrap();
    entries
        .upsert_entries(feed.id, &FeedParser::new().parse(SAMPLE_FEED).unwrap().entries)
        .await
        .unwrap();
    let all = entries.list_entries(&EntryQuery::default()).await.unwrap();
    let selected = all[0].id;
    entries.set_starred(selected, true).await.unwrap();
    let read = EntryBatchRequest {
        entry_ids: vec![selected, selected],
        action: EntryBatchAction::MarkRead,
    };
    assert_eq!(entries.apply_entry_batch(&read).await.unwrap(), 1);
    assert_eq!(entries.apply_entry_batch(&read).await.unwrap(), 0);
    let all = entries.list_entries(&EntryQuery::default()).await.unwrap();
    assert!(all.iter().find(|entry| entry.id == selected).unwrap().is_read);
    assert!(!all.iter().find(|entry| entry.id != selected).unwrap().is_read);
    sqlx::query("UPDATE entries SET published_at = NULL WHERE id = ?1")
        .bind(selected)
        .execute(&pool)
        .await
        .unwrap();
    let archive =
        EntryBatchRequest { entry_ids: vec![selected], action: EntryBatchAction::Archive };
    assert_eq!(entries.apply_entry_batch(&archive).await.unwrap(), 1);
    let visible = entries
        .list_entries(&EntryQuery {
            archive_filter: ArchiveFilter::ExcludeManual,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(visible.len(), 1);
    assert_ne!(visible[0].id, selected);
    let archived = entries
        .list_entries(&EntryQuery {
            archive_filter: ArchiveFilter::OnlyManual,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(archived.len(), 1);
    assert!(archived[0].is_read && archived[0].is_starred);
    assert_eq!(
        entries
            .count_entries(&EntryQuery {
                archive_filter: ArchiveFilter::OnlyArchived { cutoff: OffsetDateTime::now_utc() },
                ..Default::default()
            })
            .await
            .unwrap(),
        2
    );
}

#[tokio::test]
async fn source_edit_preserves_identity_flags_and_moves_filter_url_while_invalidating_old_refresh()
{
    use rssr_domain::{AppStateRepository, AppStateSnapshot, FeedEditRequest};
    use rssr_infra::db::app_state_repository::SqliteAppStateRepository;
    let pool = NativeSqliteBackend::new("sqlite::memory:").connect().await.unwrap();
    migrate(&pool).await.unwrap();
    let feeds = SqliteFeedRepository::new(pool.clone());
    let entries = SqliteEntryRepository::new(pool.clone());
    let app_state = SqliteAppStateRepository::new(pool.clone());
    let feed = feeds
        .upsert_subscription(&NewFeedSubscription {
            site_url: None,
            url: Url::parse("https://example.com/old").unwrap(),
            title: Some("Original".into()),
            folder: None,
        })
        .await
        .unwrap();
    let parsed = FeedParser::new().parse(SAMPLE_FEED).unwrap();
    entries.upsert_entries(feed.id, &parsed.entries).await.unwrap();
    let before = entries.list_entries(&EntryQuery::default()).await.unwrap();
    entries.set_read(before[0].id, true).await.unwrap();
    entries.set_starred(before[0].id, true).await.unwrap();
    let old_generation = feeds.get_feed_with_generation(feed.id).await.unwrap().unwrap().1;
    let mut state = AppStateSnapshot::default();
    state.entries_workspace.selected_feed_urls.push(feed.url.to_string());
    app_state.save(&state).await.unwrap();
    let request = FeedEditRequest {
        feed_id: feed.id,
        url: "https://example.com/new".into(),
        custom_title: Some("My source".into()),
    };
    feeds.update_subscription(&request).await.unwrap();
    let (updated, generation) = feeds.get_feed_with_generation(feed.id).await.unwrap().unwrap();
    assert_eq!(generation, old_generation + 1);
    assert_eq!(updated.title.as_deref(), Some("My source"));
    assert!(
        feeds.update_feed_metadata_for_generation(feed.id, old_generation, &parsed).await.is_err()
    );
    feeds.update_feed_metadata_for_generation(feed.id, generation, &parsed).await.unwrap();
    assert_eq!(feeds.get_feed(feed.id).await.unwrap().unwrap().title.as_deref(), Some("My source"));
    let details = feeds.edit_details(feed.id).await.unwrap();
    assert_eq!(details.original_title.as_deref(), Some("Example Feed"));
    assert_eq!(
        app_state.load().await.unwrap().entries_workspace.selected_feed_urls,
        vec![request.url.clone()]
    );
    let after = entries.list_entries(&EntryQuery::default()).await.unwrap();
    assert_eq!(
        before.iter().map(|entry| entry.id).collect::<Vec<_>>(),
        after.iter().map(|entry| entry.id).collect::<Vec<_>>()
    );
    assert!(after[0].is_read && after[0].is_starred);
    assert_eq!(after[0].feed_title, "My source");
    feeds.update_subscription(&FeedEditRequest { custom_title: None, ..request }).await.unwrap();
    assert_eq!(
        feeds.get_feed(feed.id).await.unwrap().unwrap().title.as_deref(),
        Some("Example Feed")
    );
}

#[tokio::test]
async fn conflicting_source_edit_rolls_back_name_and_url() {
    use rssr_domain::FeedEditRequest;
    let pool = NativeSqliteBackend::new("sqlite::memory:").connect().await.unwrap();
    migrate(&pool).await.unwrap();
    let feeds = SqliteFeedRepository::new(pool);
    let mut ids = Vec::new();
    for name in ["one", "two"] {
        ids.push(
            feeds
                .upsert_subscription(&NewFeedSubscription {
                    site_url: None,
                    url: Url::parse(&format!("https://example.com/{name}")).unwrap(),
                    title: Some(name.into()),
                    folder: None,
                })
                .await
                .unwrap()
                .id,
        );
    }
    let before = feeds.edit_details(ids[0]).await.unwrap();
    assert!(
        feeds
            .update_subscription(&FeedEditRequest {
                feed_id: ids[0],
                url: "https://example.com/two".into(),
                custom_title: Some("Changed".into())
            })
            .await
            .is_err()
    );
    assert_eq!(feeds.edit_details(ids[0]).await.unwrap(), before);
}

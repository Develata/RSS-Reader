#![cfg(any(not(target_arch = "wasm32"), feature = "memory-store"))]

//! Entry flags, filters, title search and bulk mark-read against the v0.2 library store
//! (port of `test_entry_state_and_search`).

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[path = "support/library.rs"]
mod library;

use std::sync::Arc;

use library::Library;
use rssr_application::ParsedEntryData;
use rssr_domain::{
    ArchiveFilter, EntryContentRepository, EntryIndexRepository, EntryQuery, FeedRepository,
    MarkReadOutcome, NewFeedSubscription, ReadFilter, StarredFilter,
};
use rssr_infra::parser::{FeedParser, feed_parser::ParsedEntry};
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

/// The parser's entries in the shape the refresh port takes.
fn to_data(entries: Vec<ParsedEntry>) -> Vec<ParsedEntryData> {
    entries
        .into_iter()
        .map(|entry| ParsedEntryData {
            external_id: entry.external_id,
            dedup_key: entry.dedup_key,
            url: entry.url,
            title: entry.title,
            author: entry.author,
            summary: entry.summary,
            content_html: entry.content_html,
            content_text: entry.content_text,
            published_at: entry.published_at,
            updated_at_source: entry.updated_at_source,
        })
        .collect()
}

async fn named_feed(library: &Library, url: &str, title: Option<&str>) -> rssr_domain::Feed {
    library
        .feeds
        .upsert_subscription(&NewFeedSubscription {
            site_url: None,
            url: Url::parse(url).expect("valid url"),
            title: title.map(str::to_owned),
            source_title: None,
            folder: None,
        })
        .await
        .expect("create feed")
}

#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn entry_repository_updates_state_and_supports_search() {
    let library = Library::memory().await;
    let feed = named_feed(&library, "https://example.com/feed.xml", Some("Example Feed")).await;

    let parsed = FeedParser::new().parse(SAMPLE_FEED).expect("parse sample feed");
    library.seed(feed.id, to_data(parsed.entries)).await;
    let entry_repository = &library.entries;

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
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn entry_repository_applies_archive_filter_in_query() {
    let library = Library::memory().await;
    let feed =
        named_feed(&library, "https://example.com/archive-feed.xml", Some("Archive Feed")).await;

    let parse = |raw: &str| OffsetDateTime::parse(raw, &Rfc3339).expect("parse timestamp");
    let entry = |id: &str, published_at: Option<OffsetDateTime>| ParsedEntryData {
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

    library
        .seed(
            feed.id,
            vec![
                entry("old", Some(parse("2020-01-01T00:00:00Z"))),
                entry("recent", Some(parse("2026-07-01T00:00:00Z"))),
                entry("undated", None),
            ],
        )
        .await;
    let entry_repository = &library.entries;

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

/// The old `upsert_entries_and_resolve_contents` handed back the ids a batch got; the library
/// has no such call, so the ids come from the list and the record (`get_entry_record`).
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn entry_repository_resolves_content_after_batch_upsert() {
    let library = Library::memory().await;
    let feed =
        named_feed(&library, "https://example.com/content-feed.xml", Some("Content Feed")).await;

    let entries = vec![
        ParsedEntryData {
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
        ParsedEntryData {
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

    assert_eq!(library.seed(feed.id, entries).await, 2);
    let resolved = library.entries.list_entries(&EntryQuery::default()).await.expect("list");
    assert_eq!(resolved.len(), 2);
    assert_ne!(resolved[0].id, resolved[1].id);

    let second = resolved.iter().find(|entry| entry.title == "Entry 2").expect("entry 2");
    let record = library.entries.get_entry_record(second.id).await.unwrap().expect("record");
    assert_eq!(record.dedup_key, "entry-2");
    assert!(record.has_content);

    let stored =
        library.entries.get_content(second.id).await.expect("get content").expect("content exists");
    assert_eq!(stored.content_html.as_deref(), Some("<p>Body 2</p>"));
    assert_eq!(stored.content_text.as_deref(), Some("Body 2"));
}

#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn unread_summaries_are_global_idempotent_and_unchanged_on_failure() {
    let library = Library::memory().await;
    let feeds = Arc::new(library.feeds.clone());
    let entries = Arc::new(library.entries.clone());
    let feed = named_feed(&library, "https://example.com/feed.xml", Some("Example Feed")).await;
    let parsed = FeedParser::new().parse(SAMPLE_FEED).unwrap();
    library.seed(feed.id, to_data(parsed.entries)).await;
    let all = entries.list_entries(&EntryQuery::default()).await.unwrap();
    let reader = rssr_application::ReaderService::new(Arc::new(library.queries.clone()));
    let snapshot = reader.load_entry(all[0].id).await.unwrap();
    assert_eq!(snapshot.feed_title.as_deref(), Some("Example Feed"));
    assert_eq!(snapshot.entry.unwrap().id, all[0].id);
    assert_eq!(feeds.list_summaries().await.unwrap()[0].unread_count, 2);
    assert!(entries.set_read(all[0].id, true).await.unwrap(), "the first request changes it");
    assert!(!entries.set_read(all[0].id, true).await.unwrap(), "a repeat changes nothing");
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

/// Flags are set to target values: a repeat writes nothing and keeps the original time;
/// unsetting clears it.
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn repeated_flag_requests_keep_the_original_time() {
    let library = Library::memory().await;
    let feed = named_feed(&library, "https://example.com/feed.xml", None).await;
    let parsed = FeedParser::new().parse(SAMPLE_FEED).unwrap();
    library.seed(feed.id, to_data(parsed.entries)).await;
    let id = library.entries.list_entries(&EntryQuery::default()).await.unwrap()[0].id;
    assert!(library.entries.set_read(id, true).await.unwrap());
    assert!(library.entries.set_starred(id, true).await.unwrap());
    let original = "2000-01-01T00:00:00Z";
    library
        .exec(format!(
            "UPDATE entries SET read_at = '{original}', starred_at = '{original}',
             updated_at = '{original}' WHERE id = {id}"
        ))
        .await;

    assert!(!library.entries.set_read(id, true).await.unwrap());
    assert!(!library.entries.set_starred(id, true).await.unwrap());
    let times = format!("SELECT read_at || starred_at || updated_at FROM entries WHERE id = {id}");
    assert_eq!(library.text(times.clone()).await, Some(original.repeat(3)));

    assert!(library.entries.set_read(id, false).await.unwrap());
    let record = library.entries.get_entry_record(id).await.unwrap().unwrap();
    assert!(!record.is_read && record.read_at.is_none());
    assert_eq!(record.starred_at, Some(OffsetDateTime::parse(original, &Rfc3339).unwrap()));
}

/// 标题搜索是字面量匹配：`%`、`_`、`\` 不能作为 LIKE 通配符扩大命中集合。
/// 列表、计数与批量已读预览共用同一筛选，三者必须一致（浏览器端为字面 contains）。
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn search_title_treats_like_wildcards_as_literals() {
    let library = Library::memory().await;
    let feed = named_feed(&library, "https://example.com/wildcards.xml", None).await;
    let entry = |id: &str, title: &str| ParsedEntryData {
        external_id: id.to_string(),
        dedup_key: id.to_string(),
        url: None,
        title: title.to_string(),
        author: None,
        summary: Some("summary".to_string()),
        content_html: None,
        content_text: Some("summary".to_string()),
        published_at: None,
        updated_at_source: None,
    };
    library
        .seed(
            feed.id,
            vec![
                entry("percent", "100% done"),
                entry("digits", "1000 done"),
                entry("underscore", "a_b"),
                entry("letter", "axb"),
                entry("backslash", r"C:\path"),
                entry("plain", "C:path"),
            ],
        )
        .await;
    let entry_repository = &library.entries;

    for (search, expected) in [("100%", "100% done"), ("a_b", "a_b"), (r"C:\", r"C:\path")] {
        let query = EntryQuery { search_title: Some(search.to_string()), ..EntryQuery::default() };
        let titles = entry_repository
            .list_entries(&query)
            .await
            .expect("search entries")
            .into_iter()
            .map(|entry| entry.title)
            .collect::<Vec<_>>();
        assert_eq!(titles, vec![expected.to_string()], "search {search:?}");
        assert_eq!(entry_repository.count_entries(&query).await.expect("count"), 1);
        let preview = entry_repository.preview_mark_read(&query).await.expect("preview");
        assert_eq!(preview.unread_entry_ids.len(), 1, "bulk preview for {search:?}");
        // 实际批量写入与预览集合一致，不得多标。
        assert_eq!(
            entry_repository.mark_read_if_unchanged(&preview).await.expect("bulk mark read"),
            MarkReadOutcome::Applied { changed_count: 1 },
            "bulk write for {search:?}"
        );
    }

    let mut still_unread = entry_repository
        .list_entries(&EntryQuery { read_filter: ReadFilter::UnreadOnly, ..EntryQuery::default() })
        .await
        .expect("list unread")
        .into_iter()
        .map(|entry| entry.title)
        .collect::<Vec<_>>();
    still_unread.sort_unstable();
    assert_eq!(still_unread, vec!["1000 done", "C:path", "axb"]);
}

/// The selection is frozen by ID, bounded, idempotent, and shared by native/Pages SQL.
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn selected_archive_is_atomic_and_counts_the_union_with_automatic_archive() {
    use rssr_application::{EntriesListService, EntryBatchService};
    use rssr_domain::{EntryBatchAction, EntryBatchRequest};
    use rssr_infra::store::LibraryWorkspaceEdits;
    let library = Library::memory().await;
    let feed = library.add_feed("https://example.com/a.xml").await;
    let now = OffsetDateTime::parse("2026-10-27T12:00:00Z", &Rfc3339).unwrap();
    library
        .seed(
            feed.id,
            vec![
                library::published("old", "Old", Some(now - time::Duration::days(700))),
                library::entry("undated"),
                library::published("recent", "Recent", Some(now)),
            ],
        )
        .await;
    let rows = library.entries.list_entries(&EntryQuery::default()).await.unwrap();
    let id = |title: &str| rows.iter().find(|r| r.title == title).unwrap().id;
    let service =
        EntryBatchService::new(Arc::new(LibraryWorkspaceEdits::new(library.store.clone())));
    let request = EntryBatchRequest {
        entry_ids: vec![id("undated"), id("undated"), id("Old")],
        action: EntryBatchAction::Archive,
    };
    assert_eq!(service.apply(request.clone()).await.unwrap(), 2);
    assert_eq!(service.apply(request).await.unwrap(), 0, "repeating changes nothing");
    let lists = EntriesListService::new(
        Arc::new(library.entries.clone()),
        Arc::new(library.queries.clone()),
    );
    for archive_filter in [ArchiveFilter::ExcludeManual, ArchiveFilter::exclude_archived(now, 6)] {
        let page =
            lists.list_entries(&EntryQuery { archive_filter, ..Default::default() }).await.unwrap();
        assert_eq!(page.entries.len(), 1);
        assert_eq!(page.entries[0].title, "Recent");
        assert_eq!(page.archived_count, 2, "automatic/manual overlap is counted once");
    }
    assert_eq!(lists.list_entries(&EntryQuery::default()).await.unwrap().entries.len(), 3);
    library.seed(feed.id, vec![library::entry("undated")]).await;
    assert_eq!(
        library
            .entries
            .count_entries(&EntryQuery {
                archive_filter: ArchiveFilter::OnlyManual,
                ..Default::default()
            })
            .await
            .unwrap(),
        2,
        "refresh preserves archive flags"
    );
    for ids in [vec![], vec![0], vec![1; rssr_domain::MAX_ENTRIES_PAGE_SIZE as usize + 1]] {
        let error = service
            .apply(EntryBatchRequest { entry_ids: ids, action: EntryBatchAction::MarkRead })
            .await
            .unwrap_err();
        assert_eq!(
            rssr_application::classify(&error).kind,
            rssr_application::ErrorKind::InvalidInput
        );
    }
    library.exec("CREATE TRIGGER fail_second_read BEFORE UPDATE OF is_read ON entries WHEN NEW.title = 'undated' BEGIN SELECT RAISE(ABORT, 'injected'); END;").await;
    let error = service
        .apply(EntryBatchRequest {
            entry_ids: rows.iter().map(|r| r.id).collect(),
            action: EntryBatchAction::MarkRead,
        })
        .await;
    assert!(error.is_err());
    assert_eq!(
        library
            .entries
            .count_entries(&EntryQuery { read_filter: ReadFilter::ReadOnly, ..Default::default() })
            .await
            .unwrap(),
        0,
        "a partial batch cannot commit"
    );
}

/// URL changes preserve articles, migrate the source filter atomically, and fence old refreshes.
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn feed_edit_keeps_identity_and_refuses_duplicate_or_partially_saved_changes() {
    use rssr_application::{FeedEditService, RefreshStorePort};
    use rssr_domain::{AppStateRepository, AppStateSnapshot, FeedEditRequest};
    use rssr_infra::store::LibraryWorkspaceEdits;
    let library = Library::memory().await;
    let feed = library.add_feed("https://example.com/old.xml").await;
    library.seed(feed.id, vec![library::entry("kept")]).await;
    let item = library.entries.list_entries(&EntryQuery::default()).await.unwrap()[0].id;
    library.entries.set_read(item, true).await.unwrap();
    library.entries.set_starred(item, true).await.unwrap();
    let mut state = AppStateSnapshot::default();
    state.entries_workspace.selected_feed_urls = vec![feed.url.to_string()];
    library.app_state.update(Box::new(move |stored| *stored = state)).await.unwrap();
    let old_target = library.refresh.get_target(feed.id).await.unwrap().unwrap();
    let service = FeedEditService::new(Arc::new(LibraryWorkspaceEdits::new(library.store.clone())));
    let changed = FeedEditRequest {
        feed_id: feed.id,
        url: " https://example.com/new.xml#ignored ".into(),
        custom_title: Some("  My source  ".into()),
    };
    service.update(changed).await.unwrap();
    let details = service.details(feed.id).await.unwrap();
    assert_eq!(details.url, "https://example.com/new.xml");
    assert_eq!(details.custom_title.as_deref(), Some("My source"));
    assert_eq!(
        library.app_state.load().await.unwrap().entries_workspace.selected_feed_urls,
        std::slice::from_ref(&details.url)
    );
    let stale = library
        .refresh
        .commit(&old_target, library::updated(vec![library::entry("obsolete")]))
        .await
        .unwrap_err();
    // Generation fencing preserves the store's existing removed/replaced-target contract.
    assert_eq!(rssr_application::classify(&stale).kind, rssr_application::ErrorKind::NotFound);
    let kept = library.entries.list_entries(&EntryQuery::default()).await.unwrap();
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].id, item);
    assert!(kept[0].is_read && kept[0].is_starred);
    let other = library.add_feed("https://example.com/other.xml").await;
    let error = service
        .update(FeedEditRequest {
            feed_id: feed.id,
            url: other.url.to_string(),
            custom_title: Some("must not apply".into()),
        })
        .await
        .unwrap_err();
    assert_eq!(rssr_application::classify(&error).kind, rssr_application::ErrorKind::Conflict);
    assert_eq!(service.details(feed.id).await.unwrap(), details);
    library.exec("CREATE TRIGGER fail_state BEFORE UPDATE ON app_settings BEGIN SELECT RAISE(ABORT, 'injected'); END;").await;
    assert!(
        service
            .update(FeedEditRequest {
                feed_id: feed.id,
                url: "https://example.com/third.xml".into(),
                custom_title: None
            })
            .await
            .is_err()
    );
    assert_eq!(
        service.details(feed.id).await.unwrap(),
        details,
        "source update rolls back with its filter migration"
    );
}

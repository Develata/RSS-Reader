#![cfg(any(not(target_arch = "wasm32"), feature = "memory-store"))]

//! The library's `LibraryQueryPort`: what each aggregate read returns. That all parts come from
//! one snapshot is the store's read transaction (`test_library_store::a_read_task_sees_one_snapshot`).

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[path = "support/library.rs"]
mod library;

use library::{Library, published};
use rssr_application::LibraryQueryPort;
use rssr_domain::{
    AppStateRepository, ArchiveFilter, EntryIndexRepository, EntryQuery, FeedRepository,
    SettingsRepository, StartupView, UserSettings,
};
use time::Duration;

async fn remember(library: &Library, feed_id: i64) {
    library
        .app_state
        .update(Box::new(move |state| state.last_opened_feed_id = Some(feed_id)))
        .await
        .expect("save last opened feed");
}

/// Migrated from the browser harness: missing dates, tied dates and default unread flags must
/// give reader navigation exactly the same ordering as the list. SQL foreign keys rule out
/// orphan feeds; deleted feeds cover the excluded entries here.
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn navigation_matches_list_order_with_missing_dates_ties_and_deleted_feeds() {
    use rssr_domain::EntryNavigation;
    let library = Library::memory().await;
    for id in 1..=4 {
        library.add_feed(&format!("https://example.com/{id}")).await;
    }
    library.exec("WITH RECURSIVE seq(id) AS (VALUES(1) UNION ALL SELECT id+1 FROM seq WHERE id<128)
        INSERT INTO entries(id, feed_id, external_id, dedup_key, title, title_search,
                            published_at, first_seen_at, created_at, updated_at, is_read)
        SELECT id, id%4+1, CAST(id AS TEXT), CAST(id AS TEXT), 'Entry', 'entry',
               CASE WHEN id%3<>0 THEN strftime('%Y-%m-%dT%H:%M:%SZ', (id*17%13)*3600, 'unixepoch') END,
               strftime('%Y-%m-%dT%H:%M:%SZ', (id*17%13)*3600, 'unixepoch'),
               strftime('%Y-%m-%dT%H:%M:%SZ', (id*17%13)*3600, 'unixepoch'),
               '1970-01-01T00:00:00Z', id%5=0 FROM seq;
        UPDATE feeds SET is_deleted=1 WHERE id IN (3,4)").await;
    let ordered = library.entries.list_entries(&EntryQuery::default()).await.unwrap();
    assert_eq!(ordered.len(), 64);
    for id in 0..=129 {
        let expected = ordered
            .iter()
            .position(|entry| entry.id == id)
            .map(|index| {
                let current = &ordered[index];
                let before = &ordered[..index];
                let after = &ordered[index + 1..];
                EntryNavigation {
                    previous_unread_entry_id: before
                        .iter()
                        .rev()
                        .find(|entry| !entry.is_read)
                        .map(|entry| entry.id),
                    next_unread_entry_id: after
                        .iter()
                        .find(|entry| !entry.is_read)
                        .map(|entry| entry.id),
                    previous_feed_entry_id: before
                        .iter()
                        .rev()
                        .find(|entry| entry.feed_id == current.feed_id)
                        .map(|entry| entry.id),
                    next_feed_entry_id: after
                        .iter()
                        .find(|entry| entry.feed_id == current.feed_id)
                        .map(|entry| entry.id),
                }
            })
            .unwrap_or_default();
        assert_eq!(library.entries.reader_navigation(id).await.unwrap(), expected, "entry {id}");
    }
}

#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn app_state_narrow_updates_do_not_lose_each_other() {
    use rssr_application::AppStateService;
    use rssr_domain::EntriesWorkspaceState;
    use std::sync::Arc;
    let library = Library::memory().await;
    let service = AppStateService::new(Arc::new(library.app_state.clone()));
    let workspace = EntriesWorkspaceState { show_archived: true, ..Default::default() };
    let (feed, entries) = futures_util::join!(
        service.save_last_opened_feed_id(Some(7)),
        service.save_entries_workspace(workspace.clone()),
    );
    feed.unwrap();
    entries.unwrap();
    let snapshot = library.app_state.load().await.unwrap();
    assert_eq!(snapshot.last_opened_feed_id, Some(7));
    assert_eq!(snapshot.entries_workspace, workspace);
}

#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn entries_page_counts_with_the_second_query() {
    let library = Library::memory().await;
    let feed = library.add_feed("https://example.com/feed.xml").await;
    let now = rssr_infra::platform_clock::now_utc();
    library
        .seed(
            feed.id,
            vec![
                published("new", "New", Some(now)),
                published("old-1", "Old 1", Some(now - Duration::days(400))),
                published("old-2", "Old 2", Some(now - Duration::days(500))),
            ],
        )
        .await;
    let cutoff = now - Duration::days(30);
    let page_query = EntryQuery {
        archive_filter: ArchiveFilter::ExcludeArchived { cutoff },
        ..EntryQuery::default()
    };
    let archived = EntryQuery {
        archive_filter: ArchiveFilter::OnlyArchived { cutoff },
        ..EntryQuery::default()
    };

    let page = library.queries.entries_page(&page_query, Some(&archived)).await.expect("page");
    assert_eq!(page.entries.iter().map(|entry| entry.title.as_str()).collect::<Vec<_>>(), ["New"]);
    assert_eq!(page.counted, Some(2));

    let uncounted = library.queries.entries_page(&page_query, None).await.expect("page");
    assert_eq!(uncounted.counted, None);
}

#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn reader_snapshot_has_the_record_body_neighbours_and_active_feed() {
    let library = Library::memory().await;
    let feed = library.add_feed("https://example.com/feed.xml").await;
    let now = rssr_infra::platform_clock::now_utc();
    let mut body = published("a", "A", Some(now));
    body.content_html = Some("<p>A</p>".into());
    library.seed(feed.id, vec![body, published("b", "B", Some(now - Duration::hours(1)))]).await;
    let entries = library.entries.list_entries(&EntryQuery::default()).await.expect("list");
    let (newer, older) = (entries[0].id, entries[1].id);

    let snapshot = library.queries.reader_snapshot(newer).await.expect("read").expect("exists");
    assert_eq!(snapshot.record.id, newer);
    assert_eq!(
        snapshot.content.and_then(|content| content.content_html).as_deref(),
        Some("<p>A</p>")
    );
    assert_eq!(snapshot.navigation.next_feed_entry_id, Some(older));
    assert_eq!(snapshot.feed.map(|feed| feed.id), Some(feed.id));

    assert!(library.queries.reader_snapshot(older + 100).await.expect("read").is_none());
}

#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn startup_state_keeps_only_an_active_last_opened_subscription() {
    let library = Library::memory().await;
    let feed = library.add_feed("https://example.com/feed.xml").await;
    let settings = UserSettings { startup_view: StartupView::LastFeed, ..UserSettings::default() };
    library.settings.save(&settings).await.expect("save settings");
    remember(&library, feed.id).await;

    let state = library.queries.startup_state().await.expect("startup");
    assert_eq!(state.settings, settings);
    assert_eq!(state.last_opened_feed_id, Some(feed.id));

    library.feeds.set_deleted(feed.id, true).await.expect("remove");
    let state = library.queries.startup_state().await.expect("startup");
    assert_eq!(state.last_opened_feed_id, None);
}

#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn entries_start_reads_only_what_is_asked() {
    let library = Library::memory().await;
    library.add_feed("https://example.com/feed.xml").await;

    let none = library.queries.entries_start(false, false).await.expect("start");
    assert_eq!((none.settings, none.workspace, none.feeds), (None, None, None));

    let all = library.queries.entries_start(true, true).await.expect("start");
    assert_eq!(all.settings, Some(UserSettings::default()));
    assert!(all.workspace.is_some());
    assert_eq!(all.feeds.map(|feeds| feeds.len()), Some(1));
}

#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn config_snapshot_has_the_active_subscriptions_and_settings() {
    let library = Library::memory().await;
    let kept = library.add_feed("https://example.com/kept.xml").await;
    let removed = library.add_feed("https://example.com/removed.xml").await;
    library.feeds.set_deleted(removed.id, true).await.expect("remove");
    let settings = UserSettings { refresh_interval_minutes: 45, ..UserSettings::default() };
    library.settings.save(&settings).await.expect("save settings");

    let snapshot = library.queries.config_snapshot().await.expect("config");
    assert_eq!(snapshot.feeds.iter().map(|feed| feed.id).collect::<Vec<_>>(), [kept.id]);
    assert_eq!(snapshot.settings, settings);
}

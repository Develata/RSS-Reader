#![cfg(not(target_arch = "wasm32"))]

//! The library's `LibraryQueryPort`: what each aggregate read returns. That all parts come from
//! one snapshot is the store's read transaction (`test_library_store::a_read_task_sees_one_snapshot`).

#[path = "support/library.rs"]
mod library;

use library::{Library, published};
use rssr_application::LibraryQueryPort;
use rssr_domain::{
    AppStateRepository, ArchiveFilter, EntryIndexRepository, EntryQuery, FeedRepository,
    SettingsRepository, StartupView, UserSettings,
};
use time::{Duration, OffsetDateTime};

async fn remember(library: &Library, feed_id: i64) {
    library
        .app_state
        .update(Box::new(move |state| state.last_opened_feed_id = Some(feed_id)))
        .await
        .expect("save last opened feed");
}

#[tokio::test]
async fn entries_page_counts_with_the_second_query() {
    let library = Library::memory().await;
    let feed = library.add_feed("https://example.com/feed.xml").await;
    let now = OffsetDateTime::now_utc();
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

#[tokio::test]
async fn reader_snapshot_has_the_record_body_neighbours_and_active_feed() {
    let library = Library::memory().await;
    let feed = library.add_feed("https://example.com/feed.xml").await;
    let now = OffsetDateTime::now_utc();
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

#[tokio::test]
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

#[tokio::test]
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

#[tokio::test]
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

use rssr_domain::UserSettings;
use serde::Deserialize;
use time::OffsetDateTime;

#[derive(Debug, Deserialize)]
struct FixturePersistedState {
    next_feed_id: i64,
    next_entry_id: i64,
    feeds: Vec<FixturePersistedFeed>,
    entries: Vec<FixturePersistedEntry>,
    settings: UserSettings,
}

#[derive(Debug, Deserialize)]
struct FixturePersistedFeed {
    id: i64,
    url: String,
    title: Option<String>,
    site_url: Option<String>,
    description: Option<String>,
    icon_url: Option<String>,
    folder: Option<String>,
    etag: Option<String>,
    last_modified: Option<String>,
    last_fetched_at: Option<OffsetDateTime>,
    last_success_at: Option<OffsetDateTime>,
    fetch_error: Option<String>,
    is_deleted: bool,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

#[derive(Debug, Deserialize)]
struct FixturePersistedEntry {
    id: i64,
    feed_id: i64,
    external_id: String,
    dedup_key: String,
    url: Option<String>,
    title: String,
    author: Option<String>,
    summary: Option<String>,
    content_html: Option<String>,
    content_text: Option<String>,
    published_at: Option<OffsetDateTime>,
    updated_at_source: Option<OffsetDateTime>,
    first_seen_at: OffsetDateTime,
    content_hash: Option<String>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

#[derive(Debug, Deserialize)]
struct FixturePersistedAppStateSlice {
    last_opened_feed_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct FixturePersistedEntryFlagsSlice {
    entries: Vec<FixturePersistedEntryFlag>,
}

#[derive(Debug, Deserialize)]
struct FixturePersistedEntryFlag {
    id: i64,
    is_read: bool,
    is_starred: bool,
    read_at: Option<OffsetDateTime>,
    starred_at: Option<OffsetDateTime>,
}

#[test]
fn reader_demo_seed_core_fixture_contains_reader_smoke_shape() {
    let raw = include_str!("../../../tests/fixtures/browser_state/reader_demo_core.json");
    let state: FixturePersistedState =
        serde_json::from_str(raw).expect("decode reader demo core fixture");

    assert_eq!(state.next_feed_id, 2);
    assert_eq!(state.next_entry_id, 3);
    assert_eq!(state.feeds.len(), 1);
    assert_eq!(state.entries.len(), 2);
    assert_eq!(state.entries[1].id, 2);
    assert_eq!(state.entries[1].title, "Demo Entry Two");
    assert_eq!(state.settings, UserSettings::default());
    assert_eq!(state.feeds[0].title.as_deref(), Some("Demo Feed"));
    assert_eq!(state.feeds[0].url, "https://example.com/feed.xml");
    assert!(!state.feeds[0].is_deleted);
    assert_eq!(state.entries[0].feed_id, 1);
}

#[test]
fn reader_demo_seed_sidecar_fixtures_expose_feed_and_entry_flags() {
    let app_state_raw =
        include_str!("../../../tests/fixtures/browser_state/reader_demo_app_state.json");
    let entry_flags_raw =
        include_str!("../../../tests/fixtures/browser_state/reader_demo_entry_flags.json");

    let app_state: FixturePersistedAppStateSlice =
        serde_json::from_str(app_state_raw).expect("decode app state fixture");
    let entry_flags: FixturePersistedEntryFlagsSlice =
        serde_json::from_str(entry_flags_raw).expect("decode entry flags fixture");

    assert_eq!(app_state.last_opened_feed_id, Some(1));
    assert_eq!(entry_flags.entries.len(), 2);
    assert_eq!(entry_flags.entries[1].id, 2);
    assert!(entry_flags.entries[1].is_starred);
    assert!(entry_flags.entries[0].is_read);
}

//! The pages' read-only refresh through the real runtime (phase C, C4b item 2): what each page
//! issues when the shell asks it to re-read only reads, and what it reads back only updates what
//! is shown. The hooks that issue these commands are the pages' (`library_revision`); that a
//! focus, a visible page or the network coming back trigger them is the browser acceptance's
//! (C4c).

use rssr_api::{BackendError, ErrorCode};
use rssr_domain::EntryQuery;

use super::execute_ui_command;
use super::fake_backend::Scripted;
use crate::pages::entries_page::{
    intent::EntriesPageIntent, reducer::reduce_entries_page_intent, state::EntriesPageState,
};
use crate::pages::feeds_page::{reducer::reduce_feeds_page_intent, state::FeedsPageState};
use crate::ui::commands::{
    EntriesCommand, FeedsCommand, ReaderCommand, SettingsCommand, ShellCommand, UiCommand,
};
use crate::ui::failure::TONE_UNKNOWN;
use crate::ui::runtime::backend::test_host;

/// Every backend operation that writes, or starts something.
const WRITES: [&str; 18] = [
    "update_settings",
    "apply_entry_batch",
    "update_subscription",
    "set_entry_read",
    "set_entry_starred",
    "save_entries_workspace_if_changed",
    "remember_opened_feed",
    "mark_read_if_unchanged",
    "add_subscription",
    "remove_feed",
    "refresh_feed",
    "refresh_all",
    "import_config_json",
    "import_opml",
    "push_remote_config",
    "pull_remote_config",
    "localize_entry_assets",
    "ensure_auto_refresh_started",
];

/// What each page's read-only refresh issues: the shell's settings, the entries page's
/// preferences and list, the subscriptions, the settings page's settings, the reader's entry.
fn rereads() -> Vec<UiCommand> {
    vec![
        UiCommand::Shell(ShellCommand::LoadSettings),
        UiCommand::Entries(EntriesCommand::Reread),
        UiCommand::Entries(EntriesCommand::LoadEntries { query: EntryQuery::default() }),
        UiCommand::Feeds(FeedsCommand::LoadSnapshot),
        UiCommand::Settings(SettingsCommand::Load),
        UiCommand::Reader(ReaderCommand::RereadEntry { entry_id: 1 }),
    ]
}

/// A read-only refresh of every page writes nothing and starts nothing (R2-07: "零写入").
#[tokio::test(flavor = "current_thread")]
async fn a_reread_only_reads() {
    let backend = Scripted::over_new_library();
    let _host = test_host::install(backend.clone());
    for command in rereads() {
        execute_ui_command(command).await;
    }
    for read in [
        "load_settings",
        "bootstrap_entries",
        "list_entries",
        "load_feeds_snapshot",
        "load_reader_entry",
    ] {
        assert!(backend.calls(read) >= 1, "{read} ran");
    }
    for write in WRITES {
        assert_eq!(backend.calls(write), 0, "{write}");
    }
}

/// A write whose outcome is unknown, then a successful read-only refresh: the page does not turn
/// the read into the write's success, and the write is not sent again (R2-07).
#[tokio::test(flavor = "current_thread")]
async fn a_read_after_an_unknown_write_claims_no_success() {
    let backend = Scripted::over_new_library();
    let _host = test_host::install(backend.clone());
    let unknown = BackendError::new(ErrorCode::OutcomeUnknown, "结果未知，请刷新查看");

    // The entry list: a flag write.
    backend.fail("set_entry_read", unknown.clone());
    let mut entries = EntriesPageState::new();
    reduce_entries_page_intent(&mut entries, EntriesPageIntent::PreferencesLoaded);
    let write = UiCommand::Entries(EntriesCommand::SetRead {
        entry_id: 1,
        entry_title: "甲".into(),
        read: true,
    });
    let reread = [
        UiCommand::Entries(EntriesCommand::Reread),
        UiCommand::Entries(EntriesCommand::LoadEntries { query: EntryQuery::default() }),
    ];
    for command in std::iter::once(write).chain(reread) {
        for intent in execute_ui_command(command).await {
            if let Some(intent) = intent.into_entries_page_intent() {
                reduce_entries_page_intent(&mut entries, intent);
            }
        }
    }
    assert!(entries.entries_loaded, "the list was read");
    assert_eq!(entries.status_tone, TONE_UNKNOWN, "{}", entries.status);
    assert!(entries.status.contains(&unknown.message));
    assert_eq!(backend.calls("set_entry_read"), 1);

    // The subscriptions: adding one.
    backend.fail("add_subscription", unknown.clone());
    let mut feeds = FeedsPageState::new();
    feeds.feed_url = "https://example.com/feed.xml".into();
    feeds.adding_feed_url = Some(feeds.feed_url.clone());
    let write = UiCommand::Feeds(FeedsCommand::AddFeed {
        raw_url: feeds.feed_url.clone(),
        fallback_site_url: None,
        token: feeds.add_token,
    });
    for command in [write, UiCommand::Feeds(FeedsCommand::LoadSnapshot)] {
        for intent in execute_ui_command(command).await {
            if let Some(intent) = intent.into_feeds_page_intent() {
                reduce_feeds_page_intent(&mut feeds, intent);
            }
        }
    }
    assert!(feeds.fingerprint.is_some(), "the snapshot was read");
    assert_eq!(feeds.status_tone, TONE_UNKNOWN, "{}", feeds.status);
    assert_eq!(feeds.feed_url, "https://example.com/feed.xml", "the address is kept");
    assert_eq!(backend.calls("add_subscription"), 1);
}

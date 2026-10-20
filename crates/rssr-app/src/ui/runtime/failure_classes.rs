//! Each class of backend failure through the real runtime into each page's state (phase C, C4b
//! item 1): the scripted backend fails one operation with an error of the class, the command runs
//! through [`super::execute_ui_command`], its intents go through the page's projection and reducer,
//! and the page's state is asserted: what the status says and its tone, the markers of work in
//! progress released, what the user typed kept, and what the failure asks of the whole app.

use std::sync::Arc;

use rssr_api::{BackendError, ErrorCode};
use rssr_domain::EntryQuery;

use super::execute_ui_command;
use super::fake_backend::Scripted;
use crate::pages::entries_page::{
    intent::EntriesPageIntent, reducer::reduce_entries_page_intent, state::EntriesPageState,
};
use crate::pages::feeds_page::{
    intent::FeedsPageIntent, reducer::reduce_feeds_page_intent, state::FeedsPageState,
};
use crate::pages::reader_page::{
    facade::status_shown as reader_status_shown, intent::ReaderPageIntent,
    reducer::reduce_reader_page_intent, state::ReaderPageState,
};
use crate::ui::commands::{EntriesCommand, FeedsCommand, ReaderCommand, ShellCommand, UiCommand};
use crate::ui::recovery::{AppRecovery, required};
use crate::ui::runtime::backend::test_host;
use crate::ui::{UiIntent, failure::TONE_UNKNOWN};

/// One class of failure: the error, the status tone it gets, whether its outcome is unknown, and
/// what it asks of the whole app.
struct Class {
    name: &'static str,
    error: BackendError,
    tone: &'static str,
    unknown: bool,
    app: Option<AppRecovery>,
}

fn classes() -> Vec<Class> {
    vec![
        Class {
            name: "failed",
            error: BackendError::new(ErrorCode::Network, "网络中断"),
            tone: "error",
            unknown: false,
            app: None,
        },
        Class {
            name: "unknown",
            error: BackendError::new(ErrorCode::OutcomeUnknown, "结果未知，请刷新查看"),
            tone: TONE_UNKNOWN,
            unknown: true,
            app: None,
        },
        Class {
            name: "signed out",
            error: BackendError::new(ErrorCode::Unauthenticated, "未登录"),
            tone: "error",
            unknown: false,
            app: Some(AppRecovery::SignIn),
        },
        Class {
            name: "signed out after acceptance",
            error: BackendError::new(ErrorCode::OutcomeUnknown, "登录已失效，结果未知")
                .with_reason(ErrorCode::Unauthenticated),
            tone: TONE_UNKNOWN,
            unknown: true,
            app: Some(AppRecovery::SignIn),
        },
        Class {
            name: "stale build",
            error: BackendError::new(ErrorCode::BuildMismatch, "页面版本与服务器不一致"),
            tone: "error",
            unknown: false,
            app: Some(AppRecovery::Reload),
        },
    ]
}

/// A scripted backend for this test thread; `operation` fails with the class's error.
fn failing(operation: &'static str, class: &Class) -> (Arc<Scripted>, impl Drop) {
    let backend = Scripted::over_new_library();
    backend.fail(operation, class.error.clone());
    let host = test_host::install(backend.clone());
    (backend, host)
}

/// Runs `command`, checks what it asks of the app, and gives its intents.
async fn run(command: UiCommand, class: &Class) -> Vec<UiIntent> {
    let intents = execute_ui_command(command).await;
    assert_eq!(required(&intents), class.app, "{}: what the app must do", class.name);
    intents
}

fn status_is(status: &str, tone: &str, class: &Class) {
    assert!(status.contains(&class.error.message), "{}: {status}", class.name);
    assert_eq!(status.contains("失败"), !class.unknown, "{}: {status}", class.name);
    assert_eq!(tone, class.tone, "{}", class.name);
}

/// A confirmed bulk mark-read that fails releases the bulk markers; an unknown outcome also drops
/// the preview (the write may have applied).
#[tokio::test(flavor = "current_thread")]
async fn a_failed_bulk_mark_read_releases_the_page() {
    for class in classes() {
        let (backend, _host) = failing("mark_read_if_unchanged", &class);
        let preview = backend_preview(&backend).await;
        let mut state = EntriesPageState::new(false);
        reduce_entries_page_intent(&mut state, EntriesPageIntent::BeginBulk { writing: true });
        state.bulk_preview = Some(preview.clone());

        let command = UiCommand::Entries(EntriesCommand::ConfirmMarkRead { preview });
        for intent in run(command, &class).await {
            if let Some(intent) = intent.into_entries_page_intent() {
                reduce_entries_page_intent(&mut state, intent);
            }
        }
        assert!(!state.bulk_busy && !state.bulk_writing, "{}: markers released", class.name);
        assert_eq!(state.bulk_preview.is_none(), class.unknown, "{}: preview", class.name);
        status_is(&state.status, &state.status_tone, &class);
        assert_eq!(backend.calls("mark_read_if_unchanged"), 1, "{}: sent once", class.name);
    }
}

async fn backend_preview(backend: &Scripted) -> rssr_domain::MarkReadPreview {
    use rssr_api::Backend;
    backend.preview_mark_read(&EntryQuery::default()).await.expect("a preview")
}

/// The entries' preferences that cannot be read are unavailable (never saved over), with the
/// class's status.
#[tokio::test(flavor = "current_thread")]
async fn unreadable_preferences_are_unavailable() {
    for class in classes() {
        let (_backend, _host) = failing("bootstrap_entries", &class);
        let mut state = EntriesPageState::new(false);
        let command = UiCommand::Entries(EntriesCommand::Bootstrap {
            load_preferences: true,
            load_feeds: true,
        });
        for intent in run(command, &class).await {
            if let Some(intent) = intent.into_entries_page_intent() {
                reduce_entries_page_intent(&mut state, intent);
            }
        }
        assert!(state.preferences_load.can_load_entries(), "{}: the list may load", class.name);
        assert!(!state.preferences_load.is_loaded(), "{}: never saved over", class.name);
        status_is(&state.status, &state.status_tone, &class);
        // The list that loads afterwards keeps the failure's tone (review C4B-A-R5-01).
        let loaded = EntriesPageIntent::SetEntries {
            entries: Vec::new(),
            archived_count: 0,
            truncated: false,
        };
        reduce_entries_page_intent(&mut state, loaded);
        status_is(&state.status, &state.status_tone, &class);
    }
}

/// A flag change that fails on the list says so with the class's tone.
#[tokio::test(flavor = "current_thread")]
async fn a_failed_flag_change_on_the_list_says_so() {
    for class in classes() {
        let (_backend, _host) = failing("set_entry_read", &class);
        let mut state = EntriesPageState::new(false);
        let command = UiCommand::Entries(EntriesCommand::SetRead {
            entry_id: 1,
            entry_title: "文章".into(),
            read: true,
        });
        for intent in run(command, &class).await {
            if let Some(intent) = intent.into_entries_page_intent() {
                reduce_entries_page_intent(&mut state, intent);
            }
        }
        status_is(&state.status, &state.status_tone, &class);
    }
}

/// Adding a subscription that fails releases the adding marker and keeps the typed address,
/// whatever the class (an unknown outcome too: the page reads again what it shows).
#[tokio::test(flavor = "current_thread")]
async fn a_failed_add_keeps_the_typed_address() {
    for class in classes() {
        let (backend, _host) = failing("add_subscription", &class);
        let mut state = FeedsPageState::new();
        state.feed_url = "https://example.invalid/feed.xml".into();
        let commands = reduce_feeds_page_intent(&mut state, FeedsPageIntent::AddFeedRequested);
        assert!(state.adding_feed_url.is_some());
        let [command] = <[UiCommand; 1]>::try_from(commands).expect("one command");
        for intent in run(command, &class).await {
            if let Some(intent) = intent.into_feeds_page_intent() {
                reduce_feeds_page_intent(&mut state, intent);
            }
        }
        assert_eq!(state.adding_feed_url, None, "{}: marker released", class.name);
        assert_eq!(state.feed_url, "https://example.invalid/feed.xml", "{}: draft", class.name);
        status_is(&state.status, &state.status_tone, &class);
        assert_eq!(backend.calls("add_subscription"), 1, "{}: sent once", class.name);
    }
}

/// A configuration import that fails ends the pending confirmation and keeps the pasted text.
#[tokio::test(flavor = "current_thread")]
async fn a_failed_config_import_keeps_the_text() {
    for class in classes() {
        let (backend, _host) = failing("import_config_json", &class);
        let fingerprint = {
            use rssr_api::Backend;
            backend.load_feeds_snapshot().await.expect("snapshot").fingerprint
        };
        let mut state = FeedsPageState::new();
        state.config_text = "{\"version\":2}".into();
        state.pending_config_import = true;
        let command = UiCommand::Feeds(FeedsCommand::ImportConfig {
            raw: state.config_text.clone(),
            expected: fingerprint,
        });
        for intent in run(command, &class).await {
            if let Some(intent) = intent.into_feeds_page_intent() {
                reduce_feeds_page_intent(&mut state, intent);
            }
        }
        assert!(!state.pending_config_import, "{}: confirmation ended", class.name);
        assert_eq!(state.config_text, "{\"version\":2}", "{}: draft", class.name);
        status_is(&state.status, &state.status_tone, &class);
    }
}

/// Removing a subscription that fails ends the pending confirmation.
#[tokio::test(flavor = "current_thread")]
async fn a_failed_removal_ends_the_confirmation() {
    for class in classes() {
        let (_backend, _host) = failing("remove_feed", &class);
        let mut state = FeedsPageState::new();
        state.pending_delete_feed = Some(1);
        let command =
            UiCommand::Feeds(FeedsCommand::RemoveFeed { feed_id: 1, feed_title: "订阅".into() });
        for intent in run(command, &class).await {
            if let Some(intent) = intent.into_feeds_page_intent() {
                reduce_feeds_page_intent(&mut state, intent);
            }
        }
        assert_eq!(state.pending_delete_feed, None, "{}", class.name);
        status_is(&state.status, &state.status_tone, &class);
    }
}

/// An entry that cannot be read shows the failure in place of the entry.
#[tokio::test(flavor = "current_thread")]
async fn an_unreadable_entry_shows_the_failure() {
    for class in classes() {
        let (_backend, _host) = failing("load_reader_entry", &class);
        let mut state = ReaderPageState::new();
        reduce_reader_page_intent(&mut state, ReaderPageIntent::BeginLoading { entry_id: 7 });
        let command = UiCommand::Reader(ReaderCommand::LoadEntry { entry_id: 7 });
        for intent in run(command, &class).await {
            if let Some(intent) = intent.into_reader_page_intent() {
                reduce_reader_page_intent(&mut state, intent);
            }
        }
        let error = state.error.as_deref().unwrap_or_default();
        assert!(error.contains(&class.error.message), "{}: {error}", class.name);
        assert_eq!(state.error_tone, class.tone, "{}: shown with its tone", class.name);
    }
}

/// The reader's image localization shows nothing, but what its failure asks of the app applies.
#[tokio::test(flavor = "current_thread")]
async fn a_failed_localization_shows_nothing_but_still_recovers() {
    for class in classes() {
        let (_backend, _host) = failing("localize_entry_assets", &class);
        let command = UiCommand::Reader(ReaderCommand::LocalizeEntryAssets { entry_id: 7 });
        let intents = run(command, &class).await;
        assert!(
            intents.into_iter().all(|intent| intent.into_reader_page_intent().is_none()),
            "{}: nothing for the page",
            class.name
        );
    }
}

/// The shell's manual refresh gives its status with the class's tone.
#[tokio::test(flavor = "current_thread")]
async fn a_failed_manual_refresh_says_so() {
    for class in classes() {
        let (_backend, _host) = failing("refresh_all", &class);
        let command = UiCommand::Shell(ShellCommand::ManualRefresh);
        let (message, tone) = run(command, &class)
            .await
            .into_iter()
            .find_map(UiIntent::into_status)
            .expect("a status");
        status_is(&message, &tone, &class);
    }
}

/// A flag change in the reader that fails is shown on screen with the class's tone, an unknown
/// outcome too (review C4B-A-01: the reader showed only errors).
#[tokio::test(flavor = "current_thread")]
async fn a_failed_flag_change_in_the_reader_is_shown() {
    for class in classes() {
        let (_backend, _host) = failing("set_entry_starred", &class);
        let mut state = ReaderPageState::new();
        reduce_reader_page_intent(&mut state, ReaderPageIntent::BeginLoading { entry_id: 7 });
        let command = UiCommand::Reader(ReaderCommand::SetStarred {
            entry_id: 7,
            starred: true,
            via_shortcut: false,
        });
        for intent in run(command, &class).await {
            if let Some(intent) = intent.into_reader_page_intent() {
                reduce_reader_page_intent(&mut state, intent);
            }
        }
        status_is(&state.status, &state.status_tone, &class);
        assert!(reader_status_shown(&state), "{}: shown on screen", class.name);
    }
}

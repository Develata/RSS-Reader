use std::future::Future;

use dioxus::prelude::*;

use crate::bootstrap::ReaderNavigation;
use crate::ui::{ReaderCommand, UiCommand, UiIntent, execute_ui_command};

use super::{reducer::dispatch_reader_page_intent, state::ReaderPageState};

#[derive(Clone, Copy)]
pub(crate) struct ReaderPageSession {
    entry_id: i64,
    state: Signal<ReaderPageState>,
}

impl ReaderPageSession {
    pub(crate) fn new(entry_id: i64, state: Signal<ReaderPageState>) -> Self {
        Self { entry_id, state }
    }

    pub(crate) fn snapshot(self) -> ReaderPageState {
        (self.state)()
    }

    pub(crate) fn previous_entry_target(self) -> Option<i64> {
        previous_entry_target(self.state.read().navigation_state)
    }

    pub(crate) fn next_entry_target(self) -> Option<i64> {
        next_entry_target(self.state.read().navigation_state)
    }

    pub(crate) fn load(self) {
        self.spawn_load(execute_ui_command(UiCommand::Reader(ReaderCommand::LoadEntry {
            entry_id: self.entry_id,
        })));
    }

    fn spawn_load(self, load: impl Future<Output = Vec<UiIntent>> + 'static) {
        // 同步先把「正在加载哪篇」写进状态，再发起异步加载：既让加载中态真的渲染得出来，
        // 也给后续结果提供判断是否过期的依据。
        dispatch_reader_page_intent(
            self.state,
            super::intent::ReaderPageIntent::BeginLoading { entry_id: self.entry_id },
        );
        self.spawn_result(load);
    }

    pub(crate) fn localize_entry_assets(self) {
        self.spawn_ui_command(UiCommand::Reader(ReaderCommand::LocalizeEntryAssets {
            entry_id: self.entry_id,
        }));
    }

    pub(crate) fn toggle_read(self, via_shortcut: bool) {
        self.spawn_ui_command(UiCommand::Reader(ReaderCommand::ToggleRead {
            entry_id: self.entry_id,
            currently_read: self.state.peek().is_read,
            via_shortcut,
        }));
    }

    pub(crate) fn toggle_starred(self, via_shortcut: bool) {
        self.spawn_ui_command(UiCommand::Reader(ReaderCommand::ToggleStarred {
            entry_id: self.entry_id,
            currently_starred: self.state.peek().is_starred,
            via_shortcut,
        }));
    }

    fn spawn_ui_command(self, command: UiCommand) {
        self.spawn_result(execute_ui_command(command));
    }

    fn spawn_result(self, result: impl Future<Output = Vec<UiIntent>> + 'static) {
        let generation = self.state.peek().load_generation;
        spawn(async move {
            let intents = result.await;
            let current = self.state.peek();
            if current.current_entry_id != self.entry_id || current.load_generation != generation {
                return;
            }
            drop(current);
            for intent in intents.into_iter().filter_map(UiIntent::into_reader_page_intent) {
                dispatch_reader_page_intent(self.state, intent);
            }
        });
    }
}

fn previous_entry_target(navigation: ReaderNavigation) -> Option<i64> {
    navigation.previous_unread_entry_id.or(navigation.previous_feed_entry_id)
}

fn next_entry_target(navigation: ReaderNavigation) -> Option<i64> {
    navigation.next_unread_entry_id.or(navigation.next_feed_entry_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::reader_page::{intent::ReaderPageIntent, state::ReaderPageLoadedContent};
    use tokio::sync::oneshot;

    fn loaded(entry_id: i64, title: &str) -> Vec<UiIntent> {
        vec![UiIntent::ReaderPage(ReaderPageIntent::ApplyLoadedContent {
            entry_id,
            content: ReaderPageLoadedContent {
                title: title.into(),
                body_text: format!("{title} body"),
                body_html: None,
                source: "Feed".into(),
                author: None,
                original_url: None,
                published_at: "Unknown".into(),
                navigation_state: ReaderNavigation::default(),
                is_read: false,
                is_starred: false,
            },
        })]
    }

    fn round_trip_with_delayed_result(result: Vec<UiIntent>, is_load: bool) -> ReaderPageState {
        let mut dom = VirtualDom::new(|| rsx! {});
        dom.rebuild_in_place();
        let (sender, receiver) = oneshot::channel();
        let session = dom.in_scope(ScopeId::APP, || {
            let session = ReaderPageSession::new(1, Signal::new(ReaderPageState::new()));
            if is_load {
                session.spawn_load(async { receiver.await.expect("old load result") });
            } else {
                // A command was started while the original A was on screen.
                dispatch_reader_page_intent(
                    session.state,
                    ReaderPageIntent::BeginLoading { entry_id: 1 },
                );
                session.spawn_result(async { receiver.await.expect("old command result") });
            }
            session
        });
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            ReaderPageSession::new(2, session.state)
                .spawn_load(async { loaded(2, "Intermediate B") });
        });
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(session.snapshot().title, "Intermediate B");
            session.spawn_load(async { loaded(1, "Latest A") });
        });
        dom.render_immediate_to_vec();
        sender.send(result).expect("deliver original A result");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || session.snapshot())
    }

    #[test]
    fn older_load_cannot_replace_the_same_entry_after_round_trip() {
        let state = round_trip_with_delayed_result(loaded(1, "Original A"), true);
        assert_eq!(state.title, "Latest A");
        assert_eq!(state.body_text.as_ref(), "Latest A body");
    }

    #[test]
    fn older_status_flags_and_error_cannot_leak_into_a_later_visit() {
        let state = round_trip_with_delayed_result(
            vec![
                UiIntent::ReaderPage(ReaderPageIntent::SetStatus {
                    message: "Old A command".into(),
                    tone: "error".into(),
                }),
                UiIntent::ReaderPage(ReaderPageIntent::PatchEntryFlags {
                    entry_id: 1,
                    is_read: Some(true),
                    is_starred: Some(true),
                }),
                UiIntent::ReaderPage(ReaderPageIntent::SetError {
                    entry_id: 1,
                    error: Some("Old A error".into()),
                }),
            ],
            false,
        );
        assert_eq!(state.title, "Latest A");
        assert!(state.status.is_empty());
        assert!(state.error.is_none());
        assert!(!state.is_read);
        assert!(!state.is_starred);
    }

    #[test]
    fn current_visit_results_apply_without_replacing_the_body() {
        let mut dom = VirtualDom::new(|| rsx! {});
        dom.rebuild_in_place();
        let session = dom.in_scope(ScopeId::APP, || {
            let session = ReaderPageSession::new(1, Signal::new(ReaderPageState::new()));
            session.spawn_load(async { loaded(1, "Current") });
            session
        });
        dom.render_immediate_to_vec();
        let (sender, receiver) = oneshot::channel();
        dom.in_scope(ScopeId::APP, || {
            session.spawn_result(async { receiver.await.expect("current command result") });
        });
        dom.render_immediate_to_vec();
        sender
            .send(vec![UiIntent::ReaderPage(ReaderPageIntent::PatchEntryFlags {
                entry_id: 1,
                is_read: Some(true),
                is_starred: None,
            })])
            .expect("deliver current result");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            let state = session.snapshot();
            assert!(state.is_read);
            assert_eq!(state.title, "Current");
            assert_eq!(state.body_text.as_ref(), "Current body");
        });
    }
}

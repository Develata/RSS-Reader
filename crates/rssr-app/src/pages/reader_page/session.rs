use std::future::Future;

use dioxus::prelude::*;

use crate::bootstrap::ReaderNavigation;
use crate::ui::{ReaderCommand, UiCommand, UiIntent, run_ui_command};

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
        self.spawn_load(run_ui_command(UiCommand::Reader(ReaderCommand::LoadEntry {
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

    /// The page's read-only refresh (C4b item 2): the entry's flags and neighbours, never its
    /// body (no `BeginLoading`: the content, the image localization asked for and the position
    /// stay). A failed load has nothing shown to keep: it is loaded again. While its load is on
    /// its way there is nothing to refresh.
    pub(crate) fn reread(self) {
        let state = self.state.peek();
        if state.current_entry_id != self.entry_id {
            return;
        }
        if state.error.is_some() {
            drop(state);
            self.load();
            return;
        }
        if !state.content_loaded {
            return;
        }
        drop(state);
        self.spawn_reread(run_ui_command(UiCommand::Reader(ReaderCommand::RereadEntry {
            entry_id: self.entry_id,
        })));
    }

    /// Applies a read-only refresh's answer unless the page moved on: another load, or a flag
    /// change applied after it was issued (it may have read the flags before the write).
    fn spawn_reread(self, reread: impl Future<Output = Vec<UiIntent>> + 'static) {
        let (load, flags) = {
            let state = self.state.peek();
            (state.load_generation, state.reread_generation)
        };
        spawn(async move {
            let intents = reread.await;
            let Ok(current) = self.state.try_peek() else { return };
            if current.current_entry_id != self.entry_id
                || current.load_generation != load
                || current.reread_generation != flags
            {
                return;
            }
            drop(current);
            for intent in intents.into_iter().filter_map(UiIntent::into_reader_page_intent) {
                dispatch_reader_page_intent(self.state, intent);
            }
        });
    }

    pub(crate) fn localize_entry_assets(self) {
        self.spawn_ui_command(UiCommand::Reader(ReaderCommand::LocalizeEntryAssets {
            entry_id: self.entry_id,
        }));
    }

    /// The reader's buttons and shortcuts toggle: they ask for the opposite of what it shows.
    pub(crate) fn toggle_read(self, via_shortcut: bool) {
        let Some(read) = self.known_flags().map(|(read, _)| !read) else { return };
        self.spawn_ui_command(UiCommand::Reader(ReaderCommand::SetRead {
            entry_id: self.entry_id,
            read,
            via_shortcut,
        }));
    }

    pub(crate) fn toggle_starred(self, via_shortcut: bool) {
        let Some(starred) = self.known_flags().map(|(_, starred)| !starred) else { return };
        self.spawn_ui_command(UiCommand::Reader(ReaderCommand::SetStarred {
            entry_id: self.entry_id,
            starred,
            via_shortcut,
        }));
    }

    /// 已读与收藏，只在它们是这篇文章存储里的值时给出；加载中按下的切换不发出。
    fn known_flags(self) -> Option<(bool, bool)> {
        let state = self.state.peek();
        state.flags_known(self.entry_id).then_some((state.is_read, state.is_starred))
    }

    fn spawn_ui_command(self, command: UiCommand) {
        self.spawn_result(run_ui_command(command));
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
    navigation.previous_unread_entry_id
}

fn next_entry_target(navigation: ReaderNavigation) -> Option<i64> {
    navigation.next_unread_entry_id
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

    fn reread_answer(is_read: bool, next_unread: Option<i64>) -> Vec<UiIntent> {
        vec![UiIntent::ReaderPage(ReaderPageIntent::ApplyReread {
            entry_id: 1,
            is_read,
            is_starred: true,
            navigation_state: ReaderNavigation {
                next_unread_entry_id: next_unread,
                ..ReaderNavigation::default()
            },
        })]
    }

    fn opened() -> (VirtualDom, ReaderPageSession) {
        let mut dom = VirtualDom::new(|| rsx! {});
        dom.rebuild_in_place();
        let session = dom.in_scope(ScopeId::APP, || {
            let session = ReaderPageSession::new(1, Signal::new(ReaderPageState::new()));
            session.spawn_load(async { loaded(1, "Current") });
            session
        });
        dom.render_immediate_to_vec();
        (dom, session)
    }

    /// The read-only refresh takes the flags and the neighbours; the body, the title, the image
    /// localization asked for and the load stay (C4b item 2).
    #[test]
    fn a_reread_updates_flags_and_neighbours_only() {
        let (mut dom, session) = opened();
        let before = dom.in_scope(ScopeId::APP, || {
            dispatch_reader_page_intent(
                session.state,
                ReaderPageIntent::SetAssetLocalizationRequested,
            );
            session.spawn_reread(async { reread_answer(true, Some(5)) });
            session.snapshot()
        });
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            let state = session.snapshot();
            assert!(state.is_read && state.is_starred);
            assert_eq!(state.navigation_state.next_unread_entry_id, Some(5));
            assert_eq!(state.title, before.title);
            assert_eq!(state.body_text, before.body_text);
            assert!(state.asset_localization_requested);
            assert_eq!(state.load_generation, before.load_generation);
            assert!(state.content_loaded);
        });
    }

    /// A flag the user changed after the read-only refresh was issued wins: the refresh may have
    /// read the flags before the write.
    #[test]
    fn a_flag_change_after_a_reread_was_issued_wins() {
        let (mut dom, session) = opened();
        let (sender, receiver) = oneshot::channel();
        dom.in_scope(ScopeId::APP, || {
            session.spawn_reread(async { receiver.await.expect("the reread's answer") });
            dispatch_reader_page_intent(
                session.state,
                ReaderPageIntent::PatchEntryFlags {
                    entry_id: 1,
                    is_read: Some(true),
                    is_starred: None,
                },
            );
        });
        dom.render_immediate_to_vec();
        sender.send(reread_answer(false, Some(5))).expect("deliver");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            let state = session.snapshot();
            assert!(state.is_read, "the write stays");
            assert_eq!(state.navigation_state.next_unread_entry_id, None, "the answer is dropped");
        });
    }

    /// The refresh does nothing while the entry's load is on its way, and loads again an entry
    /// whose load failed (nothing shown to keep).
    #[tokio::test(flavor = "current_thread")]
    async fn a_reread_waits_for_a_load_and_retries_a_failed_one() {
        use crate::ui::runtime::{backend::test_host, fake_backend::Scripted};

        let backend = Scripted::over_new_library();
        let _host = test_host::install(backend.clone());
        let mut dom = VirtualDom::new(|| rsx! {});
        dom.rebuild_in_place();
        let session = dom.in_scope(ScopeId::APP, || {
            let session = ReaderPageSession::new(1, Signal::new(ReaderPageState::new()));
            dispatch_reader_page_intent(
                session.state,
                ReaderPageIntent::BeginLoading { entry_id: 1 },
            );
            session.reread();
            session
        });
        assert_eq!(backend.calls("load_reader_entry"), 0, "the load is on its way");

        dom.in_scope(ScopeId::APP, || {
            dispatch_reader_page_intent(
                session.state,
                ReaderPageIntent::SetError {
                    entry_id: 1, error: Some("读取文章失败".into())
                },
            );
            session.reread();
            assert_eq!(session.snapshot().load_generation, 2, "loaded again");
        });
        for _ in 0..100 {
            let _ = tokio::time::timeout(std::time::Duration::from_millis(20), dom.wait_for_work())
                .await;
            if backend.calls("load_reader_entry") == 1 {
                break;
            }
        }
        assert_eq!(backend.calls("load_reader_entry"), 1);
    }
}

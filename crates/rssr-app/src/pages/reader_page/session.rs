use std::future::Future;

use dioxus::prelude::*;

use crate::ui::{ReaderCommand, UiCommand, UiIntent, run_ui_command};
use rssr_domain::EntryNavigation as ReaderNavigation;

use super::{
    intent::ReaderPageIntent,
    reducer::reduce_reader_page_intent,
    result_lifecycle::{ReaderResultScope, RereadPlan, plan_reread},
    state::{ReaderFlag, ReaderPageState},
};

pub(super) fn dispatch_reader_page_intent(
    mut state: Signal<ReaderPageState>,
    intent: ReaderPageIntent,
) {
    state.with_mut(|state| reduce_reader_page_intent(state, intent));
}

#[derive(Clone, Copy)]
pub(crate) struct ReaderPageSession {
    entry_id: i64,
    state: Signal<ReaderPageState>,
    owner: ScopeId,
}

impl ReaderPageSession {
    pub(crate) fn new(entry_id: i64, state: Signal<ReaderPageState>) -> Self {
        Self { entry_id, state, owner: dioxus::core::current_scope_id() }
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
        let scope = self.state.with_peek(|state| ReaderResultScope::capture(self.entry_id, state));
        spawn(async move {
            let intents = load.await;
            let Ok(current) = self.state.try_peek() else { return };
            if !scope.is_current(&current) {
                return;
            }
            drop(current);
            for intent in intents.into_iter().filter_map(UiIntent::into_reader_page_intent) {
                dispatch_reader_page_intent(self.state, intent);
            }
            // A refresh asked for while the load was on its way follows it (review
            // C4B-B-R3-03).
            if self.state.peek().reread_wanted {
                dispatch_reader_page_intent(
                    self.state,
                    super::intent::ReaderPageIntent::RereadWanted(false),
                );
                self.reread();
            }
        });
    }

    /// The page's read-only refresh (C4b item 2): the entry's flags and neighbours, never its
    /// body (no `BeginLoading`: the content, the image localization asked for and the position
    /// stay). A failed load has nothing shown to keep: it is loaded again, as a read only — the
    /// image localization (a write) is not asked for; opening the entry again asks for it
    /// (review C4B-B-02). While its load is on its way the refresh waits for it (review
    /// C4B-B-R3-03).
    pub(crate) fn reread(self) {
        let plan = self.state.with_peek(|state| plan_reread(self.entry_id, state));
        match plan {
            RereadPlan::Ignore => {}
            RereadPlan::RecoverLoad => {
                self.load();
                dispatch_reader_page_intent(
                    self.state,
                    ReaderPageIntent::SetAssetLocalizationRequested,
                );
            }
            RereadPlan::AfterLoad => {
                dispatch_reader_page_intent(self.state, ReaderPageIntent::RereadWanted(true));
            }
            RereadPlan::ReadFlagsAndNeighbours => {
                self.spawn_reread(run_ui_command(UiCommand::Reader(ReaderCommand::RereadEntry {
                    entry_id: self.entry_id,
                })));
            }
        }
    }

    /// Applies a read-only refresh's answer unless the page moved on: another load, a later
    /// refresh, or a flag change applied after it was issued (it may have read the flags before
    /// the write).
    fn spawn_reread(self, reread: impl Future<Output = Vec<UiIntent>> + 'static) {
        dispatch_reader_page_intent(self.state, super::intent::ReaderPageIntent::RereadIssued);
        let scope =
            self.state.with_peek(|state| ReaderResultScope::capture_reread(self.entry_id, state));
        spawn(async move {
            let intents = reread.await;
            let Ok(current) = self.state.try_peek() else { return };
            if !scope.is_current(&current) {
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
        let Some(read) = self.known_flags(ReaderFlag::Read).map(|(read, _)| !read) else { return };
        self.spawn_flag_write(
            ReaderFlag::Read,
            UiCommand::Reader(ReaderCommand::SetRead {
                entry_id: self.entry_id,
                read,
                via_shortcut,
            }),
        );
    }

    pub(crate) fn toggle_starred(self, via_shortcut: bool) {
        let Some(starred) = self.known_flags(ReaderFlag::Starred).map(|(_, starred)| !starred)
        else {
            return;
        };
        self.spawn_flag_write(
            ReaderFlag::Starred,
            UiCommand::Reader(ReaderCommand::SetStarred {
                entry_id: self.entry_id,
                starred,
                via_shortcut,
            }),
        );
    }

    /// 已读与收藏，只在它们是这篇文章存储里的值时给出；加载中按下的切换不发出。
    fn known_flags(self, flag: ReaderFlag) -> Option<(bool, bool)> {
        let state = self.state.peek();
        state.can_write_flag(self.entry_id, flag).then_some((state.is_read, state.is_starred))
    }

    /// User-confirmed flag writes belong to the app. Reading and automatic image
    /// localization still belong to the visit; neither should outlive navigation.
    fn spawn_flag_write(self, flag: ReaderFlag, command: UiCommand) {
        let shell = consume_context::<crate::ui::AppShellState>();
        // Set the gate before spawning: buttons and shortcuts can arrive before
        // the next render. The existing visit scope also owns its release.
        dispatch_reader_page_intent(self.state, ReaderPageIntent::FlagWriteStarted(flag));
        let scope = self.state.with_peek(|state| ReaderResultScope::capture(self.entry_id, state));
        shell.spawn_in_shell(async move {
            let intents = run_ui_command(command).await;
            let same_visit = self.state.try_peek().is_ok_and(|current| scope.is_current(&current));
            if same_visit {
                dioxus::core::Runtime::current().in_scope(self.owner, || {
                    for intent in intents.into_iter().filter_map(UiIntent::into_reader_page_intent)
                    {
                        dispatch_reader_page_intent(self.state, intent);
                    }
                    dispatch_reader_page_intent(
                        self.state,
                        ReaderPageIntent::FlagWriteFinished(flag),
                    );
                });
            } else {
                for intent in intents.into_iter().filter_map(UiIntent::into_reader_page_intent) {
                    if let Some(failure) = intent.failure() {
                        shell.report_job(failure.message(), failure.tone().to_owned());
                    } else if let super::intent::ReaderPageIntent::SetStatus { message, tone } =
                        intent
                    {
                        shell.report_job(message, tone);
                    }
                }
                shell.reread_now();
            }
        });
    }

    fn spawn_ui_command(self, command: UiCommand) {
        self.spawn_result(run_ui_command(command));
    }

    fn spawn_result(self, result: impl Future<Output = Vec<UiIntent>> + 'static) {
        let scope = self.state.with_peek(|state| ReaderResultScope::capture(self.entry_id, state));
        spawn(async move {
            let intents = result.await.into_iter().filter_map(UiIntent::into_reader_page_intent);
            let accepted = self.state.with_peek(|state| scope.complete(state, intents));
            for intent in accepted {
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

    #[derive(Clone, Copy)]
    struct WriteProbe {
        visible: Signal<bool>,
        session: Signal<Option<ReaderPageSession>>,
        scope: Signal<Option<ScopeId>>,
    }

    fn write_test_app() -> Element {
        let shell = crate::ui::use_app_shell_state();
        use_context_provider(|| shell);
        let visible = use_signal(|| true);
        let session = use_signal(|| None);
        let scope = use_signal(|| None);
        use_context_provider(|| WriteProbe { visible, session, scope });
        rsx! { if visible() { WriteReader {} } }
    }

    #[component]
    fn WriteReader() -> Element {
        let mut probe = use_context::<WriteProbe>();
        let state = use_signal(ReaderPageState::new);
        use_hook(|| {
            let session = ReaderPageSession::new(1, state);
            dispatch_reader_page_intent(state, ReaderPageIntent::BeginLoading { entry_id: 1 });
            for intent in
                loaded(1, "Current").into_iter().filter_map(UiIntent::into_reader_page_intent)
            {
                dispatch_reader_page_intent(state, intent);
            }
            probe.session.set(Some(session));
            probe.scope.set(Some(dioxus::core::current_scope_id()));
        });
        rsx! {}
    }

    async fn until_write(dom: &mut VirtualDom, mut done: impl FnMut() -> bool) {
        for _ in 0..150 {
            let _ = tokio::time::timeout(std::time::Duration::from_millis(20), dom.wait_for_work())
                .await;
            dom.render_immediate_to_vec();
            if dom.in_scope(ScopeId::APP, &mut done) {
                return;
            }
        }
        panic!("reader operation did not finish");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reader_buttons_and_shortcuts_share_a_flag_gate_and_failure_releases_it() {
        use crate::ui::runtime::{backend::test_host, fake_backend::Scripted};
        for (operation, flag) in
            [("set_entry_read", ReaderFlag::Read), ("set_entry_starred", ReaderFlag::Starred)]
        {
            let backend = Scripted::over_new_library();
            let hold = backend.hold(operation);
            backend.fail_once(
                operation,
                rssr_api::BackendError::new(rssr_api::ErrorCode::Network, "flag response failed"),
            );
            let _host = test_host::install(backend.clone());
            let mut dom = VirtualDom::new(write_test_app);
            dom.rebuild_in_place();
            let probe = dom.in_scope(ScopeId::APP, consume_context::<WriteProbe>);
            let session = probe.session.peek().unwrap();
            let toggle = |shortcut| match flag {
                ReaderFlag::Read => session.toggle_read(shortcut),
                ReaderFlag::Starred => session.toggle_starred(shortcut),
            };
            let body = session.snapshot().body_text;
            dom.in_scope(probe.scope.peek().unwrap(), || {
                // Both entry points can run before the next render.
                toggle(false);
                toggle(true);
                toggle(false);
            });
            until_write(&mut dom, || hold.waiting() == 1).await;
            assert_eq!(backend.calls(operation), 1);
            assert!(session.snapshot().flag_write_pending(flag));
            hold.release();
            until_write(&mut dom, || !session.snapshot().flag_write_pending(flag)).await;
            assert!(session.snapshot().status.contains("flag response failed"));
            assert!(std::sync::Arc::ptr_eq(&body, &session.snapshot().body_text));

            dom.in_scope(probe.scope.peek().unwrap(), || toggle(true));
            until_write(&mut dom, || hold.waiting() == 1).await;
            assert_eq!(backend.calls(operation), 2, "a completed failure must allow a new action");
            hold.release();
            until_write(&mut dom, || !session.snapshot().flag_write_pending(flag)).await;
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn an_old_flag_completion_cannot_release_the_new_visits_gate() {
        use crate::ui::runtime::{backend::test_host, fake_backend::Scripted};
        let backend = Scripted::over_new_library();
        let hold = backend.hold("set_entry_starred");
        let _host = test_host::install(backend.clone());
        let mut dom = VirtualDom::new(write_test_app);
        dom.rebuild_in_place();
        let (mut probe, shell) = dom.in_scope(ScopeId::APP, || {
            (consume_context::<WriteProbe>(), consume_context::<crate::ui::AppShellState>())
        });
        let first = probe.session.peek().unwrap();
        dom.in_scope(probe.scope.peek().unwrap(), || first.toggle_starred(false));
        until_write(&mut dom, || hold.waiting() == 1).await;
        dom.in_scope(ScopeId::APP, || probe.visible.set(false));
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || probe.visible.set(true));
        dom.render_immediate_to_vec();
        let next = probe.session.peek().unwrap();
        dom.in_scope(probe.scope.peek().unwrap(), || next.toggle_starred(false));
        until_write(&mut dom, || hold.waiting() == 2).await;
        hold.release();
        until_write(&mut dom, || shell.job_notice().is_some()).await;
        assert!(next.snapshot().flag_write_pending(ReaderFlag::Starred));
        dom.in_scope(probe.scope.peek().unwrap(), || next.toggle_starred(true));
        assert_eq!(backend.calls("set_entry_starred"), 2);
        hold.release();
        until_write(&mut dom, || !next.snapshot().flag_write_pending(ReaderFlag::Starred)).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reader_flag_writes_survive_navigation_without_changing_a_new_visit() {
        use crate::ui::runtime::{backend::test_host, fake_backend::Scripted};
        for operation in ["set_entry_read", "set_entry_starred"] {
            let backend = Scripted::over_new_library();
            let hold = backend.hold(operation);
            backend.fail_once(
                operation,
                rssr_api::BackendError::new(
                    rssr_api::ErrorCode::OutcomeUnknown,
                    "late reader flag",
                ),
            );
            let _host = test_host::install(backend.clone());
            let mut dom = VirtualDom::new(write_test_app);
            dom.rebuild_in_place();
            let (mut probe, shell) = dom.in_scope(ScopeId::APP, || {
                (consume_context::<WriteProbe>(), consume_context::<crate::ui::AppShellState>())
            });
            let session = probe.session.peek().unwrap();
            dom.in_scope(probe.scope.peek().unwrap(), || {
                if operation == "set_entry_read" {
                    session.toggle_read(false);
                } else {
                    session.toggle_starred(false);
                }
            });
            until_write(&mut dom, || hold.waiting() == 1).await;
            dom.in_scope(ScopeId::APP, || probe.visible.set(false));
            dom.render_immediate_to_vec();
            assert_eq!(hold.abandoned(), 0, "{operation}: navigation must not cancel a flag write");
            dom.in_scope(ScopeId::APP, || probe.visible.set(true));
            dom.render_immediate_to_vec();
            let reopened = probe.session.peek().unwrap();
            hold.release();
            until_write(&mut dom, || shell.job_notice().is_some()).await;
            dom.in_scope(ScopeId::APP, || {
                assert!(shell.job_notice().unwrap().message.contains("late reader flag"));
                assert!(shell.library_revision() > 0);
                let state = reopened.snapshot();
                assert!(state.status.is_empty(), "the new visit owns its status");
                assert!(!state.is_read && !state.is_starred);
                assert_eq!(state.body_text.as_ref(), "Current body");
            });
            assert_eq!(backend.calls(operation), 1, "no uncertain write retry");
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn closing_the_app_ends_reader_flag_waits() {
        use crate::ui::runtime::{backend::test_host, fake_backend::Scripted};
        let backend = Scripted::over_new_library();
        let hold = backend.hold("set_entry_starred");
        let _host = test_host::install(backend);
        let mut dom = VirtualDom::new(write_test_app);
        dom.rebuild_in_place();
        let probe = dom.in_scope(ScopeId::APP, consume_context::<WriteProbe>);
        let session = probe.session.peek().unwrap();
        dom.in_scope(probe.scope.peek().unwrap(), || session.toggle_starred(false));
        until_write(&mut dom, || hold.waiting() == 1).await;
        drop(dom);
        assert_eq!(hold.abandoned(), 1);
        assert_eq!(hold.waiting(), 0);
    }

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

    #[tokio::test(flavor = "current_thread")]
    async fn a_recovered_reread_clears_its_obsolete_failure() {
        use crate::ui::runtime::{backend::test_host, fake_backend::Scripted};

        let backend = Scripted::over_new_library();
        backend.fail(
            "load_reader_entry",
            rssr_api::BackendError::new(rssr_api::ErrorCode::Unavailable, "temporary read failure"),
        );
        let _host = test_host::install(backend);
        let (mut dom, session) = opened();
        let body = session.snapshot().body_text;
        dom.in_scope(ScopeId::APP, || session.reread());
        until_write(&mut dom, || session.snapshot().status.contains("temporary read failure"))
            .await;

        dom.in_scope(ScopeId::APP, || {
            session.spawn_reread(async { reread_answer(true, Some(5)) });
        });
        until_write(&mut dom, || session.snapshot().is_read).await;
        let state = session.snapshot();
        assert!(
            state.status.is_empty(),
            "a successful reread must retire its earlier read failure"
        );
        assert!(std::sync::Arc::ptr_eq(&body, &state.body_text));
        assert_eq!(state.navigation_state.next_unread_entry_id, Some(5));
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

    /// Two read-only refreshes whose answers come back in the other order: the earlier one's
    /// answer does not replace the later one's (review C4B-B-01).
    #[test]
    fn an_earlier_reread_answering_last_does_not_replace_a_later_one() {
        let (mut dom, session) = opened();
        let (first, first_answer) = oneshot::channel();
        let (second, second_answer) = oneshot::channel();
        dom.in_scope(ScopeId::APP, || {
            session.spawn_reread(async { first_answer.await.expect("the earlier reread") });
            session.spawn_reread(async { second_answer.await.expect("the later reread") });
        });
        second.send(reread_answer(true, Some(5))).expect("the later answer");
        dom.render_immediate_to_vec();
        first.send(reread_answer(false, None)).expect("the earlier answer");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            let state = session.snapshot();
            assert!(state.is_read);
            assert_eq!(state.navigation_state.next_unread_entry_id, Some(5));
        });
    }

    /// Loading a failed entry again for a read-only refresh does not ask for its image
    /// localization (a write); opening an entry does (review C4B-B-02).
    #[tokio::test(flavor = "current_thread")]
    async fn a_recovery_load_does_not_ask_for_the_image_localization() {
        use crate::ui::runtime::{backend::test_host, fake_backend::Scripted};

        let backend = Scripted::over_new_library();
        let _host = test_host::install(backend.clone());
        let mut dom = VirtualDom::new(|| rsx! {});
        dom.rebuild_in_place();
        dom.in_scope(ScopeId::APP, || {
            let session = ReaderPageSession::new(1, Signal::new(ReaderPageState::new()));
            session.load();
            assert!(!session.snapshot().asset_localization_requested, "an opened entry asks");
            dispatch_reader_page_intent(
                session.state,
                ReaderPageIntent::SetError {
                    entry_id: 1, error: Some("读取文章失败".into())
                },
            );
            session.reread();
            let state = session.snapshot();
            assert_eq!(state.load_generation, 2, "loaded again");
            assert!(state.asset_localization_requested, "a recovery load does not ask");
        });
    }

    /// A refresh asked for while the entry's load is on its way is not lost: it follows the load
    /// (review C4B-B-R3-03).
    #[tokio::test(flavor = "current_thread")]
    async fn a_reread_asked_for_during_a_load_follows_it() {
        use crate::ui::runtime::{backend::test_host, fake_backend::Scripted};

        let backend = Scripted::over_new_library();
        let _host = test_host::install(backend.clone());
        let mut dom = VirtualDom::new(|| rsx! {});
        dom.rebuild_in_place();
        let (sender, receiver) = oneshot::channel();
        let session = dom.in_scope(ScopeId::APP, || {
            let session = ReaderPageSession::new(1, Signal::new(ReaderPageState::new()));
            session.spawn_load(async { receiver.await.expect("the load") });
            session.reread();
            session
        });
        dom.render_immediate_to_vec();
        assert_eq!(backend.calls("load_reader_entry"), 0, "it waits for the load");
        sender.send(loaded(1, "Current")).expect("deliver the load");
        for _ in 0..50 {
            let _ = tokio::time::timeout(std::time::Duration::from_millis(20), dom.wait_for_work())
                .await;
            if backend.calls("load_reader_entry") == 1 {
                break;
            }
        }
        assert_eq!(backend.calls("load_reader_entry"), 1, "the refresh followed the load");
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(session.snapshot().title, "Current", "the body stays");
            assert!(!session.snapshot().reread_wanted);
        });
    }
}

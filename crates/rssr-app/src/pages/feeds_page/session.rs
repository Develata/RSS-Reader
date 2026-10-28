use std::{future::Future, time::Duration};

use dioxus::prelude::*;

use super::{
    intent::FeedsPageIntent,
    reducer::dispatch_feeds_page_intent,
    request_policy::{FeedsRequestPlan, RefreshFeedback},
    state::FeedsPageState,
};
use crate::ui::{AppShellState, UiCommand, UiIntent, execute_ui_command};

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct FeedsPageSession {
    state: Signal<FeedsPageState>,
    shell: AppShellState,
    query_generation: Signal<u64>,
}

impl FeedsPageSession {
    pub(crate) fn new(
        state: Signal<FeedsPageState>,
        shell: AppShellState,
        query_generation: Signal<u64>,
    ) -> Self {
        Self { state, shell, query_generation }
    }

    pub(crate) fn snapshot(self) -> FeedsPageState {
        (self.state)()
    }

    pub(crate) fn dismiss_edit(self) -> bool {
        if self.state.peek().edit_feed_id.is_none() {
            return false;
        }
        // An in-flight save consumes Back too; the reducer keeps its form open
        // until completion rather than letting navigation discard the result.
        self.dispatch_intent(FeedsPageIntent::EditClosed);
        true
    }

    pub(crate) fn reload_tick(self) -> u64 {
        self.state.read().reload_tick
    }

    pub(crate) fn is_refreshing(self) -> bool {
        self.shell.refresh_state().is_refreshing()
    }

    pub(crate) fn set_feed_url(self, value: String) {
        self.dispatch_intent(FeedsPageIntent::FeedUrlChanged(value));
    }

    pub(crate) fn set_config_text(self, value: String) {
        self.dispatch_intent(FeedsPageIntent::ConfigTextChanged(value));
    }

    pub(crate) fn set_opml_text(self, value: String) {
        self.dispatch_intent(FeedsPageIntent::OpmlTextChanged(value));
    }

    pub(crate) fn pending_delete_feed(self) -> Option<i64> {
        self.state.read().pending_delete_feed
    }

    pub(crate) fn load_snapshot(self) {
        self.dispatch_intent(FeedsPageIntent::LoadRequested);
    }

    pub(super) fn dispatch_intent(self, intent: FeedsPageIntent) {
        self.dispatch_intent_with(intent, execute_ui_command);
    }

    fn dispatch_intent_with<F: Future<Output = Vec<UiIntent>> + 'static>(
        mut self,
        intent: FeedsPageIntent,
        mut execute: impl FnMut(UiCommand) -> F,
    ) {
        let commands = dispatch_feeds_page_intent(self.state, intent);
        for command in commands {
            let plan = FeedsRequestPlan::for_command(&command, *self.query_generation.peek());
            if plan == FeedsRequestPlan::ManualRefresh {
                self.shell.manual_refresh();
                continue;
            }
            if let Some(generation) = plan.query_generation() {
                self.query_generation.set(generation);
            }
            let task = execute(command);
            self.spawn_result_with(plan, task, crate::ui::wait_for_refresh_feedback);
        }
    }

    fn spawn_result_with<F, W>(
        self,
        plan: FeedsRequestPlan,
        task: F,
        wait: impl FnOnce(Duration) -> W + 'static,
    ) where
        F: Future<Output = Vec<UiIntent>> + 'static,
        W: Future<Output = ()> + 'static,
    {
        spawn(async move {
            let intents = task.await;
            if !plan.accepts(*self.query_generation.peek()) {
                return;
            }
            for intent in intents.into_iter().filter_map(UiIntent::into_feeds_page_intent) {
                self.dispatch_intent(intent);
            }
            if plan == FeedsRequestPlan::SingleRefresh {
                let feedback = self.state.with(|state| {
                    RefreshFeedback::capture(
                        state.status_revision,
                        &state.status,
                        &state.status_tone,
                    )
                });
                wait(feedback.delay).await;
                self.dispatch_intent(feedback.clear);
            }
        });
    }

    pub(crate) fn is_delete_pending_for(self, feed_id: i64) -> bool {
        self.pending_delete_feed() == Some(feed_id)
    }

    pub(crate) fn add_feed(self) {
        self.dispatch_intent(FeedsPageIntent::AddFeedRequested);
    }

    pub(crate) fn refresh_all(self) {
        self.dispatch_intent(FeedsPageIntent::RefreshAllRequested);
    }

    pub(crate) fn export_config(self) {
        self.dispatch_intent(FeedsPageIntent::ExportConfigRequested);
    }

    pub(crate) fn import_config(self) {
        self.dispatch_intent(FeedsPageIntent::ImportConfigRequested);
    }

    pub(crate) fn export_opml(self) {
        self.dispatch_intent(FeedsPageIntent::ExportOpmlRequested);
    }

    pub(crate) fn import_opml(self) {
        self.dispatch_intent(FeedsPageIntent::ImportOpmlRequested);
    }

    pub(crate) fn refresh_feed(self, feed_id: i64, feed_title: String) {
        self.dispatch_intent(FeedsPageIntent::RefreshFeedRequested { feed_id, feed_title });
    }

    pub(crate) fn remove_feed(self, feed_id: i64, feed_title: String) {
        self.dispatch_intent(FeedsPageIntent::RemoveFeedRequested { feed_id, feed_title });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        pages::feeds_page::intent::FeedsPageSnapshot,
        ui::{FeedsCommand, use_app_shell_state},
    };
    use tokio::sync::oneshot;

    fn harness() -> Element {
        let shell = use_app_shell_state();
        let state = use_signal(FeedsPageState::new);
        let generation = use_signal(|| 0);
        use_context_provider(|| FeedsPageSession::new(state, shell, generation));
        rsx! {}
    }

    fn setup() -> (VirtualDom, FeedsPageSession) {
        let mut dom = VirtualDom::new(harness);
        dom.rebuild_in_place();
        let session = dom.in_scope(ScopeId::APP, consume_context);
        (dom, session)
    }

    #[test]
    fn dismiss_edit_consumes_back_without_leaving_a_save_in_flight() {
        let (dom, session) = setup();
        dom.in_scope(ScopeId::APP, || {
            assert!(!session.dismiss_edit());
            let mut state = session.state;
            state.with_mut(|state| {
                state.edit_feed_id = Some(42);
                state.edit_busy = true;
            });
            assert!(session.dismiss_edit(), "loading dialog closes before navigation");
            assert!(state.peek().edit_feed_id.is_none());
            state.with_mut(|state| {
                state.edit_feed_id = Some(42);
                state.edit_details = Some(rssr_domain::FeedEditDetails {
                    feed_id: 42,
                    url: "https://example.com/feed".into(),
                    original_title: None,
                    custom_title: Some("Draft".into()),
                });
                state.edit_busy = true;
            });
            assert!(session.dismiss_edit(), "saving consumes Back without navigation");
            assert_eq!(state.peek().edit_feed_id, Some(42));
            state.with_mut(|state| state.edit_busy = false);
            assert!(session.dismiss_edit(), "an idle draft can close");
            assert!(!session.dismiss_edit(), "the following Back can navigate normally");
        });
    }

    fn start_add(session: FeedsPageSession, expected_url: &str) -> oneshot::Sender<bool> {
        let (sender, receiver) = oneshot::channel();
        let mut receiver = Some(receiver);
        session.dispatch_intent_with(FeedsPageIntent::AddFeedRequested, |command| {
            let UiCommand::Feeds(FeedsCommand::AddFeed { raw_url, .. }) = command else {
                panic!("expected subscription command")
            };
            assert_eq!(raw_url, expected_url);
            let receiver = receiver.take().expect("only one command per submission");
            async move {
                vec![UiIntent::FeedsPage(FeedsPageIntent::AddFeedFinished {
                    saved: receiver.await.expect("controlled subscription completion"),
                })]
            }
        });
        sender
    }

    #[test]
    fn adding_subscription_gates_repeat_submit_and_preserves_next_address() {
        let (mut dom, session) = setup();
        let sender = dom.in_scope(ScopeId::APP, || {
            session.set_feed_url("https://first.example/feed".into());
            let sender = start_add(session, "https://first.example/feed");
            // Repeated Enter may arrive before the disabled button has rendered.
            session.dispatch_intent_with(
                FeedsPageIntent::AddFeedRequested,
                |_| -> std::future::Ready<Vec<UiIntent>> {
                    panic!("a repeated request must not construct another command future")
                },
            );
            session.set_feed_url("https://next.example/订阅".into());
            sender
        });
        dom.render_immediate_to_vec();
        sender.send(true).expect("complete submitted subscription");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(session.snapshot().feed_url, "https://next.example/订阅");
            assert!(session.snapshot().adding_feed_url.is_none());
        });
        let next = dom.in_scope(ScopeId::APP, || start_add(session, "https://next.example/订阅"));
        next.send(true).expect("complete next subscription");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            assert!(session.snapshot().feed_url.is_empty());
            assert!(session.snapshot().adding_feed_url.is_none());
        });
    }

    #[test]
    fn failed_subscription_retains_address_and_releases_gate_for_retry() {
        let (mut dom, session) = setup();
        let sender = dom.in_scope(ScopeId::APP, || {
            session.set_feed_url("https://retry.example/feed".into());
            start_add(session, "https://retry.example/feed")
        });
        sender.send(false).expect("fail submitted subscription");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(session.snapshot().feed_url, "https://retry.example/feed");
            assert!(session.snapshot().adding_feed_url.is_none());
        });
        let retry = dom.in_scope(ScopeId::APP, || start_add(session, "https://retry.example/feed"));
        retry.send(true).expect("successful retry");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || assert!(session.snapshot().feed_url.is_empty()));
    }

    fn snapshot(count: usize) -> Vec<UiIntent> {
        vec![UiIntent::FeedsPage(FeedsPageIntent::SnapshotLoaded(Ok(FeedsPageSnapshot {
            feeds: Vec::new(),
            feed_count: count,
            entry_count: count * 10,
            unread_count: 0,
            archived_count: 0,
        })))]
    }

    fn older_snapshot_after_latest(older_result: Vec<UiIntent>) -> FeedsPageState {
        let (mut dom, session) = setup();
        let (older_sender, older_receiver) = oneshot::channel();
        let (latest_sender, latest_receiver) = oneshot::channel();
        dom.in_scope(ScopeId::APP, || {
            for receiver in [older_receiver, latest_receiver] {
                let mut receiver = Some(receiver);
                session.dispatch_intent_with(FeedsPageIntent::LoadRequested, |command| {
                    assert!(matches!(command, UiCommand::Feeds(FeedsCommand::LoadSnapshot)));
                    let receiver = receiver.take().expect("one query");
                    async move { receiver.await.expect("controlled snapshot") }
                });
            }
        });
        dom.render_immediate_to_vec();
        latest_sender.send(snapshot(2)).expect("latest snapshot");
        dom.render_immediate_to_vec();
        older_sender.send(older_result).expect("older snapshot");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || session.snapshot())
    }

    #[test]
    fn older_feed_snapshot_cannot_replace_latest_feed_counts() {
        let state = older_snapshot_after_latest(snapshot(1));
        assert_eq!(state.feed_count, 2);
        assert_eq!(state.entry_count, 20);
    }

    #[test]
    fn older_feed_snapshot_error_cannot_replace_latest_status() {
        let state = older_snapshot_after_latest(vec![UiIntent::FeedsPage(
            FeedsPageIntent::SnapshotLoaded(Err("older read failed".into())),
        )]);
        assert_eq!(state.feed_count, 2);
        assert!(state.status.is_empty());
        assert_eq!(state.status_tone, "info");
    }

    #[test]
    fn snapshot_identity_is_published_before_synchronous_executor_construction() {
        use std::{cell::Cell, rc::Rc};
        let (mut dom, session) = setup();
        let constructed = Cell::new(false);
        let polled = Rc::new(Cell::new(false));
        dom.in_scope(ScopeId::APP, || {
            session.dispatch_intent_with(FeedsPageIntent::LoadRequested, |_| {
                constructed.set(true);
                assert_eq!(*session.query_generation.peek(), 1);
                let polled = Rc::clone(&polled);
                async move {
                    polled.set(true);
                    snapshot(4)
                }
            });
            assert!(constructed.get());
            assert!(!polled.get());
        });
        dom.render_immediate_to_vec();
        assert!(polled.get());
        dom.in_scope(ScopeId::APP, || assert_eq!(session.snapshot().feed_count, 4));
    }

    #[test]
    fn newer_snapshot_does_not_discard_an_older_write_completion() {
        let (mut dom, session) = setup();
        let sender = dom.in_scope(ScopeId::APP, || {
            session.set_feed_url("https://fixture.example/feed".into());
            let sender = start_add(session, "https://fixture.example/feed");
            assert_eq!(*session.query_generation.peek(), 0, "writes do not invalidate reads");
            session.dispatch_intent_with(FeedsPageIntent::LoadRequested, |_| {
                std::future::ready(snapshot(3))
            });
            session.dispatch_intent_with(FeedsPageIntent::LoadRequested, |_| {
                std::future::ready(snapshot(4))
            });
            sender
        });
        dom.render_immediate_to_vec();
        sender.send(true).unwrap();
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(session.snapshot().feed_count, 4);
            assert_eq!(*session.query_generation.peek(), 2);
            assert!(session.snapshot().adding_feed_url.is_none());
            assert!(session.snapshot().feed_url.is_empty());
        });
    }

    #[test]
    fn stale_snapshot_discards_its_entire_batch_including_global_status_and_reload() {
        let state = older_snapshot_after_latest(vec![
            UiIntent::SetStatus { message: "stale global error".into(), tone: "error".into() },
            UiIntent::FeedsPage(FeedsPageIntent::BumpReload),
        ]);
        assert_eq!(state.feed_count, 2);
        assert_eq!(state.reload_tick, 0);
        assert_eq!(state.status_revision, 0);
        assert!(state.status.is_empty());
    }

    #[test]
    fn refresh_all_uses_the_shell_gate_without_the_page_executor() {
        let (dom, session) = setup();
        dom.in_scope(ScopeId::APP, || {
            session.dispatch_intent_with(
                FeedsPageIntent::RefreshAllRequested,
                |_| -> std::future::Ready<Vec<UiIntent>> {
                    panic!("manual refresh belongs to the shared shell")
                },
            );
            assert!(session.is_refreshing());
            assert_eq!(*session.query_generation.peek(), 0);
        });
        // Drop before polling the shell's real command; this test only checks routing.
        drop(dom);
    }

    #[test]
    fn feedback_captures_after_all_results_and_cannot_clear_a_newer_status() {
        for replacement in ["none", "same-message", "same-revision"] {
            let (mut dom, session) = setup();
            let (sender, receiver) = oneshot::channel();
            dom.in_scope(ScopeId::APP, || {
                session.spawn_result_with(
                    FeedsRequestPlan::SingleRefresh,
                    std::future::ready(vec![
                        UiIntent::FeedsPage(FeedsPageIntent::SetStatus {
                            message: "first".into(),
                            tone: "info".into(),
                        }),
                        UiIntent::FeedsPage(FeedsPageIntent::BumpReload),
                        UiIntent::SetStatus { message: "last".into(), tone: "error".into() },
                    ]),
                    move |delay| {
                        assert_eq!(delay, Duration::from_secs(6));
                        let state = session.snapshot();
                        assert_eq!(state.status, "last");
                        assert_eq!(state.status_revision, 2);
                        assert_eq!(state.reload_tick, 1);
                        async move { receiver.await.unwrap() }
                    },
                );
            });
            dom.render_immediate_to_vec();
            let expected = dom.in_scope(ScopeId::APP, || {
                match replacement {
                    "same-message" => session.dispatch_intent(FeedsPageIntent::SetStatus {
                        message: "last".into(),
                        tone: "error".into(),
                    }),
                    "same-revision" => {
                        session.dispatch_intent(FeedsPageIntent::RemoveFeedRequested {
                            feed_id: 1,
                            feed_title: "feed".into(),
                        });
                        assert_eq!(session.snapshot().status_revision, 2);
                    }
                    _ => {}
                }
                session.snapshot()
            });
            sender.send(()).unwrap();
            dom.render_immediate_to_vec();
            dom.in_scope(ScopeId::APP, || {
                let mut wanted = expected;
                if replacement == "none" {
                    wanted.status.clear();
                }
                assert_eq!(session.snapshot(), wanted);
            });
        }
    }

    #[test]
    fn ordinary_commands_do_not_start_feedback_timers() {
        let (mut dom, session) = setup();
        dom.in_scope(ScopeId::APP, || {
            session.spawn_result_with(
                FeedsRequestPlan::Other,
                std::future::ready(vec![UiIntent::SetStatus {
                    message: "exported".into(),
                    tone: "info".into(),
                }]),
                |_| -> std::future::Ready<()> { panic!("only a single refresh schedules expiry") },
            );
        });
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || assert_eq!(session.snapshot().status, "exported"));
    }

    #[test]
    fn disposing_page_cancels_pending_results_and_feedback_waits() {
        let (mut dom, session) = setup();
        let (result_sender, result_receiver) = oneshot::channel();
        let (timer_sender, timer_receiver) = oneshot::channel();
        dom.in_scope(ScopeId::APP, || {
            session.spawn_result_with(
                FeedsRequestPlan::Other,
                async { result_receiver.await.unwrap() },
                |_| std::future::ready(()),
            );
            session.spawn_result_with(
                FeedsRequestPlan::SingleRefresh,
                std::future::ready(Vec::new()),
                move |_| async { timer_receiver.await.unwrap() },
            );
        });
        dom.render_immediate_to_vec();
        assert!(!result_sender.is_closed());
        assert!(!timer_sender.is_closed());
        drop(dom);
        assert!(result_sender.is_closed());
        assert!(timer_sender.is_closed());
    }
}

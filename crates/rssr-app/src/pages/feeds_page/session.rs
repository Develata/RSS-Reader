use std::future::Future;

use dioxus::prelude::*;

use super::{intent::FeedsPageIntent, reducer::dispatch_feeds_page_intent, state::FeedsPageState};
use crate::ui::{
    AppShellState, FeedsCommand, ShellCommand, UiCommand, UiIntent, execute_ui_command,
};

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

    pub(crate) fn reload_tick(self) -> u64 {
        self.state.read().reload_tick
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
            if matches!(command, UiCommand::Shell(ShellCommand::ManualRefresh)) {
                self.shell.manual_refresh();
                continue;
            }
            // Refresh completion and feed mutations can start overlapping snapshots.
            // Only reads use this token; writes must still publish their completion.
            let generation =
                matches!(command, UiCommand::Feeds(FeedsCommand::LoadSnapshot)).then(|| {
                    self.query_generation.with_mut(|generation| {
                        *generation = generation.wrapping_add(1);
                        *generation
                    })
                });
            let single_refresh =
                matches!(command, UiCommand::Feeds(FeedsCommand::RefreshFeed { .. }));
            let task = execute(command);
            spawn(async move {
                let intents = task.await;
                if generation.is_some_and(|generation| *self.query_generation.peek() != generation)
                {
                    return;
                }
                for intent in intents.into_iter().filter_map(UiIntent::into_feeds_page_intent) {
                    self.dispatch_intent(intent);
                }
                if single_refresh {
                    let (revision, message, failed) = self.state.with(|state| {
                        (state.status_revision, state.status.clone(), state.status_tone == "error")
                    });
                    crate::ui::wait_for_refresh_feedback(std::time::Duration::from_secs(
                        if failed { 6 } else { 3 },
                    ))
                    .await;
                    self.dispatch_intent(FeedsPageIntent::ClearRefreshStatus { revision, message });
                }
            });
        }
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
    use crate::{pages::feeds_page::intent::FeedsPageSnapshot, ui::use_app_shell_state};
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
}

use std::future::Future;

use dioxus::prelude::*;

use super::{intent::FeedsPageIntent, reducer::dispatch_feeds_page_intent, state::FeedsPageState};
use crate::{
    theme::ThemeController,
    ui::{AppShellState, FeedsCommand, ShellCommand, UiCommand, UiIntent, run_ui_command},
};

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct FeedsPageSession {
    state: Signal<FeedsPageState>,
    shell: AppShellState,
    theme: ThemeController,
    query_generation: Signal<u64>,
}

impl FeedsPageSession {
    pub(crate) fn new(
        state: Signal<FeedsPageState>,
        shell: AppShellState,
        theme: ThemeController,
        query_generation: Signal<u64>,
    ) -> Self {
        Self { state, shell, theme, query_generation }
    }

    pub(crate) fn snapshot(self) -> FeedsPageState {
        (self.state)()
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
        self.dispatch_intent_with(intent, run_ui_command);
    }

    fn dispatch_intent_with<F: Future<Output = Vec<UiIntent>> + 'static>(
        mut self,
        intent: FeedsPageIntent,
        mut execute: impl FnMut(UiCommand) -> F,
    ) {
        if let FeedsPageIntent::SettingsReplaced(settings) = intent {
            // 主题、自定义 CSS、密度与字号由全局设置驱动，导入后立即生效，不必等设置页或重启。
            self.theme.store(settings);
            return;
        }
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
            // Jobs (refreshing one subscription, adding one) are the app's: leaving the page does
            // not stop the wait, and their end re-reads every page (C4b item 3).
            let job = matches!(
                command,
                UiCommand::Feeds(FeedsCommand::RefreshFeed { .. } | FeedsCommand::AddFeed { .. })
            );
            // 配置导入可能替换全局设置：任务交给 App 作用域，离开订阅页不会把它取消在
            // “已提交、未回执”的中途，新外观照样生效（审计 E1）。
            let shell_owned =
                job || matches!(command, UiCommand::Feeds(FeedsCommand::ImportConfig { .. }));
            let task = execute(command);
            let completion = async move {
                let intents = task.await;
                // Every page re-reads what the job or the import may have changed, including a
                // settings page opened meanwhile (review C4B-B-03).
                if shell_owned {
                    self.shell.reread_now();
                }
                if generation.is_some_and(|generation| *self.query_generation.peek() != generation)
                {
                    return;
                }
                let intents: Vec<_> =
                    intents.into_iter().filter_map(UiIntent::into_feeds_page_intent).collect();
                // The results reach only the page that started the operation, this instance
                // (its state); a page opened since has its own (R2-02). When it was left, only
                // what concerns the whole app applies: the replaced settings, and the job's
                // result as the shell's notice.
                if self.state.try_peek().is_err() {
                    let mut notice = None;
                    for intent in intents {
                        match intent {
                            FeedsPageIntent::SettingsReplaced(settings) => {
                                self.theme.store(settings)
                            }
                            FeedsPageIntent::SetStatus { message, tone } => {
                                notice = Some((message, tone))
                            }
                            FeedsPageIntent::Failed(failure) => {
                                notice = Some((failure.message(), failure.tone().to_string()))
                            }
                            FeedsPageIntent::FeedCandidates { .. } => {
                                notice = Some((
                                    "添加订阅：网页上有多个订阅地址，请回到订阅页重新添加并选择。"
                                        .to_string(),
                                    "info".to_string(),
                                ))
                            }
                            _ => {}
                        }
                    }
                    if let Some((message, tone)) = notice {
                        self.shell.report_job(message, tone);
                    }
                    return;
                }
                for intent in intents {
                    self.dispatch_intent(intent);
                }
                if single_refresh {
                    let (revision, message, failed) = self.state.with(|state| {
                        let tone = state.status_tone.as_str();
                        let lasting = tone == "error" || tone == crate::ui::failure::TONE_UNKNOWN;
                        (state.status_revision, state.status.clone(), lasting)
                    });
                    crate::ui::wait_for_refresh_feedback(std::time::Duration::from_secs(
                        if failed { 6 } else { 3 },
                    ))
                    .await;
                    // The wait is the app's too: the page may have been left meanwhile.
                    if self.state.try_peek().is_ok() {
                        self.dispatch_intent(FeedsPageIntent::ClearRefreshStatus {
                            revision,
                            message,
                        });
                    }
                }
            };
            if shell_owned {
                self.shell.spawn_in_shell(completion);
            } else {
                spawn(completion);
            }
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
    use rssr_domain::UserSettings;
    use tokio::sync::oneshot;

    fn harness() -> Element {
        let shell = use_app_shell_state();
        let state = use_signal(FeedsPageState::new);
        let theme = crate::theme::use_theme_controller();
        let generation = use_signal(|| 0);
        use_context_provider(|| theme);
        use_context_provider(|| FeedsPageSession::new(state, shell, theme, generation));
        rsx! {}
    }

    fn setup() -> (VirtualDom, FeedsPageSession) {
        let mut dom = VirtualDom::new(harness);
        dom.rebuild_in_place();
        let session = dom.in_scope(ScopeId::APP, consume_context);
        (dom, session)
    }

    #[derive(Clone, Copy)]
    struct UnmountProbe {
        show_page: Signal<bool>,
        page: Signal<Option<(FeedsPageSession, ScopeId)>>,
    }

    fn app_with_unmountable_page() -> Element {
        let shell = use_app_shell_state();
        let theme = crate::theme::use_theme_controller();
        let show_page = use_signal(|| true);
        let page = use_signal(|| None);
        use_context_provider(|| shell);
        use_context_provider(|| theme);
        use_context_provider(|| UnmountProbe { show_page, page });
        rsx! {
            if show_page() {
                FeedsPageProbe {}
            }
        }
    }

    #[component]
    fn FeedsPageProbe() -> Element {
        let shell = use_context::<AppShellState>();
        let theme = use_context::<ThemeController>();
        let mut probe = use_context::<UnmountProbe>();
        let state = use_signal(FeedsPageState::new);
        let generation = use_signal(|| 0);
        use_hook(|| {
            let session = FeedsPageSession::new(state, shell, theme, generation);
            probe.page.set(Some((session, dioxus::core::current_scope_id())));
        });
        rsx! {}
    }

    /// 导入已提交、结果还没返回时离开订阅页：任务不被取消，新外观照样生效，
    /// 已卸载页面的回执被丢弃而不是写进已释放的状态（独立评审的反例）。
    #[tokio::test(flavor = "current_thread")]
    async fn config_import_still_applies_settings_after_leaving_the_page() {
        let mut dom = VirtualDom::new(app_with_unmountable_page);
        dom.rebuild_in_place();
        let probe = dom.in_scope(ScopeId::APP, consume_context::<UnmountProbe>);
        let (session, page_scope) = probe.page.peek().expect("feeds page mounted");
        let (sender, receiver) = oneshot::channel();
        let mut receiver = Some(receiver);
        dom.in_scope(page_scope, || {
            session.dispatch_intent(shown_snapshot());
            session.dispatch_intent(FeedsPageIntent::PendingConfigImportSet(true));
            session.dispatch_intent_with(FeedsPageIntent::ImportConfigRequested, |command| {
                assert!(matches!(command, UiCommand::Feeds(FeedsCommand::ImportConfig { .. })));
                let receiver = receiver.take().expect("one import command");
                async move { receiver.await.expect("import result") }
            });
        });
        dom.render_immediate_to_vec();

        let mut show_page = probe.show_page;
        dom.in_scope(ScopeId::APP, || show_page.set(false));
        dom.render_immediate_to_vec();
        assert!(session.state.try_peek().is_err(), "the feeds page must be unmounted");

        let replaced = UserSettings {
            theme: rssr_domain::ThemeMode::Dark,
            custom_css: "body { color: red; }".into(),
            ..UserSettings::default()
        };
        sender
            .send(vec![
                UiIntent::FeedsPage(FeedsPageIntent::PendingConfigImportSet(false)),
                UiIntent::FeedsPage(FeedsPageIntent::BumpReload),
                UiIntent::FeedsPage(FeedsPageIntent::SettingsReplaced(replaced.clone())),
                UiIntent::FeedsPage(FeedsPageIntent::SetStatus {
                    message: "配置包已导入".into(),
                    tone: "info".into(),
                }),
            ])
            .expect("deliver import result");
        dom.render_immediate_to_vec();

        let theme = dom.in_scope(ScopeId::APP, consume_context::<ThemeController>);
        assert_eq!(*theme.settings.peek(), replaced);
    }

    /// 导入配置包后外观立即生效，不依赖设置页是否挂载（审计 E1）。
    #[test]
    fn config_import_applies_replaced_settings_to_the_global_theme() {
        let (mut dom, session) = setup();
        let replaced = UserSettings {
            theme: rssr_domain::ThemeMode::Dark,
            custom_css: "body { color: red; }".into(),
            ..UserSettings::default()
        };
        let reply = replaced.clone();
        dom.in_scope(ScopeId::APP, || {
            session.dispatch_intent(shown_snapshot());
            session.dispatch_intent(FeedsPageIntent::PendingConfigImportSet(true));
            let mut reply = Some(reply);
            session.dispatch_intent_with(FeedsPageIntent::ImportConfigRequested, |command| {
                assert!(matches!(command, UiCommand::Feeds(FeedsCommand::ImportConfig { .. })));
                let settings = reply.take().expect("one import command");
                async move {
                    vec![
                        UiIntent::FeedsPage(FeedsPageIntent::PendingConfigImportSet(false)),
                        UiIntent::FeedsPage(FeedsPageIntent::BumpReload),
                        UiIntent::FeedsPage(FeedsPageIntent::SettingsReplaced(settings)),
                    ]
                }
            });
        });
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            let theme = consume_context::<ThemeController>();
            assert_eq!(*theme.settings.peek(), replaced);
            assert!(!session.snapshot().pending_config_import);
            // Every page re-reads what the import changed, a settings page opened meanwhile
            // too (review C4B-B-03).
            assert_eq!(session.shell.library_revision(), 1);
        });
    }

    fn start_add(session: FeedsPageSession, expected_url: &str) -> oneshot::Sender<bool> {
        let (sender, receiver) = oneshot::channel();
        let mut receiver = Some(receiver);
        session.dispatch_intent_with(FeedsPageIntent::AddFeedRequested, |command| {
            let UiCommand::Feeds(FeedsCommand::AddFeed { raw_url, token, .. }) = command else {
                panic!("expected subscription command")
            };
            assert_eq!(raw_url, expected_url);
            let receiver = receiver.take().expect("only one command per submission");
            async move {
                vec![UiIntent::FeedsPage(FeedsPageIntent::AddFeedFinished {
                    token,
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

    /// The page showing an empty library: an import from it expects that configuration.
    fn shown_snapshot() -> FeedsPageIntent {
        snapshot(0).into_iter().find_map(UiIntent::into_feeds_page_intent).expect("snapshot")
    }

    fn snapshot(count: u64) -> Vec<UiIntent> {
        vec![UiIntent::FeedsPage(FeedsPageIntent::SnapshotLoaded(Ok(FeedsPageSnapshot {
            feeds: Vec::new(),
            feed_count: count,
            entry_count: count * 10,
            fingerprint: rssr_domain::ConfigFingerprint::compute(
                Vec::new(),
                &rssr_domain::UserSettings::default(),
            ),
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
            FeedsPageIntent::SnapshotLoaded(Err(crate::ui::failure::plain("older read failed"))),
        )]);
        assert_eq!(state.feed_count, 2);
        assert!(state.status.is_empty());
        assert_eq!(state.status_tone, "info");
    }

    /// Runs the app's tasks until `done` holds.
    async fn run_until(dom: &mut VirtualDom, what: &str, mut done: impl FnMut() -> bool) {
        for _ in 0..300 {
            let _ = tokio::time::timeout(std::time::Duration::from_millis(20), dom.wait_for_work())
                .await;
            dom.render_immediate_to_vec();
            if dom.in_scope(ScopeId::APP, &mut done) {
                return;
            }
        }
        panic!("{what}");
    }

    /// The app with the subscriptions page, a scripted backend, and the page's session.
    fn app_with_jobs() -> (VirtualDom, UnmountProbe, AppShellState, FeedsPageSession, ScopeId) {
        let mut dom = VirtualDom::new(app_with_unmountable_page);
        dom.rebuild_in_place();
        let (probe, shell) = dom.in_scope(ScopeId::APP, || (consume_context(), consume_context()));
        let probe: UnmountProbe = probe;
        let (session, scope) = probe.page.peek().expect("feeds page mounted");
        (dom, probe, shell, session, scope)
    }

    /// A job started on the page goes on when the page is left (the wait is the app's): its end
    /// re-reads every page, and its result is the shell's notice, not the gone page's
    /// (C4b item 3).
    #[tokio::test(flavor = "current_thread")]
    async fn a_job_outlives_its_page() {
        use crate::ui::runtime::{backend::test_host, fake_backend::Scripted};

        let backend = Scripted::over_new_library();
        let hold = backend.hold("refresh_feed");
        let _host = test_host::install(backend.clone());
        let (mut dom, probe, shell, session, scope) = app_with_jobs();
        dom.in_scope(scope, || session.refresh_feed(1, "甲".into()));
        run_until(&mut dom, "the job waits", || hold.waiting() == 1).await;

        let mut show_page = probe.show_page;
        dom.in_scope(ScopeId::APP, || show_page.set(false));
        dom.render_immediate_to_vec();
        assert!(session.state.try_peek().is_err(), "the page was left");

        hold.release();
        run_until(&mut dom, "the job's notice", || shell.job_notice().is_some()).await;
        assert_eq!(hold.abandoned(), 0, "leaving the page did not stop the wait");
        assert_eq!(backend.calls("refresh_feed"), 1);
        dom.in_scope(ScopeId::APP, || assert_eq!(shell.library_revision(), 1, "pages re-read"));
    }

    /// A job that ends after its page was left and opened again does not touch the new page: its
    /// draft, its gate and its status are its own (R2-02).
    #[tokio::test(flavor = "current_thread")]
    async fn a_job_ending_after_the_page_was_reopened_leaves_the_new_page_alone() {
        use crate::ui::runtime::{backend::test_host, fake_backend::Scripted};

        let backend = Scripted::over_new_library();
        let hold = backend.hold("add_subscription");
        let _host = test_host::install(backend.clone());
        let (mut dom, probe, shell, first, scope) = app_with_jobs();
        dom.in_scope(scope, || {
            first.set_feed_url("https://a.example/feed.xml".into());
            first.add_feed();
        });
        run_until(&mut dom, "the add waits", || hold.waiting() == 1).await;

        let mut show_page = probe.show_page;
        dom.in_scope(ScopeId::APP, || show_page.set(false));
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || show_page.set(true));
        dom.render_immediate_to_vec();
        let (second, scope) = probe.page.peek().expect("the page opened again");
        assert_ne!(second.state, first.state, "a new page");
        dom.in_scope(scope, || second.set_feed_url("https://b.example/feed.xml".into()));

        hold.release();
        run_until(&mut dom, "the add's notice", || shell.job_notice().is_some()).await;
        dom.in_scope(scope, || {
            let state = second.snapshot();
            assert_eq!(state.feed_url, "https://b.example/feed.xml");
            assert_eq!(state.adding_feed_url, None);
            assert!(state.status.is_empty(), "{}", state.status);
            assert!(state.feed_candidates.is_none());
        });
    }

    /// Closing the app stops waiting for its jobs: the call is dropped (the server's job goes on;
    /// the client's polling and backoff end with the call).
    #[tokio::test(flavor = "current_thread")]
    async fn closing_the_app_stops_waiting_for_its_jobs() {
        use crate::ui::runtime::{backend::test_host, fake_backend::Scripted};

        let backend = Scripted::over_new_library();
        let hold = backend.hold("refresh_feed");
        let _host = test_host::install(backend.clone());
        let (mut dom, _probe, _shell, session, scope) = app_with_jobs();
        dom.in_scope(scope, || session.refresh_feed(1, "甲".into()));
        run_until(&mut dom, "the job waits", || hold.waiting() == 1).await;
        drop(dom);
        assert_eq!(hold.waiting(), 0);
        assert_eq!(hold.abandoned(), 1);
    }
}

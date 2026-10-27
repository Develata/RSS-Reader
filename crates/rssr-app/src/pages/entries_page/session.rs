//! Entries page session: owns UI task scopes and publishes reducer intents.
//! Read coordination, preference saving and article mutations stay in private
//! modules; callers use this single session interface. See session/README.md.

mod articles;
mod queries;
mod workspace;

use dioxus::prelude::*;

use self::workspace::WorkspaceSaveQueue;
use super::{
    intent::EntriesPageIntent, reducer::dispatch_entries_page_intent,
    request_lifecycle::EntriesRequestLifecycle, state::EntriesPageState,
};
use crate::ui::{UiCommand, UiIntent, spawn_projected_ui_command};

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct EntriesPageSession {
    feed_id: Option<i64>,
    state: Signal<EntriesPageState>,
    requests: Signal<EntriesRequestLifecycle>,
    workspace_saves: Signal<WorkspaceSaveQueue>,
    shell: crate::ui::AppShellState,
    owner: ScopeId,
}

/// Allocate each coordinator once for this page, independently of render count.
pub(super) fn use_entries_page_session(feed_id: Option<i64>) -> EntriesPageSession {
    let state = use_signal(EntriesPageState::new);
    let requests = use_signal(EntriesRequestLifecycle::default);
    let workspace_saves = use_signal(WorkspaceSaveQueue::default);
    EntriesPageSession::new(feed_id, state, requests, workspace_saves)
}

impl EntriesPageSession {
    fn new(
        feed_id: Option<i64>,
        state: Signal<EntriesPageState>,
        requests: Signal<EntriesRequestLifecycle>,
        workspace_saves: Signal<WorkspaceSaveQueue>,
    ) -> Self {
        Self {
            feed_id,
            state,
            requests,
            workspace_saves,
            shell: consume_context(),
            owner: dioxus::core::current_scope_id(),
        }
    }

    pub(crate) fn snapshot(self) -> EntriesPageState {
        (self.state)()
    }

    /// 借用状态而不是克隆整份。
    ///
    /// `snapshot()` 会克隆 `EntriesPageState`（集合以 Arc 共享，但 status / selected_feed_urls 仍需复制），
    /// 用在「每次状态变化都要跑一遍」的投影里代价白扔。`Readable::with` 同样会建立订阅。
    pub(crate) fn with_state<R>(self, read: impl FnOnce(&EntriesPageState) -> R) -> R {
        self.state.with(read)
    }

    pub(crate) fn feed_id(self) -> Option<i64> {
        self.feed_id
    }

    pub(crate) fn dispatch(mut self, intent: EntriesPageIntent) {
        let supersedes_query = matches!(intent, EntriesPageIntent::PatchEntryFlags { .. })
            && self.state.peek().query_in_flight;
        // A read of the preferences still on its way when a save of them landed, or ended with an
        // unknown outcome, may have read the store before the save: its answer would put the
        // saved filters back, or tell an old baseline for an unknown one. A new read replaces it
        // (C4b item 2, R2-03; review C4B-B-R3-02).
        let supersedes_reread = matches!(
            intent,
            EntriesPageIntent::WorkspaceSaved(_) | EntriesPageIntent::WorkspaceSaveUnknown
        ) && self.state.peek().bootstrap_in_flight;
        dispatch_entries_page_intent(self.state, intent);
        if supersedes_query {
            // 写入成功时还在途的查询可能读的是写入前的存储：丢弃它的结果，由 reducer 递增的
            // requery_revision 触发的新查询接替（审计 A3）。
            self.requests.with_mut(EntriesRequestLifecycle::invalidate_query);
        }
        if supersedes_reread {
            self.reread();
        }
    }

    fn spawn_ui_command(self, command: UiCommand) {
        spawn_projected_ui_command(command, UiIntent::into_entries_page_intent, move |intent| {
            self.dispatch(intent)
        });
    }
}

#[cfg(test)]
mod tests;

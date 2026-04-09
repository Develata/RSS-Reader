use dioxus::prelude::*;

use super::{
    commands::{FeedsPageCommand, FeedsPageCommandOutcome},
    dispatch::execute_command,
    queries::{FeedsPageSnapshot, load_feeds_page_snapshot},
    state::FeedsPageState,
};

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct FeedsPageSession {
    state: Signal<FeedsPageState>,
}

impl FeedsPageSession {
    pub(crate) fn new(state: Signal<FeedsPageState>) -> Self {
        Self { state }
    }

    pub(crate) fn reload_tick(self) -> u64 {
        (self.state)().reload_tick
    }

    pub(crate) fn feed_url(self) -> String {
        (self.state)().feed_url
    }

    pub(crate) fn set_feed_url(mut self, value: String) {
        self.state.with_mut(|state| state.feed_url = value);
    }

    pub(crate) fn config_text(self) -> String {
        (self.state)().config_text
    }

    pub(crate) fn set_config_text(mut self, value: String) {
        self.state.with_mut(|state| {
            state.pending_config_import = false;
            state.config_text = value;
        });
    }

    pub(crate) fn opml_text(self) -> String {
        (self.state)().opml_text
    }

    pub(crate) fn set_opml_text(mut self, value: String) {
        self.state.with_mut(|state| state.opml_text = value);
    }

    pub(crate) fn pending_config_import(self) -> bool {
        (self.state)().pending_config_import
    }

    pub(crate) fn pending_delete_feed(self) -> Option<i64> {
        (self.state)().pending_delete_feed
    }

    pub(crate) fn feeds(self) -> Vec<rssr_domain::FeedSummary> {
        (self.state)().feeds
    }

    pub(crate) fn feed_count(self) -> usize {
        (self.state)().feed_count
    }

    pub(crate) fn entry_count(self) -> usize {
        (self.state)().entry_count
    }

    pub(crate) fn status(self) -> String {
        (self.state)().status
    }

    pub(crate) fn status_tone(self) -> String {
        (self.state)().status_tone
    }

    pub(crate) async fn load_snapshot(self) {
        match load_feeds_page_snapshot().await {
            Ok(snapshot) => self.apply_snapshot(snapshot),
            Err(err) => self.set_status_error(err.to_string()),
        }
    }

    pub(crate) fn run(self, command: FeedsPageCommand) {
        spawn(async move {
            let outcome = execute_command(command).await;
            self.apply_command_outcome(outcome);
        });
    }

    pub(crate) fn set_status_error(mut self, message: String) {
        self.state.with_mut(|state| {
            state.status = message;
            state.status_tone = "error".to_string();
        });
    }

    fn apply_snapshot(mut self, snapshot: FeedsPageSnapshot) {
        self.state.with_mut(|state| {
            state.feed_count = snapshot.feed_count;
            state.entry_count = snapshot.entry_count;
            state.feeds = snapshot.feeds;
        });
    }

    fn apply_command_outcome(mut self, outcome: FeedsPageCommandOutcome) {
        self.state.with_mut(|state| {
            if let Some(next) = outcome.patch.feed_url {
                state.feed_url = next;
            }
            if let Some(next) = outcome.patch.config_text {
                state.config_text = next;
            }
            if let Some(next) = outcome.patch.opml_text {
                state.opml_text = next;
            }
            if let Some(next) = outcome.patch.pending_config_import {
                state.pending_config_import = next;
            }
            if let Some(next) = outcome.patch.pending_delete_feed {
                state.pending_delete_feed = next;
            }
            state.status = outcome.status_message;
            state.status_tone = outcome.status_tone.to_string();
            if outcome.reload {
                state.reload_tick += 1;
            }
        });
    }

    pub(crate) fn is_delete_pending_for(self, feed_id: i64) -> bool {
        self.pending_delete_feed() == Some(feed_id)
    }

    pub(crate) fn add_feed(self) {
        self.run(FeedsPageCommand::AddFeed { raw_url: self.feed_url() });
    }

    pub(crate) fn refresh_all(self) {
        self.run(FeedsPageCommand::RefreshAll);
    }

    pub(crate) fn export_config(self) {
        self.run(FeedsPageCommand::ExportConfig);
    }

    pub(crate) fn import_config(self) {
        self.run(FeedsPageCommand::ImportConfig {
            raw: self.config_text(),
            confirmed: self.pending_config_import(),
        });
    }

    pub(crate) fn export_opml(self) {
        self.run(FeedsPageCommand::ExportOpml);
    }

    pub(crate) fn import_opml(self) {
        self.run(FeedsPageCommand::ImportOpml { raw: self.opml_text() });
    }

    pub(crate) fn refresh_feed(self, feed_id: i64, feed_title: String) {
        self.run(FeedsPageCommand::RefreshFeed { feed_id, feed_title });
    }

    pub(crate) fn remove_feed(self, feed_id: i64, feed_title: String) {
        self.run(FeedsPageCommand::RemoveFeed {
            feed_id,
            feed_title,
            confirmed: self.is_delete_pending_for(feed_id),
        });
    }

    pub(crate) fn paste_feed_url_result(self, result: Result<Option<String>, String>) {
        match result {
            Ok(Some(text)) => self.set_feed_url(text),
            Ok(None) => {}
            Err(err) => self.set_status_error(format!("读取系统剪贴板失败：{err}")),
        }
    }
}

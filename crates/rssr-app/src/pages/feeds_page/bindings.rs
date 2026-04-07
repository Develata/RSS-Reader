use dioxus::prelude::*;
use rssr_domain::FeedSummary;

use super::{commands::FeedsPageCommandOutcome, queries::FeedsPageSnapshot};

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct FeedsPageBindings {
    feed_url: Signal<String>,
    config_text: Signal<String>,
    opml_text: Signal<String>,
    pending_config_import: Signal<bool>,
    pending_delete_feed: Signal<Option<i64>>,
    reload_tick: Signal<u64>,
    feeds: Signal<Vec<FeedSummary>>,
    feed_count: Signal<usize>,
    entry_count: Signal<usize>,
    status: Signal<String>,
    status_tone: Signal<String>,
}

impl FeedsPageBindings {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        feed_url: Signal<String>,
        config_text: Signal<String>,
        opml_text: Signal<String>,
        pending_config_import: Signal<bool>,
        pending_delete_feed: Signal<Option<i64>>,
        reload_tick: Signal<u64>,
        feeds: Signal<Vec<FeedSummary>>,
        feed_count: Signal<usize>,
        entry_count: Signal<usize>,
        status: Signal<String>,
        status_tone: Signal<String>,
    ) -> Self {
        Self {
            feed_url,
            config_text,
            opml_text,
            pending_config_import,
            pending_delete_feed,
            reload_tick,
            feeds,
            feed_count,
            entry_count,
            status,
            status_tone,
        }
    }

    pub(crate) fn apply_snapshot(mut self, snapshot: FeedsPageSnapshot) {
        self.feed_count.set(snapshot.feed_count);
        self.entry_count.set(snapshot.entry_count);
        self.feeds.set(snapshot.feeds);
    }

    pub(crate) fn set_status_error(mut self, message: String) {
        self.status.set(message);
        self.status_tone.set("error".to_string());
    }

    pub(crate) fn apply_command_outcome(mut self, outcome: FeedsPageCommandOutcome) {
        apply_patch(self.feed_url, outcome.patch.feed_url);
        apply_patch(self.config_text, outcome.patch.config_text);
        apply_patch(self.opml_text, outcome.patch.opml_text);
        apply_patch(self.pending_config_import, outcome.patch.pending_config_import);
        apply_patch(self.pending_delete_feed, outcome.patch.pending_delete_feed);
        self.status.set(outcome.status_message);
        self.status_tone.set(outcome.status_tone.to_string());
        if outcome.reload {
            let mut reload_tick = self.reload_tick;
            reload_tick += 1;
        }
    }
}

fn apply_patch<T: 'static>(mut signal: Signal<T>, next: Option<T>) {
    if let Some(next) = next {
        signal.set(next);
    }
}

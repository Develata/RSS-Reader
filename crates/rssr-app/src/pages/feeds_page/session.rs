use dioxus::prelude::*;

use super::{intent::FeedsPageIntent, reducer::dispatch_feeds_page_intent, state::FeedsPageState};
use crate::ui::{UiIntent, spawn_projected_ui_command};

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct FeedsPageSession {
    state: Signal<FeedsPageState>,
}

impl FeedsPageSession {
    pub(crate) fn new(state: Signal<FeedsPageState>) -> Self {
        Self { state }
    }

    pub(crate) fn snapshot(self) -> FeedsPageState {
        (self.state)()
    }

    pub(crate) fn reload_tick(self) -> u64 {
        self.snapshot().reload_tick
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
        self.snapshot().pending_delete_feed
    }

    pub(crate) fn load_snapshot(self) {
        self.dispatch_intent(FeedsPageIntent::LoadRequested);
    }

    fn dispatch_intent(self, intent: FeedsPageIntent) {
        let commands = dispatch_feeds_page_intent(self.state, intent);
        for command in commands {
            spawn_projected_ui_command(command, UiIntent::into_feeds_page_intent, move |intent| {
                self.dispatch_intent(intent);
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

    pub(crate) fn paste_feed_url(self) {
        self.dispatch_intent(FeedsPageIntent::PasteFeedUrlRequested);
    }
}

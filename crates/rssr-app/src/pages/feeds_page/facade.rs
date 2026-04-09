use rssr_domain::FeedSummary;

use super::{session::FeedsPageSession, state::FeedsPageState};

#[derive(Clone, PartialEq)]
pub(crate) struct FeedsPageFacade {
    session: FeedsPageSession,
    snapshot: FeedsPageState,
}

impl FeedsPageFacade {
    pub(crate) fn new(session: FeedsPageSession, snapshot: FeedsPageState) -> Self {
        Self { session, snapshot }
    }

    pub(crate) fn feed_url(&self) -> &str {
        &self.snapshot.feed_url
    }

    pub(crate) fn set_feed_url(&self, value: String) {
        self.session.set_feed_url(value);
    }

    pub(crate) fn config_text(&self) -> &str {
        &self.snapshot.config_text
    }

    pub(crate) fn set_config_text(&self, value: String) {
        self.session.set_config_text(value);
    }

    pub(crate) fn opml_text(&self) -> &str {
        &self.snapshot.opml_text
    }

    pub(crate) fn set_opml_text(&self, value: String) {
        self.session.set_opml_text(value);
    }

    pub(crate) fn is_config_import_pending(&self) -> bool {
        self.snapshot.pending_config_import
    }

    pub(crate) fn config_import_button_variant(&self) -> &'static str {
        if self.is_config_import_pending() { "danger" } else { "secondary" }
    }

    pub(crate) fn config_import_button_label(&self) -> &'static str {
        if self.is_config_import_pending() { "确认覆盖导入" } else { "导入配置" }
    }

    pub(crate) fn config_import_state(&self) -> &'static str {
        if self.is_config_import_pending() { "confirm" } else { "idle" }
    }

    pub(crate) fn is_delete_pending_for(&self, feed_id: i64) -> bool {
        self.session.is_delete_pending_for(feed_id)
    }

    pub(crate) fn empty_feeds_message(&self) -> &'static str {
        "还没有订阅，先添加一个 feed URL。"
    }

    pub(crate) fn feeds_list_state(&self) -> &'static str {
        if self.feeds().is_empty() { "empty" } else { "populated" }
    }

    pub(crate) fn remove_feed_button_variant(&self, feed_id: i64) -> &'static str {
        if self.is_delete_pending_for(feed_id) { "danger" } else { "danger-outline" }
    }

    pub(crate) fn remove_feed_button_label(&self, feed_id: i64) -> &'static str {
        if self.is_delete_pending_for(feed_id) { "确认删除" } else { "删除订阅" }
    }

    pub(crate) fn remove_feed_state(&self, feed_id: i64) -> &'static str {
        if self.is_delete_pending_for(feed_id) { "confirm" } else { "idle" }
    }

    pub(crate) fn feeds(&self) -> &[FeedSummary] {
        &self.snapshot.feeds
    }

    pub(crate) fn total_feed_count(&self) -> usize {
        self.snapshot.feed_count
    }

    pub(crate) fn total_entry_count(&self) -> usize {
        self.snapshot.entry_count
    }

    pub(crate) fn status_message(&self) -> &str {
        &self.snapshot.status
    }

    pub(crate) fn status_tone(&self) -> &str {
        &self.snapshot.status_tone
    }

    pub(crate) fn has_status_message(&self) -> bool {
        !self.status_message().is_empty()
    }

    pub(crate) fn add_feed(&self) {
        self.session.add_feed();
    }

    pub(crate) fn refresh_all(&self) {
        self.session.refresh_all();
    }

    pub(crate) fn export_config(&self) {
        self.session.export_config();
    }

    pub(crate) fn import_config(&self) {
        self.session.import_config();
    }

    pub(crate) fn export_opml(&self) {
        self.session.export_opml();
    }

    pub(crate) fn import_opml(&self) {
        self.session.import_opml();
    }

    pub(crate) fn refresh_feed(&self, feed_id: i64, feed_title: String) {
        self.session.refresh_feed(feed_id, feed_title);
    }

    pub(crate) fn remove_feed(&self, feed_id: i64, feed_title: String) {
        self.session.remove_feed(feed_id, feed_title);
    }

    pub(crate) fn paste_feed_url(&self) {
        self.session.paste_feed_url();
    }
}

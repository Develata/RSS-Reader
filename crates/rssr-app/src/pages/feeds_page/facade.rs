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

    pub(crate) fn pending_config_import(&self) -> bool {
        self.snapshot.pending_config_import
    }

    pub(crate) fn is_delete_pending_for(&self, feed_id: i64) -> bool {
        self.session.is_delete_pending_for(feed_id)
    }

    pub(crate) fn feeds(&self) -> &[FeedSummary] {
        &self.snapshot.feeds
    }

    pub(crate) fn feed_count(&self) -> usize {
        self.snapshot.feed_count
    }

    pub(crate) fn entry_count(&self) -> usize {
        self.snapshot.entry_count
    }

    pub(crate) fn status(&self) -> &str {
        &self.snapshot.status
    }

    pub(crate) fn status_tone(&self) -> &str {
        &self.snapshot.status_tone
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

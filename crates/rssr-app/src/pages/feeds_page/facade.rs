use super::{session::FeedsPageSession, state::FeedsPageState};

#[derive(Clone, PartialEq)]
pub(crate) struct FeedsPageFacade {
    pub(crate) session: FeedsPageSession,
    pub(crate) snapshot: FeedsPageState,
}

impl FeedsPageFacade {
    pub(crate) fn new(session: FeedsPageSession, snapshot: FeedsPageState) -> Self {
        Self { session, snapshot }
    }

    pub(crate) fn set_feed_url(&self, value: String) {
        self.session.set_feed_url(value);
    }

    pub(crate) fn set_config_text(&self, value: String) {
        self.session.set_config_text(value);
    }

    pub(crate) fn set_opml_text(&self, value: String) {
        self.session.set_opml_text(value);
    }

    pub(crate) fn is_delete_pending_for(&self, feed_id: i64) -> bool {
        self.session.is_delete_pending_for(feed_id)
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

use super::edit_state::FeedEditState;
use rssr_domain::FeedSummary;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FeedsPageState {
    pub(super) edit: FeedEditState,
    pub(crate) unread_count: u64,
    pub(crate) archived_count: u64,
    pub(crate) feed_candidates: Option<(url::Url, Vec<rssr_application::FeedDiscoveryCandidate>)>,
    pub(crate) feed_url: String,
    pub(crate) adding_feed_url: Option<String>,
    /// The latest add's operation token: only its results apply (C4b item 3).
    pub(crate) add_token: u64,
    pub(crate) config_text: String,
    pub(crate) opml_text: String,
    pub(crate) pending_config_import: bool,
    pub(crate) pending_delete_feed: Option<i64>,
    /// A confirmed delete cannot be cancelled or submitted a second time.
    pub(crate) removing_feed: Option<i64>,
    pub(crate) feeds: Vec<FeedSummary>,
    pub(crate) feed_count: u64,
    pub(crate) entry_count: u64,
    /// The configuration the shown snapshot reflects; an import expects it.
    pub(crate) fingerprint: Option<rssr_domain::ConfigFingerprint>,
    pub(crate) status_revision: u64,
    pub(crate) status: String,
    pub(crate) status_tone: String,
    pub(crate) reload_tick: u64,
}

impl FeedsPageState {
    pub(crate) fn new() -> Self {
        Self {
            edit: FeedEditState::new(),
            unread_count: 0,
            archived_count: 0,
            feed_candidates: None,
            feed_url: String::new(),
            adding_feed_url: None,
            add_token: 0,
            config_text: String::new(),
            opml_text: String::new(),
            pending_config_import: false,
            pending_delete_feed: None,
            removing_feed: None,
            feeds: Vec::new(),
            feed_count: 0,
            entry_count: 0,
            fingerprint: None,
            status_revision: 0,
            status: String::new(),
            status_tone: "info".to_string(),
            reload_tick: 0,
        }
    }
}

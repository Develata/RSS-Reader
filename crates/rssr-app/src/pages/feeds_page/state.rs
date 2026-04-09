use rssr_domain::FeedSummary;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FeedsPageState {
    pub(crate) feed_url: String,
    pub(crate) config_text: String,
    pub(crate) opml_text: String,
    pub(crate) pending_config_import: bool,
    pub(crate) pending_delete_feed: Option<i64>,
    pub(crate) feeds: Vec<FeedSummary>,
    pub(crate) feed_count: usize,
    pub(crate) entry_count: usize,
    pub(crate) status: String,
    pub(crate) status_tone: String,
    pub(crate) reload_tick: u64,
}

impl FeedsPageState {
    pub(crate) fn new() -> Self {
        Self {
            feed_url: String::new(),
            config_text: String::new(),
            opml_text: String::new(),
            pending_config_import: false,
            pending_delete_feed: None,
            feeds: Vec::new(),
            feed_count: 0,
            entry_count: 0,
            status: String::new(),
            status_tone: "info".to_string(),
            reload_tick: 0,
        }
    }
}

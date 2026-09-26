use rssr_domain::FeedSummary;

#[derive(Debug, Clone)]
pub(crate) struct FeedsPageSnapshot {
    pub(crate) feeds: Vec<FeedSummary>,
    pub(crate) feed_count: usize,
    pub(crate) entry_count: usize,
}

#[derive(Debug, Clone)]
pub(crate) enum FeedsPageIntent {
    FeedCandidates { page_url: url::Url, candidates: Vec<rssr_application::FeedDiscoveryCandidate> },
    SelectFeedCandidate(usize),
    CancelFeedCandidates,
    LoadRequested,
    FeedUrlChanged(String),
    ConfigTextChanged(String),
    OpmlTextChanged(String),
    AddFeedRequested,
    AddFeedFinished { saved: bool },
    RefreshAllRequested,
    RefreshFeedRequested { feed_id: i64, feed_title: String },
    RemoveFeedRequested { feed_id: i64, feed_title: String },
    ExportConfigRequested,
    ImportConfigRequested,
    ExportOpmlRequested,
    ImportOpmlRequested,
    SnapshotLoaded(Result<FeedsPageSnapshot, String>),
    ConfigTextExported(String),
    OpmlTextExported(String),
    PendingConfigImportSet(bool),
    PendingDeleteFeedSet(Option<i64>),
    SetStatus { message: String, tone: String },
    BumpReload,
}

use super::{commands::FeedsPageCommandOutcome, queries::FeedsPageSnapshot};

#[derive(Debug)]
pub(crate) enum FeedsPageIntent {
    LoadRequested,
    FeedUrlChanged(String),
    ConfigTextChanged(String),
    OpmlTextChanged(String),
    AddFeedRequested,
    RefreshAllRequested,
    RefreshFeedRequested { feed_id: i64, feed_title: String },
    RemoveFeedRequested { feed_id: i64, feed_title: String },
    ExportConfigRequested,
    ImportConfigRequested,
    ExportOpmlRequested,
    ImportOpmlRequested,
    PasteFeedUrlRequested,
    SnapshotLoaded(Result<FeedsPageSnapshot, String>),
    CommandCompleted(FeedsPageCommandOutcome),
    ClipboardReadCompleted(Result<Option<String>, String>),
}

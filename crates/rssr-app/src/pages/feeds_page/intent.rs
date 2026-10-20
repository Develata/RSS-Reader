use rssr_domain::FeedSummary;

use crate::ui::failure::UiFailure;

#[derive(Debug, Clone)]
pub(crate) struct FeedsPageSnapshot {
    pub(crate) feeds: Vec<FeedSummary>,
    pub(crate) feed_count: u64,
    pub(crate) entry_count: u64,
    pub(crate) fingerprint: rssr_domain::ConfigFingerprint,
}

#[derive(Debug, Clone)]
pub(crate) enum FeedsPageIntent {
    FeedCandidates {
        page_url: url::Url,
        candidates: Vec<rssr_application::FeedDiscoveryCandidate>,
    },
    SelectFeedCandidate(usize),
    CancelFeedCandidates,

    ClearRefreshStatus {
        revision: u64,
        message: String,
    },
    LoadRequested,
    FeedUrlChanged(String),
    ConfigTextChanged(String),
    OpmlTextChanged(String),
    AddFeedRequested,
    AddFeedFinished {
        saved: bool,
    },
    RefreshAllRequested,
    RefreshFeedRequested {
        feed_id: i64,
        feed_title: String,
    },
    RemoveFeedRequested {
        feed_id: i64,
        feed_title: String,
    },
    ExportConfigRequested,
    ImportConfigRequested,
    ExportOpmlRequested,
    ImportOpmlRequested,
    SnapshotLoaded(Result<FeedsPageSnapshot, UiFailure>),
    ConfigTextExported(String),
    OpmlTextExported(String),
    PendingConfigImportSet(bool),
    PendingDeleteFeedSet(Option<i64>),
    SetStatus {
        message: String,
        tone: String,
    },
    /// A backend call failed (C4b item 1).
    Failed(UiFailure),
    BumpReload,
    /// 配置包替换了持久化设置；由 session 写入全局 `ThemeController`，订阅页状态不变。
    SettingsReplaced(rssr_domain::UserSettings),
}

impl FeedsPageIntent {
    /// The backend failure this intent carries.
    pub(crate) fn failure(&self) -> Option<&UiFailure> {
        match self {
            Self::SnapshotLoaded(Err(failure)) | Self::Failed(failure) => Some(failure),
            _ => None,
        }
    }
}

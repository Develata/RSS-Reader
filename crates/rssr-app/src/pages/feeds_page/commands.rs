#[derive(Debug, Clone)]
pub(crate) enum FeedsPageCommand {
    AddFeed { raw_url: String },
    RefreshAll,
    RefreshFeed { feed_id: i64, feed_title: String },
    RemoveFeed { feed_id: i64, feed_title: String, confirmed: bool },
    ExportConfig,
    ImportConfig { raw: String, confirmed: bool },
    ExportOpml,
    ImportOpml { raw: String },
}

#[derive(Debug, Default)]
pub(crate) struct FeedsPageUiPatch {
    pub(crate) feed_url: Option<String>,
    pub(crate) config_text: Option<String>,
    pub(crate) opml_text: Option<String>,
    pub(crate) pending_config_import: Option<bool>,
    pub(crate) pending_delete_feed: Option<Option<i64>>,
}

#[derive(Debug)]
pub(crate) struct FeedsPageCommandOutcome {
    pub(crate) patch: FeedsPageUiPatch,
    pub(crate) status_message: String,
    pub(crate) status_tone: &'static str,
    pub(crate) reload: bool,
}

pub(crate) fn info(
    message: impl Into<String>,
    patch: FeedsPageUiPatch,
    reload: bool,
) -> FeedsPageCommandOutcome {
    FeedsPageCommandOutcome { patch, status_message: message.into(), status_tone: "info", reload }
}

pub(crate) fn error(
    message: impl Into<String>,
    patch: FeedsPageUiPatch,
    reload: bool,
) -> FeedsPageCommandOutcome {
    FeedsPageCommandOutcome { patch, status_message: message.into(), status_tone: "error", reload }
}

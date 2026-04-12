#[derive(Debug, Clone)]
pub(crate) enum FeedsCommand {
    LoadSnapshot,
    AddFeed { raw_url: String },
    RefreshAll,
    RefreshFeed { feed_id: i64, feed_title: String },
    RemoveFeed { feed_id: i64, feed_title: String },
    ExportConfig,
    ImportConfig { raw: String },
    ExportOpml,
    ImportOpml { raw: String },
    ReadFeedUrlFromClipboard,
}

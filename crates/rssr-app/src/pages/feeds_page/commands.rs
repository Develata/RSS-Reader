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

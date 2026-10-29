#[derive(Debug, Clone)]
pub(crate) enum FeedsCommand {
    LoadEdit {
        feed_id: i64,
        generation: u64,
    },
    SaveEdit {
        edit: rssr_domain::FeedEditRequest,
        generation: u64,
    },
    LoadSnapshot,
    /// `token`: the page's operation it answers (C4b item 3); its results carry it back.
    AddFeed {
        raw_url: String,
        fallback_site_url: Option<url::Url>,
        token: u64,
    },
    RefreshFeed {
        feed_id: i64,
        feed_title: String,
    },
    RemoveFeed {
        feed_id: i64,
        feed_title: String,
    },
    ExportConfig,
    /// `expected`: the fingerprint of the snapshot the page shows.
    ImportConfig {
        raw: String,
        expected: rssr_domain::ConfigFingerprint,
    },
    ExportOpml,
    ImportOpml {
        raw: String,
    },
}

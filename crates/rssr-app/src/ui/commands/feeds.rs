#[derive(Debug, Clone)]
pub(crate) enum FeedsCommand {
    LoadSnapshot,
    AddFeed {
        raw_url: String,
        fallback_site_url: Option<url::Url>,
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

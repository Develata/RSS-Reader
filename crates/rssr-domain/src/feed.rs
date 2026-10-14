use time::OffsetDateTime;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Feed {
    pub id: i64,
    pub url: Url,
    /// 显示名称：用户或导入指定的名称优先，否则是源标题；两者都没有时为 `None`
    /// （URL 只在显示边界兜底，不写回这里，否则导出后再导入会变成指定名称）。
    pub title: Option<String>,
    /// 订阅源自己声明的标题，只由添加时的探测和刷新更新（审计 B6）。
    pub source_title: Option<String>,
    pub site_url: Option<Url>,
    pub description: Option<String>,
    pub icon_url: Option<Url>,
    // `folder` 仅用于保留 OPML / 配置交换中的分组信息，不参与当前 GUI 的阅读组织能力。
    pub folder: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub last_fetched_at: Option<OffsetDateTime>,
    pub last_success_at: Option<OffsetDateTime>,
    pub fetch_error: Option<String>,
    pub is_deleted: bool,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedSummary {
    pub id: i64,
    pub title: String,
    pub url: String,
    pub unread_count: u32,
    pub entry_count: u32,
    pub last_fetched_at: Option<OffsetDateTime>,
    pub last_success_at: Option<OffsetDateTime>,
    pub fetch_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewFeedSubscription {
    pub site_url: Option<Url>,
    pub url: Url,
    /// 用户或导入指定的名称，刷新不会覆盖：`None` 保留已有名称，`Some("")`（或只有空白）清除，
    /// 其余设置为该名称。
    pub title: Option<String>,
    /// 探测到的源标题；`None` 保留已有值，从不清除。
    pub source_title: Option<String>,
    // `folder` 仅用于导入导出保真，不应扩展为产品主线里的文件夹系统。
    pub folder: Option<String>,
}

pub fn normalize_feed_url(url: &Url) -> Url {
    let mut normalized = url.clone();
    normalized.set_fragment(None);

    let drop_port = matches!(
        (normalized.scheme(), normalized.port()),
        ("http", Some(80)) | ("https", Some(443))
    );
    if drop_port {
        let _ = normalized.set_port(None);
    }

    normalized
}

#[cfg(test)]
mod tests {
    use url::Url;

    use super::normalize_feed_url;

    #[test]
    fn normalize_feed_url_drops_fragment() {
        let url = Url::parse("https://example.com/feed.xml#section").expect("valid url");
        assert_eq!(normalize_feed_url(&url).as_str(), "https://example.com/feed.xml");
    }

    #[test]
    fn normalize_feed_url_drops_default_port() {
        let url = Url::parse("https://example.com:443/feed.xml").expect("valid url");
        assert_eq!(normalize_feed_url(&url).as_str(), "https://example.com/feed.xml");
    }
}

/// Where a recorded refresh failure came from (B2-D5). A diagnostic without one predates the
/// categories (imported from v0.1.x) and is shown generically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FetchErrorKind {
    /// Reaching the feed failed: network, HTTP status, response size.
    Fetch,
    /// The response was not a readable feed.
    Parse,
    /// Saving what was fetched failed.
    Storage,
}

impl FetchErrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fetch => "fetch",
            Self::Parse => "parse",
            Self::Storage => "storage",
        }
    }

    pub fn from_stored(raw: &str) -> Option<Self> {
        match raw {
            "fetch" => Some(Self::Fetch),
            "parse" => Some(Self::Parse),
            "storage" => Some(Self::Storage),
            _ => None,
        }
    }
}

/// The recorded diagnostic as the subscriptions page shows it: fetch and parse failures as
/// recorded (they name the address, the status or the format); a storage failure or one without
/// a category as a fixed text, since its detail is internal.
pub fn public_fetch_error(
    kind: Option<FetchErrorKind>,
    recorded: Option<String>,
) -> Option<String> {
    let recorded = recorded?;
    Some(match kind {
        Some(FetchErrorKind::Fetch | FetchErrorKind::Parse) => recorded,
        Some(FetchErrorKind::Storage) => "上次刷新时本地存储出错，请稍后重试".to_string(),
        None => "上次刷新失败".to_string(),
    })
}

#[cfg(test)]
mod fetch_error_tests {
    use super::{FetchErrorKind, public_fetch_error};

    #[test]
    fn only_fetch_and_parse_diagnostics_show_their_text() {
        let shown = |kind| public_fetch_error(kind, Some("HTTP 404".to_string()));
        assert_eq!(shown(Some(FetchErrorKind::Fetch)).as_deref(), Some("HTTP 404"));
        assert_eq!(shown(Some(FetchErrorKind::Parse)).as_deref(), Some("HTTP 404"));
        assert_eq!(
            shown(Some(FetchErrorKind::Storage)).as_deref(),
            Some("上次刷新时本地存储出错，请稍后重试")
        );
        assert_eq!(shown(None).as_deref(), Some("上次刷新失败"));
        assert_eq!(public_fetch_error(Some(FetchErrorKind::Fetch), None), None);
        for kind in [FetchErrorKind::Fetch, FetchErrorKind::Parse, FetchErrorKind::Storage] {
            assert_eq!(FetchErrorKind::from_stored(kind.as_str()), Some(kind));
        }
    }
}

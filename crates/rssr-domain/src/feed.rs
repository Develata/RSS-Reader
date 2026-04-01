use time::OffsetDateTime;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Feed {
    pub id: i64,
    pub url: Url,
    pub title: Option<String>,
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
    pub url: Url,
    pub title: Option<String>,
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

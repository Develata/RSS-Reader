use anyhow::Context;
use reqwest::StatusCode;
use url::Url;

use super::feed_response::looks_like_proxy_login_or_spa_shell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WebFeedRequestKind {
    Proxy,
    Direct,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WebFeedRequest {
    pub(super) url: String,
    pub(super) kind: WebFeedRequestKind,
}

pub(super) fn web_refresh_request_urls(raw: &str) -> anyhow::Result<Vec<WebFeedRequest>> {
    web_refresh_request_urls_with_origin_and_now(
        raw,
        browser_origin().as_deref(),
        js_sys::Date::now(),
    )
}

pub(super) fn should_fallback_web_feed_request(
    index: usize,
    total: usize,
    request: &WebFeedRequest,
    response: &reqwest::Response,
) -> bool {
    should_fallback_web_feed_request_parts(
        index,
        total,
        request.kind,
        response.status(),
        looks_like_proxy_login_or_spa_shell(response),
    )
}

fn browser_origin() -> Option<String> {
    let window = web_sys::window()?;
    window.location().origin().ok()
}

fn web_refresh_request_urls_with_origin_and_now(
    raw: &str,
    origin: Option<&str>,
    now_millis: f64,
) -> anyhow::Result<Vec<WebFeedRequest>> {
    let url = Url::parse(raw).with_context(|| format!("订阅 URL 不合法：{raw}"))?;
    let mut request_urls = Vec::new();

    if let Some(proxy_url) = web_feed_proxy_request_url(origin, url.as_str()) {
        request_urls.push(WebFeedRequest { url: proxy_url, kind: WebFeedRequestKind::Proxy });
    }

    let mut direct_url = url;
    if matches!(direct_url.scheme(), "http" | "https") {
        direct_url.query_pairs_mut().append_pair("_rssr_fetch", &now_millis.round().to_string());
    }
    request_urls
        .push(WebFeedRequest { url: direct_url.to_string(), kind: WebFeedRequestKind::Direct });
    Ok(request_urls)
}

fn web_feed_proxy_request_url(origin: Option<&str>, feed_url: &str) -> Option<String> {
    let mut proxy_url = Url::parse(origin?).ok()?;
    proxy_url.set_path("/feed-proxy");
    proxy_url.set_query(None);
    proxy_url.query_pairs_mut().append_pair("url", feed_url);
    Some(proxy_url.to_string())
}

fn status_requires_web_feed_fallback(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::NOT_FOUND
            | StatusCode::UNAUTHORIZED
            | StatusCode::FORBIDDEN
            | StatusCode::METHOD_NOT_ALLOWED
            | StatusCode::BAD_REQUEST
    )
}

fn should_fallback_web_feed_request_parts(
    index: usize,
    total: usize,
    kind: WebFeedRequestKind,
    status: StatusCode,
    looks_like_proxy_shell: bool,
) -> bool {
    index + 1 < total
        && (status_requires_web_feed_fallback(status)
            || kind == WebFeedRequestKind::Proxy && status.is_success() && looks_like_proxy_shell)
}

#[cfg(test)]
mod tests {
    use reqwest::StatusCode;

    use super::{
        WebFeedRequestKind, should_fallback_web_feed_request_parts,
        status_requires_web_feed_fallback, web_refresh_request_urls_with_origin_and_now,
    };

    #[test]
    fn request_urls_include_proxy_then_direct_for_http_feeds() {
        let request_urls = web_refresh_request_urls_with_origin_and_now(
            "https://feeds.example.com/rss.xml",
            Some("https://reader.example.com/app?mode=web"),
            1234.2,
        )
        .expect("build request urls");

        assert_eq!(request_urls.len(), 2);
        assert_eq!(request_urls[0].kind, WebFeedRequestKind::Proxy);
        assert_eq!(
            request_urls[0].url,
            "https://reader.example.com/feed-proxy?url=https%3A%2F%2Ffeeds.example.com%2Frss.xml"
        );
        assert_eq!(request_urls[1].kind, WebFeedRequestKind::Direct);
        assert_eq!(request_urls[1].url, "https://feeds.example.com/rss.xml?_rssr_fetch=1234");
    }

    #[test]
    fn request_urls_skip_proxy_when_browser_origin_is_unavailable() {
        let request_urls = web_refresh_request_urls_with_origin_and_now(
            "https://feeds.example.com/rss.xml",
            None,
            1.0,
        )
        .expect("build request urls");

        assert_eq!(request_urls.len(), 1);
        assert_eq!(request_urls[0].kind, WebFeedRequestKind::Direct);
    }

    #[test]
    fn request_urls_do_not_append_fetch_marker_for_non_http_schemes() {
        let request_urls = web_refresh_request_urls_with_origin_and_now(
            "ftp://feeds.example.com/rss.xml",
            Some("https://reader.example.com"),
            1234.0,
        )
        .expect("build request urls");

        assert_eq!(request_urls.len(), 2);
        assert_eq!(request_urls[1].url, "ftp://feeds.example.com/rss.xml");
    }

    #[test]
    fn fallback_statuses_match_browser_retry_policy() {
        assert!(status_requires_web_feed_fallback(StatusCode::NOT_FOUND));
        assert!(status_requires_web_feed_fallback(StatusCode::UNAUTHORIZED));
        assert!(status_requires_web_feed_fallback(StatusCode::FORBIDDEN));
        assert!(status_requires_web_feed_fallback(StatusCode::METHOD_NOT_ALLOWED));
        assert!(status_requires_web_feed_fallback(StatusCode::BAD_REQUEST));
        assert!(!status_requires_web_feed_fallback(StatusCode::OK));
        assert!(!status_requires_web_feed_fallback(StatusCode::TOO_MANY_REQUESTS));
    }

    #[test]
    fn fallback_policy_retries_proxy_shell_when_direct_request_remains() {
        assert!(should_fallback_web_feed_request_parts(
            0,
            2,
            WebFeedRequestKind::Proxy,
            StatusCode::OK,
            true
        ));
    }

    #[test]
    fn fallback_policy_does_not_retry_direct_or_final_requests() {
        assert!(!should_fallback_web_feed_request_parts(
            0,
            2,
            WebFeedRequestKind::Direct,
            StatusCode::OK,
            true
        ));
        assert!(!should_fallback_web_feed_request_parts(
            1,
            2,
            WebFeedRequestKind::Proxy,
            StatusCode::BAD_REQUEST,
            true
        ));
    }

    #[test]
    fn fallback_policy_retries_selected_error_statuses_only_when_next_request_exists() {
        assert!(should_fallback_web_feed_request_parts(
            0,
            2,
            WebFeedRequestKind::Proxy,
            StatusCode::FORBIDDEN,
            false
        ));
        assert!(!should_fallback_web_feed_request_parts(
            0,
            2,
            WebFeedRequestKind::Proxy,
            StatusCode::TOO_MANY_REQUESTS,
            false
        ));
    }
}

use anyhow::Context;
use url::Url;

/// 静态 Web 包直接向 feed 地址发请求，受目标站点的 CORS 策略约束；没有代理回退。
/// （由 rssr-web 托管的页面走服务端模式，由服务端抓取，不经过这里。）
///
/// 刷新请求在 HTTP(S) 地址上附加 `_rssr_fetch` 时间戳，绕开浏览器 HTTP 缓存；订阅发现使用原始地址，
/// 让站点看到用户输入的地址，最终地址取自响应。
pub(super) fn web_feed_request_url(raw: &str, discovery: bool) -> anyhow::Result<String> {
    web_feed_request_url_at(raw, discovery, js_sys::Date::now())
}

fn web_feed_request_url_at(raw: &str, discovery: bool, now_millis: f64) -> anyhow::Result<String> {
    let mut url = Url::parse(raw).with_context(|| format!("订阅 URL 不合法：{raw}"))?;
    if !discovery && matches!(url.scheme(), "http" | "https") {
        url.query_pairs_mut().append_pair("_rssr_fetch", &now_millis.round().to_string());
    }
    Ok(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::web_feed_request_url_at;

    #[test]
    fn refresh_requests_go_direct_with_a_cache_busting_marker() {
        assert_eq!(
            web_feed_request_url_at("https://feeds.example.com/rss.xml", false, 1234.2).unwrap(),
            "https://feeds.example.com/rss.xml?_rssr_fetch=1234"
        );
    }

    #[test]
    fn discovery_uses_the_address_as_entered() {
        assert_eq!(
            web_feed_request_url_at("https://example.com/blog", true, 1234.0).unwrap(),
            "https://example.com/blog"
        );
    }

    #[test]
    fn non_http_schemes_get_no_fetch_marker() {
        assert_eq!(
            web_feed_request_url_at("ftp://feeds.example.com/rss.xml", false, 1234.0).unwrap(),
            "ftp://feeds.example.com/rss.xml"
        );
    }

    #[test]
    fn invalid_addresses_are_rejected() {
        assert!(web_feed_request_url_at("not a url", false, 0.0).is_err());
    }
}

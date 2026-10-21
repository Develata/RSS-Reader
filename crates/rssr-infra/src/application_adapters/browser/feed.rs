use anyhow::Context;
use reqwest::header;

use crate::feed_normalization::parse_feed_xml;
pub use crate::feed_normalization::{ParsedEntry, ParsedFeed, hash_content};

use super::feed_request::web_feed_request_url;
use super::feed_response::looks_like_html_response_body;

pub async fn web_fetch_feed_response(
    client: &reqwest::Client,
    raw: &str,
) -> anyhow::Result<reqwest::Response> {
    fetch_web_response(client, raw, false).await
}

/// 订阅探测：返回响应与重定向之后的最终地址。
pub(crate) async fn web_fetch_subscription_response(
    client: &reqwest::Client,
    raw: &str,
) -> anyhow::Result<(reqwest::Response, url::Url)> {
    let response = fetch_web_response(client, raw, true).await?;
    let final_url = response.url().clone();
    Ok((response, final_url))
}

async fn fetch_web_response(
    client: &reqwest::Client,
    raw: &str,
    discovery: bool,
) -> anyhow::Result<reqwest::Response> {
    client
        .get(web_feed_request_url(raw, discovery)?)
        .timeout(std::time::Duration::from_secs(30))
        .header(
            header::ACCEPT,
            "application/atom+xml, application/rss+xml, application/xml, text/xml;q=0.9, */*;q=0.1",
        )
        .send()
        .await
        .context("发送 feed 抓取请求失败（浏览器直连时通常是目标站点未开放 CORS，或当前网络不可达；需要服务端抓取时请使用桌面版或 rssr-web）")
}

pub fn parse_feed(raw: &str) -> anyhow::Result<ParsedFeed> {
    if looks_like_html_response_body(raw) {
        anyhow::bail!(
            "当前响应不是 XML feed，而是 HTML 页面（可能是网站首页、登录页或错误页，请确认填写的是 RSS/Atom 地址）"
        );
    }

    parse_feed_xml(raw, false)
}

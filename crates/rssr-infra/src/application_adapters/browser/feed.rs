use anyhow::Context;
use reqwest::header;

use crate::feed_normalization::parse_feed_xml;
pub use crate::feed_normalization::{ParsedEntry, ParsedFeed, hash_content};

use super::feed_request::{should_fallback_web_feed_request, web_refresh_request_urls};
use super::feed_response::looks_like_html_response_body;

pub async fn web_fetch_feed_response(
    client: &reqwest::Client,
    raw: &str,
) -> anyhow::Result<reqwest::Response> {
    let request_urls = web_refresh_request_urls(raw)?;
    let mut last_error = None;

    for (index, request) in request_urls.iter().enumerate() {
        let response = client
            .get(&request.url)
            .header(
                header::ACCEPT,
                "application/atom+xml, application/rss+xml, application/xml, text/xml;q=0.9, */*;q=0.1",
            )
            .send()
            .await;

        match response {
            Ok(response)
                if should_fallback_web_feed_request(
                    index,
                    request_urls.len(),
                    request,
                    &response,
                ) =>
            {
                continue;
            }
            Ok(response) => return Ok(response),
            Err(error) => last_error = Some(error),
        }
    }

    let error = last_error.map(anyhow::Error::from).unwrap_or_else(|| {
        anyhow::anyhow!(
            "发送 feed 抓取请求失败（浏览器环境下通常是目标站点未开放 CORS、当前部署未启用 feed 代理，或当前网络不可达）"
        )
    });
    Err(error).context(
        "发送 feed 抓取请求失败（浏览器环境下通常是目标站点未开放 CORS、当前部署未启用 feed 代理，或当前网络不可达）",
    )
}

pub fn parse_feed(raw: &str) -> anyhow::Result<ParsedFeed> {
    if looks_like_html_response_body(raw) {
        anyhow::bail!(
            "当前响应不是 XML feed，而是 HTML 页面（通常说明当前部署未启用 feed 代理，或请求被登录页/静态壳页面拦截）"
        );
    }

    parse_feed_xml(raw, false)
}

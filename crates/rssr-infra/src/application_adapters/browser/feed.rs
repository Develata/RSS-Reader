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
    Ok(fetch_web_response(client, raw, false).await?.0)
}

pub(crate) async fn web_fetch_subscription_response(
    client: &reqwest::Client,
    raw: &str,
) -> anyhow::Result<(reqwest::Response, url::Url)> {
    let (response, is_proxy) = fetch_web_response(client, raw, true).await?;
    let final_url = if is_proxy {
        response
            .headers()
            .get("x-rssr-final-url")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| url::Url::parse(value).ok())
            .filter(|url| matches!(url.scheme(), "http" | "https"))
            .context("订阅代理未提供最终地址，请升级服务端后重试，或使用可直连的 feed 地址。")?
    } else {
        response.url().clone()
    };
    Ok((response, final_url))
}

async fn fetch_web_response(
    client: &reqwest::Client,
    raw: &str,
    discovery: bool,
) -> anyhow::Result<(reqwest::Response, bool)> {
    let mut request_urls = web_refresh_request_urls(raw)?;
    if discovery {
        for request in &mut request_urls {
            if request.kind == super::feed_request::WebFeedRequestKind::Direct {
                request.url = raw.to_string();
            }
        }
    }
    let mut last_error = None;

    for (index, request) in request_urls.iter().enumerate() {
        let response = client
            .get(&request.url)
            .timeout(std::time::Duration::from_secs(30))
            .header(
                header::ACCEPT,
                "application/atom+xml, application/rss+xml, application/xml, text/xml;q=0.9, */*;q=0.1",
            )
            .send()
            .await;

        match response {
            Ok(response)
                if !(discovery
                    && request.kind == super::feed_request::WebFeedRequestKind::Proxy
                    && response.headers().contains_key("x-rssr-final-url"))
                    && should_fallback_web_feed_request(
                        index,
                        request_urls.len(),
                        request,
                        &response,
                    ) =>
            {
                continue;
            }
            Ok(response) => {
                return Ok((
                    response,
                    request.kind == super::feed_request::WebFeedRequestKind::Proxy,
                ));
            }
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

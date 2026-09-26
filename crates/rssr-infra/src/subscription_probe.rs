//! 添加订阅专用的有界抓取与解析；发现策略由 application 决定。
mod html;

use anyhow::{Context, Result, bail};
use futures_util::StreamExt;
use reqwest::header;
use rssr_application::{
    FeedRefreshUpdate, RefreshHttpMetadata, SubscriptionProbeOutcome, SubscriptionProbePort,
};
use url::Url;

pub const MAX_SUBSCRIPTION_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone)]
pub struct HttpSubscriptionProbe {
    client: reqwest::Client,
}

impl HttpSubscriptionProbe {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl SubscriptionProbePort for HttpSubscriptionProbe {
    async fn probe(&self, url: &Url) -> Result<SubscriptionProbeOutcome> {
        if !matches!(url.scheme(), "http" | "https") {
            bail!("订阅只支持 HTTP 或 HTTPS 地址。");
        }
        #[cfg(not(target_arch = "wasm32"))]
        let response =
            self.client.get(url.clone()).timeout(std::time::Duration::from_secs(30)).send().await?;
        #[cfg(not(target_arch = "wasm32"))]
        let (response, final_url) = {
            let final_url = response.url().clone();
            (response, final_url)
        };
        #[cfg(target_arch = "wasm32")]
        let (response, final_url) =
            crate::application_adapters::browser::feed::web_fetch_subscription_response(
                &self.client,
                url.as_str(),
            )
            .await?;
        let response = response.error_for_status().context("订阅探测返回非成功状态")?;
        let metadata = RefreshHttpMetadata {
            etag: response
                .headers()
                .get(header::ETAG)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string),
            last_modified: response
                .headers()
                .get(header::LAST_MODIFIED)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string),
        };
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        if response.content_length().is_some_and(|length| length > MAX_SUBSCRIPTION_BYTES as u64) {
            bail!("订阅响应超过 8 MiB 上限。");
        }
        let mut bytes = Vec::new();
        let stream = response.bytes_stream();
        futures_util::pin_mut!(stream);
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("读取订阅响应失败")?;
            if chunk.len() > MAX_SUBSCRIPTION_BYTES - bytes.len() {
                bail!("订阅响应超过 8 MiB 上限。");
            }
            bytes.extend_from_slice(&chunk);
        }
        let charset = content_type.split(';').skip(1).find_map(|part| {
            let (key, value) = part.trim().split_once('=')?;
            key.eq_ignore_ascii_case("charset").then(|| value.trim().trim_matches('"'))
        });
        let encoding = charset
            .and_then(|label| encoding_rs::Encoding::for_label(label.as_bytes()))
            .unwrap_or(encoding_rs::UTF_8);
        let (body, _, _) = encoding.decode(&bytes);
        classify_subscription_response(final_url, metadata, &content_type, &body)
    }
}

pub fn classify_subscription_response(
    url: Url,
    metadata: RefreshHttpMetadata,
    content_type: &str,
    body: &str,
) -> Result<SubscriptionProbeOutcome> {
    if body.len() > MAX_SUBSCRIPTION_BYTES {
        bail!("订阅响应超过 8 MiB 上限。");
    }
    if let Ok(parsed) = crate::feed_normalization::parse_feed_xml(body, false) {
        return Ok(SubscriptionProbeOutcome::Feed {
            url: rssr_domain::normalize_feed_url(&url),
            update: FeedRefreshUpdate { metadata, feed: map_parsed_feed(parsed) },
        });
    }
    let leading = body
        .trim_start_matches('\u{feff}')
        .trim_start()
        .chars()
        .take(256)
        .collect::<String>()
        .to_ascii_lowercase();
    let mime = content_type.split(';').next().unwrap_or("").trim();
    if !mime.eq_ignore_ascii_case("text/html")
        && !mime.eq_ignore_ascii_case("application/xhtml+xml")
        && !leading.starts_with("<!doctype html")
        && !leading.starts_with("<html")
    {
        bail!("响应既不是可解析的 RSS/Atom，也不是 HTML 网页。");
    }
    Ok(SubscriptionProbeOutcome::Html { candidates: html::discover(&url, body), page_url: url })
}

fn map_parsed_feed(
    parsed: crate::feed_normalization::ParsedFeed,
) -> rssr_application::ParsedFeedUpdate {
    rssr_application::ParsedFeedUpdate {
        title: parsed.title,
        site_url: parsed.site_url,
        description: parsed.description,
        entries: parsed
            .entries
            .into_iter()
            .map(|e| rssr_application::ParsedEntryData {
                external_id: e.external_id,
                dedup_key: e.dedup_key,
                url: e.url,
                title: e.title,
                author: e.author,
                summary: e.summary,
                content_html: e.content_html,
                content_text: e.content_text,
                published_at: e.published_at,
                updated_at_source: e.updated_at_source,
            })
            .collect(),
    }
}

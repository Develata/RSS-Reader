use anyhow::Context;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use regex::Regex;
use reqwest::header;
use std::time::Duration;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use tokio::sync::Semaphore;
use url::Url;

use super::image_html::{collect_localizable_sources, image_tag_regex, rewrite_localized_html};

#[derive(Debug, Clone)]
pub struct BodyAssetLocalizer {
    inner: reqwest::Client,
    image_request_slots: Arc<Semaphore>,
}

#[derive(Debug)]
struct LocalizationPlan {
    img_regex: Regex,
    sources: BTreeSet<(String, Url)>,
}

impl BodyAssetLocalizer {
    const IMAGE_USER_AGENT: &'static str =
        "Mozilla/5.0 (compatible; RSS-Reader image localizer; +https://github.com)";
    pub const IMAGE_REQUEST_TIMEOUT: Duration = Duration::from_secs(8);
    const MAX_IMAGE_BYTES: usize = 1024 * 1024;
    const MAX_TOTAL_IMAGE_BYTES: usize = 2 * 1024 * 1024;
    const MAX_HTML_BYTES: usize = 256 * 1024;
    const MAX_CONCURRENT_IMAGE_REQUESTS: usize = 2;

    pub fn new() -> Self {
        Self {
            inner: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(3))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            image_request_slots: Arc::new(Semaphore::new(Self::MAX_CONCURRENT_IMAGE_REQUESTS)),
        }
    }

    pub const fn max_images_per_entry() -> usize {
        8
    }

    pub async fn localize_html_images(
        &self,
        html: &str,
        base_url: Option<&Url>,
    ) -> anyhow::Result<String> {
        let Some(plan) = self.build_localization_plan(html, base_url) else {
            return Ok(html.to_string());
        };
        let localized = self.localize_sources(&plan.sources, base_url).await;

        if localized.is_empty() {
            return Ok(html.to_string());
        }

        Ok(rewrite_localized_html(html, &plan.img_regex, &localized))
    }

    async fn fetch_image_as_data_url(
        &self,
        url: &Url,
        referer: Option<&Url>,
        max_bytes: usize,
    ) -> anyhow::Result<Option<(String, usize)>> {
        let mut request = self
            .inner
            .get(url.clone())
            .header(header::USER_AGENT, Self::IMAGE_USER_AGENT)
            .header(
                header::ACCEPT,
                "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8",
            )
            .timeout(Self::IMAGE_REQUEST_TIMEOUT);
        if let Some(referer) = referer {
            request = request.header(header::REFERER, referer.as_str());
        }

        let _permit = self
            .image_request_slots
            .clone()
            .acquire_owned()
            .await
            .context("等待正文图片抓取节流槽位失败")?;

        let response = request
            .send()
            .await
            .with_context(|| format!("抓取正文图片失败: {url}"))?
            .error_for_status()
            .with_context(|| format!("正文图片返回非成功状态: {url}"))?;

        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(super::image_html::normalize_image_content_type);
        let Some(content_type) = content_type else {
            return Ok(None);
        };

        let bytes = response.bytes().await.with_context(|| format!("读取正文图片失败: {url}"))?;
        if bytes.len() > max_bytes {
            tracing::warn!(asset_url = %url, byte_len = bytes.len(), "正文图片过大，跳过本地化");
            return Ok(None);
        }

        let byte_len = bytes.len();
        Ok(Some((format!("data:{content_type};base64,{}", BASE64.encode(bytes)), byte_len)))
    }

    fn build_localization_plan(
        &self,
        html: &str,
        base_url: Option<&Url>,
    ) -> Option<LocalizationPlan> {
        if !html.contains("<img") || html.len() > Self::MAX_HTML_BYTES {
            return None;
        }

        let img_regex = image_tag_regex();
        let sources = collect_localizable_sources(html, base_url, Self::max_images_per_entry());
        if sources.is_empty() {
            return None;
        }

        Some(LocalizationPlan { img_regex, sources })
    }

    async fn localize_sources(
        &self,
        sources: &BTreeSet<(String, Url)>,
        base_url: Option<&Url>,
    ) -> BTreeMap<String, String> {
        let mut localized = BTreeMap::new();
        let mut total_localized_bytes = 0_usize;

        for (raw, resolved) in sources {
            match self.fetch_image_as_data_url(resolved, base_url, Self::MAX_IMAGE_BYTES).await {
                Ok(Some((data_url, byte_len))) => {
                    if total_localized_bytes + byte_len > Self::MAX_TOTAL_IMAGE_BYTES {
                        tracing::warn!(
                            asset_url = %resolved,
                            byte_len,
                            total_localized_bytes,
                            "正文图片累计体积过大，跳过剩余本地化"
                        );
                        break;
                    }
                    localized.insert(raw.clone(), data_url);
                    total_localized_bytes += byte_len;
                }
                Ok(None) => {}
                Err(error) => {
                    tracing::warn!(asset_url = %resolved, error = %error, "正文图片本地化失败，保留远端地址");
                }
            }
        }

        localized
    }
}

impl Default for BodyAssetLocalizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::fetch::client::image_html;

    use super::BodyAssetLocalizer;

    #[test]
    fn body_asset_localizer_default_matches_new() {
        assert_eq!(
            BodyAssetLocalizer::default().image_request_slots.available_permits(),
            BodyAssetLocalizer::new().image_request_slots.available_permits()
        );
    }

    #[test]
    fn normalize_image_content_type_delegates_to_image_html_module() {
        assert_eq!(
            image_html::normalize_image_content_type("image/png; charset=binary").as_deref(),
            Some("image/png")
        );
        assert_eq!(image_html::normalize_image_content_type("text/html"), None);
    }
}

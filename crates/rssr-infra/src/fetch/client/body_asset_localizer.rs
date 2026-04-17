use anyhow::Context;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use reqwest::header;
use std::time::Duration;
use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::Semaphore;
use url::Url;

use super::image_html::{LocalizableImageDocument, normalize_html_for_live_display};

#[derive(Debug, Clone)]
pub struct BodyAssetLocalizer {
    inner: reqwest::Client,
    image_request_slots: Arc<Semaphore>,
    budget: LocalizationBudget,
}

#[derive(Debug, Clone, Copy)]
struct LocalizationBudget {
    profile: &'static str,
    image_request_timeout: Duration,
    max_image_bytes: usize,
    max_total_image_bytes: usize,
    max_html_bytes: usize,
    max_images_per_entry: usize,
    max_concurrent_image_requests: usize,
}

#[derive(Debug)]
struct LocalizationPlan {
    document: LocalizableImageDocument,
}

#[derive(Debug)]
enum FetchImageError {
    Timeout,
    Request(String),
    HttpStatus(u16),
    NonImageContentType(Option<String>),
    TooLarge { byte_len: usize, max_bytes: usize },
    Read(String),
}

impl BodyAssetLocalizer {
    const IMAGE_USER_AGENT: &'static str =
        "Mozilla/5.0 (compatible; RSS-Reader image localizer; +https://github.com)";
    const BACKGROUND_BUDGET: LocalizationBudget = LocalizationBudget {
        profile: "background",
        image_request_timeout: Duration::from_secs(8),
        max_image_bytes: 3 * 1024 * 1024,
        max_total_image_bytes: 6 * 1024 * 1024,
        max_html_bytes: 768 * 1024,
        max_images_per_entry: 16,
        max_concurrent_image_requests: 2,
    };
    const READER_BUDGET: LocalizationBudget = LocalizationBudget {
        profile: "reader_on_demand",
        image_request_timeout: Duration::from_secs(10),
        max_image_bytes: 6 * 1024 * 1024,
        max_total_image_bytes: 18 * 1024 * 1024,
        max_html_bytes: 1536 * 1024,
        max_images_per_entry: 24,
        max_concurrent_image_requests: 2,
    };

    pub fn new() -> Self {
        Self::with_budget(Self::BACKGROUND_BUDGET)
    }

    pub fn for_reader_entry() -> Self {
        Self::with_budget(Self::READER_BUDGET)
    }

    fn with_budget(budget: LocalizationBudget) -> Self {
        Self {
            inner: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(3))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            image_request_slots: Arc::new(Semaphore::new(budget.max_concurrent_image_requests)),
            budget,
        }
    }

    pub fn max_images_per_entry(&self) -> usize {
        self.budget.max_images_per_entry
    }

    pub fn image_request_timeout(&self) -> Duration {
        self.budget.image_request_timeout
    }

    pub async fn localize_html_images(
        &self,
        html: &str,
        base_url: Option<&Url>,
    ) -> anyhow::Result<String> {
        let normalized_html = normalize_html_for_live_display(html, base_url);
        let Some(plan) = self.build_localization_plan(&normalized_html, base_url) else {
            return Ok(normalized_html);
        };
        let localized = self.localize_sources(&plan.document, base_url).await;

        if localized.is_empty() {
            return Ok(normalized_html);
        }

        Ok(plan.document.rewrite_html(&normalized_html, &localized))
    }

    async fn fetch_image_as_data_url(
        &self,
        url: &Url,
        referer: Option<&Url>,
        max_bytes: usize,
    ) -> Result<(String, usize), FetchImageError> {
        let mut request = self
            .inner
            .get(url.clone())
            .header(header::USER_AGENT, Self::IMAGE_USER_AGENT)
            .header(
                header::ACCEPT,
                "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8",
            )
            .timeout(self.budget.image_request_timeout);
        if let Some(referer) = referer {
            request = request.header(header::REFERER, referer.as_str());
        }

        let _permit = self
            .image_request_slots
            .clone()
            .acquire_owned()
            .await
            .context("等待正文图片抓取节流槽位失败")
            .map_err(|error| FetchImageError::Request(error.to_string()))?;

        let response = request.send().await.map_err(|error| {
            if error.is_timeout() {
                FetchImageError::Timeout
            } else {
                FetchImageError::Request(error.to_string())
            }
        })?;

        if !response.status().is_success() {
            return Err(FetchImageError::HttpStatus(response.status().as_u16()));
        }

        let raw_content_type =
            response.headers().get(header::CONTENT_TYPE).and_then(|value| value.to_str().ok());
        let content_type =
            raw_content_type.and_then(super::image_html::normalize_image_content_type);
        let Some(content_type) = content_type else {
            return Err(FetchImageError::NonImageContentType(
                raw_content_type.map(ToOwned::to_owned),
            ));
        };

        let bytes = response.bytes().await.map_err(|error| {
            if error.is_timeout() {
                FetchImageError::Timeout
            } else {
                FetchImageError::Read(error.to_string())
            }
        })?;
        if bytes.len() > max_bytes {
            return Err(FetchImageError::TooLarge { byte_len: bytes.len(), max_bytes });
        }

        let byte_len = bytes.len();
        Ok((format!("data:{content_type};base64,{}", BASE64.encode(bytes)), byte_len))
    }

    fn build_localization_plan(
        &self,
        html: &str,
        base_url: Option<&Url>,
    ) -> Option<LocalizationPlan> {
        if html.len() > self.budget.max_html_bytes {
            tracing::warn!(
                profile = self.budget.profile,
                reason = "html_too_large",
                html_len = html.len(),
                max_html_bytes = self.budget.max_html_bytes,
                entry_url = ?base_url,
                "正文 HTML 超过图片本地化预算，跳过当前文章"
            );
            return None;
        }

        let document =
            LocalizableImageDocument::parse(html, base_url, self.budget.max_images_per_entry);
        if !document.has_image_markup() {
            return None;
        }

        if document.unsupported_slot_count() > 0 {
            tracing::debug!(
                profile = self.budget.profile,
                reason = "unsupported_pattern",
                unsupported_slot_count = document.unsupported_slot_count(),
                entry_url = ?base_url,
                "正文中存在未命中的图片模式，保留原始地址"
            );
        }
        if document.unresolved_slot_count() > 0 {
            tracing::debug!(
                profile = self.budget.profile,
                reason = "resolve_failed",
                unresolved_slot_count = document.unresolved_slot_count(),
                entry_url = ?base_url,
                "正文图片地址无法基于当前上下文解析"
            );
        }

        if document.slots().is_empty() {
            return None;
        }

        Some(LocalizationPlan { document })
    }

    async fn localize_sources(
        &self,
        document: &LocalizableImageDocument,
        base_url: Option<&Url>,
    ) -> BTreeMap<usize, String> {
        let mut localized = BTreeMap::new();
        let mut total_localized_bytes = 0_usize;

        for (slot_index, slot) in document.slots().iter().enumerate() {
            match self
                .fetch_image_as_data_url(slot.resolved_url(), base_url, self.budget.max_image_bytes)
                .await
            {
                Ok((data_url, byte_len)) => {
                    if total_localized_bytes + byte_len > self.budget.max_total_image_bytes {
                        tracing::warn!(
                            profile = self.budget.profile,
                            reason = "too_large_total",
                            asset_url = %slot.resolved_url(),
                            byte_len,
                            total_localized_bytes,
                            max_total_image_bytes = self.budget.max_total_image_bytes,
                            "正文图片累计体积过大，跳过剩余本地化"
                        );
                        break;
                    }
                    localized.insert(slot_index, data_url);
                    total_localized_bytes += byte_len;
                }
                Err(FetchImageError::Timeout) => {
                    tracing::warn!(
                        profile = self.budget.profile,
                        reason = "timeout",
                        asset_url = %slot.resolved_url(),
                        timeout_secs = self.budget.image_request_timeout.as_secs(),
                        "正文图片抓取超时，保留远端地址"
                    );
                }
                Err(FetchImageError::HttpStatus(status)) => {
                    tracing::warn!(
                        profile = self.budget.profile,
                        reason = "http_status",
                        asset_url = %slot.resolved_url(),
                        status,
                        "正文图片返回非成功状态，保留远端地址"
                    );
                }
                Err(FetchImageError::NonImageContentType(content_type)) => {
                    tracing::warn!(
                        profile = self.budget.profile,
                        reason = "non_image_content_type",
                        asset_url = %slot.resolved_url(),
                        content_type = content_type.as_deref().unwrap_or("<missing>"),
                        "正文图片地址返回了非图片内容，保留远端地址"
                    );
                }
                Err(FetchImageError::TooLarge { byte_len, max_bytes }) => {
                    tracing::warn!(
                        profile = self.budget.profile,
                        reason = "too_large",
                        asset_url = %slot.resolved_url(),
                        byte_len,
                        max_bytes,
                        "正文图片体积超过单图预算，保留远端地址"
                    );
                }
                Err(FetchImageError::Request(error)) | Err(FetchImageError::Read(error)) => {
                    tracing::warn!(
                        profile = self.budget.profile,
                        reason = "request_failed",
                        asset_url = %slot.resolved_url(),
                        error = %error,
                        "正文图片本地化失败，保留远端地址"
                    );
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
    use crate::fetch::client::image_html::{self, LocalizableImageDocument};

    use super::BodyAssetLocalizer;
    use url::Url;

    #[test]
    fn body_asset_localizer_default_matches_new() {
        assert_eq!(
            BodyAssetLocalizer::default().image_request_slots.available_permits(),
            BodyAssetLocalizer::new().image_request_slots.available_permits()
        );
    }

    #[test]
    fn reader_localizer_has_wider_budget_than_background() {
        let background = BodyAssetLocalizer::new();
        let reader = BodyAssetLocalizer::for_reader_entry();

        assert!(reader.max_images_per_entry() > background.max_images_per_entry());
        assert!(reader.image_request_timeout() >= background.image_request_timeout());
    }

    #[test]
    fn normalize_image_content_type_delegates_to_image_html_module() {
        assert_eq!(
            image_html::normalize_image_content_type("image/png; charset=binary").as_deref(),
            Some("image/png")
        );
        assert_eq!(image_html::normalize_image_content_type("text/html"), None);
    }

    #[test]
    fn build_localization_plan_detects_responsive_images() {
        let localizer = BodyAssetLocalizer::for_reader_entry();
        let document = LocalizableImageDocument::parse(
            r#"<picture><source srcset="https://cdn.example.com/pic.avif 1x, https://cdn.example.com/pic@2x.avif 2x"><img src="https://cdn.example.com/pic.jpg"></picture>"#,
            None,
            localizer.max_images_per_entry(),
        );

        assert_eq!(document.slots().len(), 1);
    }

    #[tokio::test]
    async fn localize_html_images_falls_back_to_live_remote_sources_when_html_is_too_large() {
        let localizer = BodyAssetLocalizer::for_reader_entry();
        let article = Url::parse("https://blogs.nvidia.com/blog/example-post/").expect("article");
        let padding = "x".repeat(1_600_000);
        let html = format!(
            r#"<base href="https://blogs.nvidia.com/wp-content/uploads/2026/04/"><div>{padding}</div><img src="/blank.gif" data-src="Filmora-1680x945.png" data-srcset="Filmora-1680x945.png 1680w, Filmora.png 1920w">"#
        );

        let rewritten = localizer
            .localize_html_images(&html, Some(&article))
            .await
            .expect("normalize large html");

        assert!(rewritten.contains(
            r#"src="https://blogs.nvidia.com/wp-content/uploads/2026/04/Filmora-1680x945.png""#
        ));
        assert!(!rewritten.contains("Filmora.png 1920w"));
        assert!(!rewritten.contains("srcset="));
        assert!(!rewritten.contains(r#"src="/blank.gif""#));
        assert!(!rewritten.contains("data-srcset"));
        assert!(!rewritten.contains(r#"loading="lazy""#));
    }
}

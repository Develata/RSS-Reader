use anyhow::Context;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use regex::Regex;
use reqwest::{StatusCode, header};
use std::collections::{BTreeMap, BTreeSet};
use url::Url;

#[derive(Debug, Clone, Default)]
pub struct FetchClient {
    inner: reqwest::Client,
}

#[derive(Debug, Clone)]
pub struct FetchRequest {
    pub url: String,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HttpMetadata {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchResult {
    NotModified(HttpMetadata),
    Fetched { body: String, metadata: HttpMetadata },
}

#[derive(Debug, Clone, Default)]
pub struct BodyAssetLocalizer {
    inner: reqwest::Client,
}

impl FetchClient {
    pub fn new() -> Self {
        Self { inner: reqwest::Client::new() }
    }

    pub async fn fetch(&self, request: &FetchRequest) -> anyhow::Result<FetchResult> {
        let mut builder = self.inner.get(&request.url).header(
            header::ACCEPT,
            "application/atom+xml, application/rss+xml, application/xml, text/xml;q=0.9, */*;q=0.1",
        );

        if let Some(etag) = &request.etag {
            builder = builder.header(header::IF_NONE_MATCH, etag);
        }
        if let Some(last_modified) = &request.last_modified {
            builder = builder.header(header::IF_MODIFIED_SINCE, last_modified);
        }

        let response = builder.send().await.context("发送 feed 抓取请求失败")?;
        let metadata = HttpMetadata {
            etag: response
                .headers()
                .get(header::ETAG)
                .and_then(|value| value.to_str().ok())
                .map(ToOwned::to_owned),
            last_modified: response
                .headers()
                .get(header::LAST_MODIFIED)
                .and_then(|value| value.to_str().ok())
                .map(ToOwned::to_owned),
        };

        if response.status() == StatusCode::NOT_MODIFIED {
            return Ok(FetchResult::NotModified(metadata));
        }

        let response = response.error_for_status().context("feed 抓取返回非成功状态")?;
        let body = response.text().await.context("读取 feed 响应正文失败")?;

        Ok(FetchResult::Fetched { body, metadata })
    }
}

impl BodyAssetLocalizer {
    pub fn new() -> Self {
        Self { inner: reqwest::Client::new() }
    }

    pub fn max_images_per_entry() -> usize {
        4
    }

    pub async fn localize_html_images(
        &self,
        html: &str,
        base_url: Option<&Url>,
    ) -> anyhow::Result<String> {
        const MAX_IMAGE_BYTES: usize = 512 * 1024;
        const MAX_TOTAL_IMAGE_BYTES: usize = 1024 * 1024;
        const MAX_HTML_BYTES: usize = 256 * 1024;

        if !html.contains("<img") || html.len() > MAX_HTML_BYTES {
            return Ok(html.to_string());
        }

        let src_regex = Regex::new(
            r#"(?is)(<img\b[^>]*?\bsrc\s*=\s*)(?P<quote>['"])(?P<src>[^'"]+)(?P=quote)"#,
        )
        .expect("valid image src regex");

        let mut sources = BTreeSet::new();
        for captures in src_regex.captures_iter(html) {
            let Some(src_match) = captures.name("src") else {
                continue;
            };
            if sources.len() >= Self::max_images_per_entry() {
                break;
            }
            let raw = src_match.as_str().trim();
            if should_skip_asset(raw) {
                continue;
            }
            if let Some(resolved) = resolve_asset_url(raw, base_url) {
                sources.insert((raw.to_string(), resolved));
            }
        }

        if sources.is_empty() {
            return Ok(html.to_string());
        }

        let mut localized = BTreeMap::new();
        let mut total_localized_bytes = 0_usize;
        for (raw, resolved) in sources {
            match self.fetch_image_as_data_url(&resolved, MAX_IMAGE_BYTES).await {
                Ok(Some((data_url, byte_len))) => {
                    if total_localized_bytes + byte_len > MAX_TOTAL_IMAGE_BYTES {
                        tracing::warn!(
                            asset_url = %resolved,
                            byte_len,
                            total_localized_bytes,
                            "正文图片累计体积过大，跳过剩余本地化"
                        );
                        break;
                    }
                    localized.insert(raw, data_url);
                    total_localized_bytes += byte_len;
                }
                Ok(None) => {}
                Err(error) => {
                    tracing::warn!(asset_url = %resolved, error = %error, "正文图片本地化失败，保留远端地址");
                }
            }
        }

        if localized.is_empty() {
            return Ok(html.to_string());
        }

        Ok(src_regex
            .replace_all(html, |captures: &regex::Captures<'_>| {
                let prefix = captures.get(1).map(|value| value.as_str()).unwrap_or_default();
                let quote = captures.name("quote").map(|value| value.as_str()).unwrap_or("\"");
                let raw = captures.name("src").map(|value| value.as_str()).unwrap_or_default();
                if let Some(rewritten) = localized.get(raw) {
                    format!("{prefix}{quote}{rewritten}{quote}")
                } else {
                    captures.get(0).map(|value| value.as_str()).unwrap_or_default().to_string()
                }
            })
            .into_owned())
    }

    async fn fetch_image_as_data_url(
        &self,
        url: &Url,
        max_bytes: usize,
    ) -> anyhow::Result<Option<(String, usize)>> {
        let response = self
            .inner
            .get(url.clone())
            .send()
            .await
            .with_context(|| format!("抓取正文图片失败: {url}"))?
            .error_for_status()
            .with_context(|| format!("正文图片返回非成功状态: {url}"))?;

        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(normalize_image_content_type);
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
}

fn should_skip_asset(raw: &str) -> bool {
    raw.is_empty() || raw.starts_with("data:") || raw.starts_with("blob:")
}

fn resolve_asset_url(raw: &str, base_url: Option<&Url>) -> Option<Url> {
    let resolved =
        Url::parse(raw).ok().or_else(|| base_url.and_then(|base| base.join(raw).ok()))?;
    matches!(resolved.scheme(), "http" | "https").then_some(resolved)
}

fn normalize_image_content_type(raw: &str) -> Option<String> {
    let mime = raw.split(';').next()?.trim().to_ascii_lowercase();
    mime.starts_with("image/").then_some(mime)
}

#[cfg(test)]
mod tests {
    use super::{normalize_image_content_type, resolve_asset_url};
    use url::Url;

    #[test]
    fn resolve_asset_url_uses_base_for_relative_paths() {
        let base = Url::parse("https://example.com/posts/entry").expect("valid base url");
        let resolved = resolve_asset_url("/images/pic.png", Some(&base)).expect("resolved image");
        assert_eq!(resolved.as_str(), "https://example.com/images/pic.png");
    }

    #[test]
    fn resolve_asset_url_rejects_non_http_schemes() {
        assert!(resolve_asset_url("mailto:test@example.com", None).is_none());
        assert!(resolve_asset_url("javascript:alert(1)", None).is_none());
    }

    #[test]
    fn normalize_image_content_type_only_accepts_images() {
        assert_eq!(
            normalize_image_content_type("image/png; charset=binary").as_deref(),
            Some("image/png")
        );
        assert_eq!(normalize_image_content_type("text/html"), None);
    }
}

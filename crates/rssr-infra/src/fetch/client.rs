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
    const IMAGE_USER_AGENT: &'static str =
        "Mozilla/5.0 (compatible; RSS-Reader image localizer; +https://github.com)";
    const LAZY_IMAGE_ATTRIBUTES: &'static [&'static str] =
        &["data-src", "data-original", "data-lazy-src", "data-orig-file"];

    pub fn new() -> Self {
        Self { inner: reqwest::Client::new() }
    }

    pub fn max_images_per_entry() -> usize {
        8
    }

    pub async fn localize_html_images(
        &self,
        html: &str,
        base_url: Option<&Url>,
    ) -> anyhow::Result<String> {
        const MAX_IMAGE_BYTES: usize = 1024 * 1024;
        const MAX_TOTAL_IMAGE_BYTES: usize = 2 * 1024 * 1024;
        const MAX_HTML_BYTES: usize = 256 * 1024;

        if !html.contains("<img") || html.len() > MAX_HTML_BYTES {
            return Ok(html.to_string());
        }

        let img_regex = Regex::new(r#"(?is)<img\b[^>]*>"#).expect("valid image tag regex");

        let mut sources = BTreeSet::new();
        for img_tag in img_regex.find_iter(html).map(|match_| match_.as_str()) {
            for raw in image_source_candidates(img_tag) {
                if sources.len() >= Self::max_images_per_entry() {
                    break;
                }
                if should_skip_asset(&raw) {
                    continue;
                }
                if let Some(resolved) = resolve_asset_url(&raw, base_url) {
                    sources.insert((raw, resolved));
                }
            }
            if sources.len() >= Self::max_images_per_entry() {
                break;
            }
        }

        if sources.is_empty() {
            return Ok(html.to_string());
        }

        let mut localized = BTreeMap::new();
        let mut total_localized_bytes = 0_usize;
        for (raw, resolved) in sources {
            match self.fetch_image_as_data_url(&resolved, base_url, MAX_IMAGE_BYTES).await {
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

        Ok(img_regex
            .replace_all(html, |captures: &regex::Captures<'_>| {
                let raw_tag = captures.get(0).map(|value| value.as_str()).unwrap_or_default();
                rewrite_localized_image_tag(raw_tag, &localized)
            })
            .into_owned())
    }

    async fn fetch_image_as_data_url(
        &self,
        url: &Url,
        referer: Option<&Url>,
        max_bytes: usize,
    ) -> anyhow::Result<Option<(String, usize)>> {
        let mut request =
            self.inner.get(url.clone()).header(header::USER_AGENT, Self::IMAGE_USER_AGENT).header(
                header::ACCEPT,
                "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8",
            );
        if let Some(referer) = referer {
            request = request.header(header::REFERER, referer.as_str());
        }

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
    raw.is_empty()
        || raw.starts_with("data:")
        || raw.starts_with("blob:")
        || looks_like_placeholder_asset(raw)
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

fn image_source_candidates(img_tag: &str) -> Vec<String> {
    let mut candidates = Vec::new();

    for attr in BodyAssetLocalizer::LAZY_IMAGE_ATTRIBUTES {
        if let Some(value) = quoted_attribute_value(img_tag, attr) {
            push_source_candidate(&mut candidates, &value);
        }
    }

    if let Some(value) = quoted_attribute_value(img_tag, "srcset") {
        for source in srcset_urls(&value) {
            push_source_candidate(&mut candidates, &source);
        }
    }

    if let Some(value) = quoted_attribute_value(img_tag, "src") {
        push_source_candidate(&mut candidates, &value);
    }

    candidates
}

fn push_source_candidate(candidates: &mut Vec<String>, raw: &str) {
    let raw = raw.trim();
    if raw.is_empty() || candidates.iter().any(|existing| existing == raw) {
        return;
    }
    candidates.push(raw.to_string());
}

fn quoted_attribute_value(tag: &str, attr: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)(?:^|[\s<]){}\s*=\s*(?:"(?P<dq>[^"]*)"|'(?P<sq>[^']*)')"#,
        regex::escape(attr)
    );
    let regex = Regex::new(&pattern).ok()?;
    let captures = regex.captures(tag)?;
    captures.name("dq").or_else(|| captures.name("sq")).map(|value| value.as_str().to_string())
}

fn srcset_urls(raw: &str) -> Vec<String> {
    raw.split(',')
        .filter_map(|candidate| candidate.split_whitespace().next())
        .filter(|candidate| !candidate.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn rewrite_localized_image_tag(tag: &str, localized: &BTreeMap<String, String>) -> String {
    let Some(rewritten_src) =
        image_source_candidates(tag).into_iter().find_map(|source| localized.get(&source).cloned())
    else {
        return tag.to_string();
    };

    let tag = remove_quoted_attribute(tag, "srcset");
    set_or_insert_quoted_attribute(&tag, "src", &rewritten_src)
}

fn remove_quoted_attribute(tag: &str, attr: &str) -> String {
    let pattern = format!(r#"(?is)\s+{}\s*=\s*(?:"[^"]*"|'[^']*')"#, regex::escape(attr));
    Regex::new(&pattern)
        .expect("valid quoted attribute removal regex")
        .replace_all(tag, "")
        .into_owned()
}

fn set_or_insert_quoted_attribute(tag: &str, attr: &str, value: &str) -> String {
    let pattern =
        format!(r#"(?is)(?P<prefix>(?:^|[\s<]){}\s*=\s*)(?:"[^"]*"|'[^']*')"#, regex::escape(attr));
    let regex = Regex::new(&pattern).expect("valid quoted attribute replacement regex");
    if regex.is_match(tag) {
        return regex
            .replace(tag, |captures: &regex::Captures<'_>| {
                let prefix = captures.name("prefix").map(|value| value.as_str()).unwrap_or("");
                format!("{prefix}\"{value}\"")
            })
            .into_owned();
    }

    if let Some(prefix) = tag.strip_suffix("/>") {
        return format!("{prefix} {attr}=\"{value}\"/>");
    }
    if let Some(prefix) = tag.strip_suffix('>') {
        return format!("{prefix} {attr}=\"{value}\">");
    }

    tag.to_string()
}

fn looks_like_placeholder_asset(raw: &str) -> bool {
    let lower = raw.to_ascii_lowercase();
    lower.contains("placeholder")
        || lower.ends_with("/blank.gif")
        || lower.ends_with("/transparent.gif")
        || lower.ends_with("/spacer.gif")
        || lower.ends_with("/1x1.gif")
        || lower.ends_with("/pixel.gif")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        image_source_candidates, normalize_image_content_type, resolve_asset_url,
        rewrite_localized_image_tag, srcset_urls,
    };
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

    #[test]
    fn image_source_candidates_prefers_lazy_and_srcset_before_src_placeholder() {
        let sources = image_source_candidates(
            r#"<img src="/blank.gif" data-src="/real.webp" srcset="/real.webp 1x, /real@2x.webp 2x">"#,
        );

        assert_eq!(sources, vec!["/real.webp", "/real@2x.webp", "/blank.gif"]);
    }

    #[test]
    fn srcset_urls_extracts_candidate_urls() {
        assert_eq!(
            srcset_urls("/small.jpg 480w, https://cdn.example.com/large.jpg 960w"),
            vec!["/small.jpg", "https://cdn.example.com/large.jpg"]
        );
    }

    #[test]
    fn rewrite_localized_image_tag_promotes_lazy_source_to_src() {
        let mut localized = BTreeMap::new();
        localized.insert("/real.webp".to_string(), "data:image/webp;base64,abcd".to_string());

        let rewritten = rewrite_localized_image_tag(
            r#"<img src="/blank.gif" data-src="/real.webp">"#,
            &localized,
        );

        assert_eq!(rewritten, r#"<img src="data:image/webp;base64,abcd" data-src="/real.webp">"#);
    }

    #[test]
    fn rewrite_localized_image_tag_drops_remote_srcset_when_local_src_is_available() {
        let mut localized = BTreeMap::new();
        localized.insert("/real.webp".to_string(), "data:image/webp;base64,abcd".to_string());

        let rewritten = rewrite_localized_image_tag(
            r#"<img src="/fallback.jpg" srcset="/real.webp 1x, /real@2x.webp 2x">"#,
            &localized,
        );

        assert_eq!(rewritten, r#"<img src="data:image/webp;base64,abcd">"#);
    }
}

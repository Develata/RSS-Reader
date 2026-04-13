use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use url::Url;

const LAZY_IMAGE_ATTRIBUTES: &[&str] =
    &["data-src", "data-original", "data-lazy-src", "data-orig-file"];

pub(crate) fn image_tag_regex() -> Regex {
    Regex::new(r#"(?is)<img\b[^>]*>"#).expect("valid image tag regex")
}

pub(crate) fn collect_localizable_sources(
    html: &str,
    base_url: Option<&Url>,
    max_images: usize,
) -> BTreeSet<(String, Url)> {
    let img_regex = image_tag_regex();
    let mut sources = BTreeSet::new();

    for img_tag in img_regex.find_iter(html).map(|match_| match_.as_str()) {
        for raw in image_source_candidates(img_tag) {
            if sources.len() >= max_images {
                break;
            }
            if should_skip_asset(&raw) {
                continue;
            }
            if let Some(resolved) = resolve_asset_url(&raw, base_url) {
                sources.insert((raw, resolved));
            }
        }

        if sources.len() >= max_images {
            break;
        }
    }

    sources
}

pub(crate) fn resolve_asset_url(raw: &str, base_url: Option<&Url>) -> Option<Url> {
    let resolved =
        Url::parse(raw).ok().or_else(|| base_url.and_then(|base| base.join(raw).ok()))?;
    matches!(resolved.scheme(), "http" | "https").then_some(resolved)
}

pub(crate) fn normalize_image_content_type(raw: &str) -> Option<String> {
    let mime = raw.split(';').next()?.trim().to_ascii_lowercase();
    mime.starts_with("image/").then_some(mime)
}

pub(crate) fn rewrite_localized_html(
    html: &str,
    img_regex: &Regex,
    localized: &BTreeMap<String, String>,
) -> String {
    img_regex
        .replace_all(html, |captures: &regex::Captures<'_>| {
            let raw_tag = captures.get(0).map(|value| value.as_str()).unwrap_or_default();
            rewrite_localized_image_tag(raw_tag, localized)
        })
        .into_owned()
}

fn should_skip_asset(raw: &str) -> bool {
    raw.is_empty()
        || raw.starts_with("data:")
        || raw.starts_with("blob:")
        || looks_like_placeholder_asset(raw)
}

fn image_source_candidates(img_tag: &str) -> Vec<String> {
    let mut candidates = Vec::new();

    for attr in LAZY_IMAGE_ATTRIBUTES {
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
        collect_localizable_sources, image_source_candidates, image_tag_regex,
        normalize_image_content_type, resolve_asset_url, rewrite_localized_html, srcset_urls,
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
    fn collect_localizable_sources_skips_placeholders_and_caps_total_images() {
        let base = Url::parse("https://example.com/posts/entry").expect("valid base url");
        let html = r#"
            <img src="/blank.gif" data-src="/real-1.webp">
            <img src="/real-2.webp">
            <img src="/real-3.webp">
        "#;

        let sources = collect_localizable_sources(html, Some(&base), 2);

        assert_eq!(sources.len(), 2);
        assert!(sources.contains(&(
            "/real-1.webp".to_string(),
            Url::parse("https://example.com/real-1.webp").expect("resolved url")
        )));
        assert!(sources.contains(&(
            "/real-2.webp".to_string(),
            Url::parse("https://example.com/real-2.webp").expect("resolved url")
        )));
    }

    #[test]
    fn srcset_urls_extracts_candidate_urls() {
        assert_eq!(
            srcset_urls("/small.jpg 480w, https://cdn.example.com/large.jpg 960w"),
            vec!["/small.jpg", "https://cdn.example.com/large.jpg"]
        );
    }

    #[test]
    fn rewrite_localized_html_only_rewrites_matching_image_tags() {
        let regex = image_tag_regex();
        let mut localized = BTreeMap::new();
        localized.insert("/real.webp".to_string(), "data:image/webp;base64,abcd".to_string());

        let rewritten = rewrite_localized_html(
            r#"<p>lead</p><img src="/real.webp"><img src="/keep.png">"#,
            &regex,
            &localized,
        );

        assert!(rewritten.contains(r#"<img src="data:image/webp;base64,abcd">"#));
        assert!(rewritten.contains(r#"<img src="/keep.png">"#));
    }

    #[test]
    fn rewrite_localized_html_promotes_lazy_source_and_drops_srcset() {
        let regex = image_tag_regex();
        let mut localized = BTreeMap::new();
        localized.insert("/real.webp".to_string(), "data:image/webp;base64,abcd".to_string());

        let rewritten = rewrite_localized_html(
            r#"<img src="/fallback.jpg" data-src="/real.webp" srcset="/real.webp 1x, /real@2x.webp 2x">"#,
            &regex,
            &localized,
        );

        assert_eq!(rewritten, r#"<img src="data:image/webp;base64,abcd" data-src="/real.webp">"#);
    }
}

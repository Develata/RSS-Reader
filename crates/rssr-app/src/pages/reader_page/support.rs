use std::sync::OnceLock;

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
use std::collections::BTreeMap;

use regex::Regex;
use time::{OffsetDateTime, UtcOffset, macros::format_description};
use url::Url;

#[cfg(not(target_arch = "wasm32"))]
use rssr_infra::fetch::normalize_html_for_live_display;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReaderBody {
    Html(String),
    Text(String),
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
pub(crate) const DESKTOP_IMAGE_PROXY_SCHEME: &str = "rssr-img";

pub(crate) fn select_reader_body(
    content_html: Option<String>,
    content_text: Option<String>,
    summary: Option<String>,
    base_url: Option<&Url>,
) -> ReaderBody {
    if let Some(html) = content_html.as_deref().and_then(|raw| sanitize_remote_html(raw, base_url))
    {
        return ReaderBody::Html(html);
    }

    if let Some(html) = content_text
        .as_deref()
        .filter(|raw| looks_like_html_fragment(raw))
        .and_then(|raw| sanitize_remote_html(raw, base_url))
    {
        return ReaderBody::Html(html);
    }

    if let Some(html) = summary
        .as_deref()
        .filter(|raw| looks_like_html_fragment(raw))
        .and_then(|raw| sanitize_remote_html(raw, base_url))
    {
        return ReaderBody::Html(html);
    }

    ReaderBody::Text(content_text.or(summary).unwrap_or_else(|| "暂无正文".to_string()))
}

pub(crate) fn sanitize_remote_html(raw: &str, base_url: Option<&Url>) -> Option<String> {
    let normalized = normalize_remote_html_for_reader(raw, base_url);
    let sanitized = ammonia::Builder::default()
        .add_tags(&["picture", "source"])
        .add_url_schemes(&["data"])
        .add_tag_attributes(
            "img",
            &[
                "class",
                "data-src",
                "data-original",
                "data-lazy-src",
                "data-orig-file",
                "data-srcset",
                "srcset",
                "sizes",
                "loading",
                "decoding",
                "fetchpriority",
                "media",
                "type",
            ],
        )
        .add_tag_attributes("source", &["src", "srcset", "data-srcset", "sizes", "media", "type"])
        .clean(&normalized)
        .to_string();
    let sanitized = rewrite_reader_media_sources_for_runtime(&sanitized, base_url);
    let trimmed = sanitized.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn normalize_remote_html_for_reader(raw: &str, base_url: Option<&Url>) -> String {
    let mut output = String::with_capacity(raw.len());
    let mut cursor = 0_usize;

    for matched in img_tag_regex().find_iter(raw) {
        output.push_str(&raw[cursor..matched.start()]);
        let raw_tag = matched.as_str();
        if let Some(replacement) = wordpress_emoji_replacement(raw_tag) {
            output.push_str(&replacement);
        } else {
            output.push_str(raw_tag);
        }
        cursor = matched.end();
    }

    output.push_str(&raw[cursor..]);
    normalize_live_display_html(&output, base_url)
}

fn img_tag_regex() -> &'static Regex {
    static IMG_TAG_REGEX: OnceLock<Regex> = OnceLock::new();
    IMG_TAG_REGEX.get_or_init(|| Regex::new(r#"(?is)<img\b[^>]*>"#).expect("valid img regex"))
}

fn wordpress_emoji_replacement(raw_tag: &str) -> Option<String> {
    let attributes = parse_tag_attributes(raw_tag)?;
    let alt = tag_attribute_value(&attributes, "alt")?.trim();
    if alt.is_empty() {
        return None;
    }

    let class = tag_attribute_value(&attributes, "class").unwrap_or_default();
    let src = tag_attribute_value(&attributes, "src").unwrap_or_default();
    if looks_like_wordpress_emoji_class(class) || looks_like_wordpress_emoji_src(src) {
        Some(escape_html_text(alt))
    } else {
        None
    }
}

fn parse_tag_attributes(raw_tag: &str) -> Option<Vec<HtmlAttribute>> {
    parse_html_tag(raw_tag)
        .and_then(|tag| tag.name.eq_ignore_ascii_case("img").then_some(tag.attributes))
}

fn parse_html_tag(raw_tag: &str) -> Option<HtmlTag> {
    if raw_tag.len() < 5 || !raw_tag.starts_with('<') || !raw_tag.ends_with('>') {
        return None;
    }

    let inner = raw_tag[1..raw_tag.len() - 1].trim();
    let self_closing = inner.ends_with('/');
    let inner = inner.trim_end_matches('/').trim_end();
    let name_end = inner.find(|ch: char| ch.is_ascii_whitespace()).unwrap_or(inner.len());
    if name_end == 0 {
        return None;
    }

    Some(HtmlTag {
        name: inner[..name_end].to_ascii_lowercase(),
        original_name: inner[..name_end].to_string(),
        attributes: parse_attributes(&inner[name_end..]),
        self_closing,
    })
}

fn parse_attributes(raw: &str) -> Vec<HtmlAttribute> {
    let bytes = raw.as_bytes();
    let mut attributes = Vec::new();
    let mut index = 0_usize;

    while index < bytes.len() {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() || bytes[index] == b'/' {
            break;
        }

        let name_start = index;
        while index < bytes.len()
            && !bytes[index].is_ascii_whitespace()
            && bytes[index] != b'='
            && bytes[index] != b'/'
        {
            index += 1;
        }

        if name_start == index {
            index += 1;
            continue;
        }

        let original_name = raw[name_start..index].to_string();
        let name = original_name.to_ascii_lowercase();
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }

        let value = if index < bytes.len() && bytes[index] == b'=' {
            index += 1;
            while index < bytes.len() && bytes[index].is_ascii_whitespace() {
                index += 1;
            }
            let parsed = parse_attribute_value(raw, bytes, &mut index);
            Some(parsed)
        } else {
            None
        };

        attributes.push(HtmlAttribute { name, original_name, value });
    }

    attributes
}

fn parse_attribute_value(raw: &str, bytes: &[u8], index: &mut usize) -> String {
    if *index >= bytes.len() {
        return String::new();
    }

    if bytes[*index] == b'"' || bytes[*index] == b'\'' {
        let quote = bytes[*index];
        *index += 1;
        let value_start = *index;
        while *index < bytes.len() && bytes[*index] != quote {
            *index += 1;
        }
        let raw_value = raw[value_start..*index].to_string();
        if *index < bytes.len() {
            *index += 1;
        }
        return decode_html_attribute_entities(&raw_value);
    }

    let value_start = *index;
    while *index < bytes.len() && !bytes[*index].is_ascii_whitespace() && bytes[*index] != b'/' {
        *index += 1;
    }
    let raw_value = raw[value_start..*index].to_string();
    decode_html_attribute_entities(&raw_value)
}

fn tag_attribute_value<'a>(attributes: &'a [HtmlAttribute], name: &str) -> Option<&'a str> {
    attributes
        .iter()
        .find(|attribute| attribute.name == name)
        .and_then(|attribute| attribute.value.as_deref())
}

fn normalize_live_display_html(raw: &str, base_url: Option<&Url>) -> String {
    #[cfg(not(target_arch = "wasm32"))]
    {
        normalize_html_for_live_display(raw, base_url)
    }

    #[cfg(target_arch = "wasm32")]
    {
        let _ = base_url;
        raw.to_string()
    }
}

fn rewrite_reader_media_sources_for_runtime(raw: &str, base_url: Option<&Url>) -> String {
    let _ = base_url;
    raw.to_string()
}

#[allow(dead_code)]
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn rewrite_media_tag_for_desktop_runtime(tag: &HtmlTag, base_url: Option<&Url>) -> String {
    let mut changes = BTreeMap::new();

    match tag.name.as_str() {
        "img" => {
            if let Some(proxy_src) = tag_attribute_value(&tag.attributes, "src")
                .and_then(|value| resolve_runtime_image_url(value, base_url))
                .map(|target| build_desktop_image_proxy_url(&target, base_url))
            {
                changes.insert("src".to_string(), Some(proxy_src));
                changes.insert("srcset".to_string(), None);
                changes.insert("data-src".to_string(), None);
                changes.insert("data-original".to_string(), None);
                changes.insert("data-lazy-src".to_string(), None);
                changes.insert("data-orig-file".to_string(), None);
                changes.insert("data-srcset".to_string(), None);
                changes.insert("sizes".to_string(), None);
                changes.insert("loading".to_string(), None);
                changes.insert("fetchpriority".to_string(), None);
            }
        }
        "source" => {
            if let Some(proxy_src) = tag_attribute_value(&tag.attributes, "src")
                .and_then(|value| resolve_runtime_image_url(value, base_url))
                .map(|target| build_desktop_image_proxy_url(&target, base_url))
            {
                changes.insert("src".to_string(), Some(proxy_src));
            }
            if let Some(proxy_srcset) = tag_attribute_value(&tag.attributes, "srcset")
                .and_then(|value| proxy_srcset(value, base_url))
            {
                changes.insert("srcset".to_string(), Some(proxy_srcset));
                changes.insert("data-srcset".to_string(), None);
            }
        }
        _ => {}
    }

    if changes.is_empty() { raw_html_tag(tag) } else { rewrite_tag_attributes(tag, &changes) }
}

#[allow(dead_code)]
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
pub(crate) fn build_desktop_image_proxy_url(target: &Url, referer: Option<&Url>) -> String {
    let mut query = url::form_urlencoded::Serializer::new(String::new());
    query.append_pair("target", target.as_str());
    if let Some(referer) = referer {
        query.append_pair("referer", referer.as_str());
    }

    format!("{DESKTOP_IMAGE_PROXY_SCHEME}://fetch?{}", query.finish())
}

#[allow(dead_code)]
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn resolve_runtime_image_url(raw: &str, base_url: Option<&Url>) -> Option<Url> {
    Url::parse(raw)
        .ok()
        .or_else(|| base_url.and_then(|base| base.join(raw).ok()))
        .filter(|url| matches!(url.scheme(), "http" | "https"))
}

#[allow(dead_code)]
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn proxy_srcset(raw: &str, base_url: Option<&Url>) -> Option<String> {
    let proxied = raw
        .split(',')
        .filter_map(|candidate| {
            let candidate = candidate.trim();
            if candidate.is_empty() {
                return None;
            }

            let (url, descriptor) = split_srcset_candidate(candidate)?;
            let target = resolve_runtime_image_url(url, base_url)?;
            let proxied = build_desktop_image_proxy_url(&target, base_url);

            Some(if descriptor.is_empty() { proxied } else { format!("{proxied} {descriptor}") })
        })
        .collect::<Vec<_>>();

    (!proxied.is_empty()).then(|| proxied.join(", "))
}

#[allow(dead_code)]
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn split_srcset_candidate(raw: &str) -> Option<(&str, &str)> {
    let url_end = raw.find(char::is_whitespace).unwrap_or(raw.len());
    let url = raw[..url_end].trim();
    (!url.is_empty()).then_some((url, raw[url_end..].trim()))
}

#[allow(dead_code)]
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn raw_html_tag(tag: &HtmlTag) -> String {
    rewrite_tag_attributes(tag, &BTreeMap::new())
}

#[allow(dead_code)]
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn rewrite_tag_attributes(tag: &HtmlTag, changes: &BTreeMap<String, Option<String>>) -> String {
    let mut output = String::from("<");
    output.push_str(&tag.original_name);

    let mut applied = BTreeMap::new();
    for attribute in &tag.attributes {
        let Some(change) = changes.get(&attribute.name) else {
            push_attribute(&mut output, &attribute.original_name, attribute.value.as_deref());
            continue;
        };

        applied.insert(attribute.name.clone(), true);
        if let Some(value) = change {
            push_attribute(&mut output, &attribute.original_name, Some(value));
        }
    }

    for (name, value) in changes {
        if applied.contains_key(name) {
            continue;
        }
        if let Some(value) = value {
            push_attribute(&mut output, name, Some(value));
        }
    }

    if tag.self_closing {
        output.push_str("/>");
    } else {
        output.push('>');
    }
    output
}

#[allow(dead_code)]
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn push_attribute(output: &mut String, name: &str, value: Option<&str>) {
    output.push(' ');
    output.push_str(name);
    if let Some(value) = value {
        output.push_str("=\"");
        output.push_str(&escape_html_attribute(value));
        output.push('"');
    }
}

fn looks_like_wordpress_emoji_class(raw: &str) -> bool {
    raw.split_ascii_whitespace().any(|class_name| {
        class_name.eq_ignore_ascii_case("wp-smiley")
            || class_name.eq_ignore_ascii_case("emoji")
            || class_name.eq_ignore_ascii_case("wp-emoji")
    })
}

fn looks_like_wordpress_emoji_src(raw: &str) -> bool {
    let lower = raw.to_ascii_lowercase();
    lower.contains("s.w.org/images/core/emoji/") || lower.contains("/wp-includes/images/smilies/")
}

fn escape_html_text(raw: &str) -> String {
    raw.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn decode_html_attribute_entities(raw: &str) -> String {
    let mut decoded = raw.to_string();
    for _ in 0..8 {
        let next = decode_html_attribute_entities_once(&decoded);
        if next == decoded {
            return next;
        }
        decoded = next;
    }

    decoded
}

fn decode_html_attribute_entities_once(raw: &str) -> String {
    if !raw.contains('&') {
        return raw.to_string();
    }

    let mut output = String::with_capacity(raw.len());
    let mut cursor = 0_usize;

    while let Some(relative_start) = raw[cursor..].find('&') {
        let start = cursor + relative_start;
        output.push_str(&raw[cursor..start]);

        let Some(relative_end) = raw[start + 1..].find(';') else {
            output.push_str(&raw[start..]);
            return output;
        };

        let end = start + 1 + relative_end;
        let entity = &raw[start + 1..end];
        if let Some(decoded) = decode_html_entity(entity) {
            output.push(decoded);
            cursor = end + 1;
        } else {
            output.push('&');
            cursor = start + 1;
        }
    }

    output.push_str(&raw[cursor..]);
    output
}

fn decode_html_entity(entity: &str) -> Option<char> {
    match entity {
        "amp" => Some('&'),
        "quot" => Some('"'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "apos" => Some('\''),
        "nbsp" => Some('\u{a0}'),
        _ => decode_numeric_html_entity(entity),
    }
}

fn decode_numeric_html_entity(entity: &str) -> Option<char> {
    let value = if let Some(hex) = entity.strip_prefix("#x").or_else(|| entity.strip_prefix("#X")) {
        u32::from_str_radix(hex, 16).ok()?
    } else if let Some(decimal) = entity.strip_prefix('#') {
        decimal.parse::<u32>().ok()?
    } else {
        return None;
    };

    char::from_u32(value)
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn escape_html_attribute(raw: &str) -> String {
    raw.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HtmlAttribute {
    name: String,
    original_name: String,
    value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HtmlTag {
    name: String,
    original_name: String,
    attributes: Vec<HtmlAttribute>,
    self_closing: bool,
}

fn looks_like_html_fragment(raw: &str) -> bool {
    let trimmed = raw.trim();
    if !(trimmed.starts_with('<') && trimmed.contains('>')) {
        return false;
    }

    let lower = trimmed.to_ascii_lowercase();
    [
        "<p",
        "<div",
        "<article",
        "<section",
        "<blockquote",
        "<ul",
        "<ol",
        "<li",
        "<a ",
        "<img",
        "<br",
        "<hr",
        "<h1",
        "<h2",
        "<h3",
        "<h4",
        "<h5",
        "<h6",
        "<table",
        "<pre",
        "<code",
    ]
    .iter()
    .any(|tag| lower.contains(tag))
}

pub(crate) fn format_reader_datetime_utc(published_at: Option<OffsetDateTime>) -> Option<String> {
    const READER_DATETIME_FORMAT: &[time::format_description::FormatItem<'static>] =
        format_description!("[year]-[month]-[day] [hour]:[minute] UTC");

    published_at
        .and_then(|value| value.to_offset(UtcOffset::UTC).format(READER_DATETIME_FORMAT).ok())
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;
    use url::Url;

    use super::{ReaderBody, format_reader_datetime_utc, sanitize_remote_html, select_reader_body};

    #[test]
    fn reader_prefers_full_html_over_summary_text() {
        let body = select_reader_body(
            Some("<article><p>Full body</p></article>".to_string()),
            Some("Summary teaser".to_string()),
            Some("Summary teaser".to_string()),
            None,
        );

        assert_eq!(body, ReaderBody::Html("<article><p>Full body</p></article>".to_string()));
    }

    #[test]
    fn reader_sanitizes_remote_html() {
        let body = select_reader_body(
            Some(r#"<p onclick="alert(1)">Hello</p><script>alert(2)</script>"#.to_string()),
            None,
            None,
            None,
        );

        match body {
            ReaderBody::Html(html) => {
                assert!(html.contains("<p>Hello</p>"));
                assert!(!html.contains("onclick"));
                assert!(!html.contains("<script"));
            }
            ReaderBody::Text(_) => panic!("expected html body"),
        }
    }

    #[test]
    fn reader_preserves_image_source_attributes_after_sanitizing() {
        let body = select_reader_body(
            Some(
                r#"<p><img src="/fallback.jpg" srcset="/image.webp 1x, /image@2x.webp 2x" data-src="/image.webp" data-srcset="/lazy.webp 1x, /lazy@2x.webp 2x" onerror="alert(1)"></p>"#
                    .to_string(),
            ),
            None,
            None,
            None,
        );

        match body {
            ReaderBody::Html(html) => {
                assert!(html.contains(r#"src="/fallback.jpg""#));
                assert!(html.contains(r#"data-src="/image.webp""#));
                assert!(!html.contains("onerror"));
                #[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
                {
                    assert!(html.contains(r#"srcset="/lazy.webp 1x, /lazy@2x.webp 2x""#));
                    assert!(!html.contains("data-srcset="));
                }
                #[cfg(any(target_arch = "wasm32", target_os = "android"))]
                {
                    assert!(html.contains(r#"srcset="/image.webp 1x, /image@2x.webp 2x""#));
                    assert!(html.contains(r#"data-srcset="/lazy.webp 1x, /lazy@2x.webp 2x""#));
                }
            }
            ReaderBody::Text(_) => panic!("expected html body"),
        }
    }

    #[test]
    fn reader_preserves_picture_and_source_tags_after_sanitizing() {
        let body = select_reader_body(
            Some(
                r#"<picture><source srcset="/hero.avif 1x, /hero@2x.avif 2x" type="image/avif" media="(min-width: 800px)"><img src="/hero.jpg" sizes="100vw"></picture>"#
                    .to_string(),
            ),
            None,
            None,
            None,
        );

        match body {
            ReaderBody::Html(html) => {
                assert!(html.contains("<picture>"));
                assert!(html.contains("<source"));
                assert!(html.contains(r#"srcset="/hero.avif 1x, /hero@2x.avif 2x""#));
                assert!(html.contains(r#"type="image/avif""#));
                assert!(html.contains(r#"media="(min-width: 800px)""#));
                assert!(html.contains(r#"sizes="100vw""#));
            }
            ReaderBody::Text(_) => panic!("expected html body"),
        }
    }

    #[test]
    fn reader_replaces_wordpress_emoji_images_with_alt_text() {
        let sanitized = sanitize_remote_html(
            r#"<p><img src="https://s.w.org/images/core/emoji/17.0.2/72x72/1f4f9.png" alt="📹" class="wp-smiley"> Check out this session.</p>"#,
            None,
        )
        .expect("sanitized html");

        assert!(sanitized.contains("📹"));
        assert!(sanitized.contains("Check out this session."));
        assert!(!sanitized.contains("<img"));
        assert!(!sanitized.contains("s.w.org"));
    }

    #[test]
    fn reader_preserves_localized_data_url_images_after_sanitizing() {
        let sanitized = sanitize_remote_html(
            r#"<p><img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAAB" alt="localized chart"></p>"#,
            None,
        )
        .expect("sanitized html");

        assert!(
            sanitized.contains(r#"src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAAB""#)
        );
        assert!(sanitized.contains(r#"alt="localized chart""#));
    }

    #[test]
    fn reader_treats_html_like_summary_as_html_fallback() {
        let body = select_reader_body(
            None,
            Some(
                "<p>Summary fallback</p><a href=\"https://example.com\">Read more</a>".to_string(),
            ),
            None,
            None,
        );

        match body {
            ReaderBody::Html(html) => {
                assert!(html.contains("<p>Summary fallback</p>"));
                assert!(html.contains("Read more"));
            }
            ReaderBody::Text(_) => panic!("expected html body"),
        }
    }

    #[test]
    fn reader_formats_published_time_in_utc_without_seconds() {
        let published_at = OffsetDateTime::parse(
            "2026-03-29T19:45:33+08:00",
            &time::format_description::well_known::Rfc3339,
        )
        .expect("parse rfc3339");

        assert_eq!(
            format_reader_datetime_utc(Some(published_at)).as_deref(),
            Some("2026-03-29 11:45 UTC")
        );
    }

    #[test]
    fn reader_resolves_relative_lazy_images_before_rendering() {
        let article = Url::parse("https://example.com/posts/entry").expect("article");
        let sanitized = sanitize_remote_html(
            r#"<base href="https://cdn.example.com/assets/"><p><img src="/blank.gif" data-src="hero.png" data-srcset="hero.png 1x, hero@2x.png 2x"></p>"#,
            Some(&article),
        )
        .expect("sanitized html");

        assert!(sanitized.contains(r#"src="https://cdn.example.com/assets/hero.png""#));
        assert!(!sanitized.contains(r#"src="/blank.gif""#));
    }

    #[test]
    fn reader_keeps_remote_images_direct_after_normalizing() {
        let article = Url::parse("https://blogs.nvidia.com/blog/example-post/").expect("article");
        let sanitized = sanitize_remote_html(
            r#"<p><img src="https://blogs.nvidia.com/wp-content/uploads/2026/04/Filmora-1680x945.png" loading="lazy" srcset="https://blogs.nvidia.com/wp-content/uploads/2026/04/Filmora.png 1920w"></p>"#,
            Some(&article),
        )
        .expect("sanitized html");

        assert!(sanitized.contains(
            r#"src="https://blogs.nvidia.com/wp-content/uploads/2026/04/Filmora-1680x945.png""#,
        ));
        assert!(!sanitized.contains("srcset="));
        assert!(!sanitized.contains(r#"loading="lazy""#));
    }

    #[test]
    fn reader_normalization_canonicalizes_polluted_alt_entities() {
        let article = Url::parse("https://blogs.nvidia.com/blog/example-post/").expect("article");
        let sanitized = sanitize_remote_html(
            r#"<p><img src="https://blogs.nvidia.com/wp-content/uploads/2026/04/Filmora.png" alt="Image describing the &amp;amp;quot;inference iceberg.&amp;amp;quot; &amp;amp; platform tradeoffs"></p>"#,
            Some(&article),
        )
        .expect("sanitized html");

        assert!(
            sanitized.contains(
                r#"src="https://blogs.nvidia.com/wp-content/uploads/2026/04/Filmora.png""#,
            )
        );
        assert!(sanitized.contains(
            r#"alt="Image describing the &quot;inference iceberg.&quot; &amp; platform tradeoffs""#,
        ));
        assert!(!sanitized.contains("&amp;amp;quot;"));
        assert!(!sanitized.contains("&amp;amp;"));
    }
}

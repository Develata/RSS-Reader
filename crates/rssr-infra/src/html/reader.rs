//! 阅读页正文的消毒与归一化。
//!
//! 这些逻辑此前住在 `rssr-app/src/pages/reader_page/support.rs`，连带一整套与
//! [`super::live_display`] 重复的标签解析器和 HTML 实体解码。页面层只应该负责显示，
//! 解析和消毒是基础设施职责，因此收敛到这里并复用同一个解析器。

use std::borrow::Cow;

use url::Url;

use super::live_display::{
    attribute_value, looks_like_wordpress_emoji_asset, normalize_html_for_live_display,
    parse_html_tags, srcset_candidates,
};

/// 判断一段纯文本字段里是否其实塞的是 HTML 片段。
///
/// 不少 feed 会把完整 HTML 放进 `summary`/`content_text`，这时按纯文本渲染会把标签当字面量
/// 显示出来。
pub fn looks_like_html_fragment(raw: &str) -> bool {
    const HTML_FRAGMENT_MARKERS: &[&str] = &[
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
    ];

    let trimmed = raw.trim();
    if !(trimmed.starts_with('<') && trimmed.contains('>')) {
        return false;
    }

    let lower = trimmed.to_ascii_lowercase();
    HTML_FRAGMENT_MARKERS.iter().any(|marker| lower.contains(marker))
}

/// 把远端正文 HTML 处理成可以直接塞进阅读页的安全片段。
///
/// 顺序是有意义的：先做 emoji 替换和地址归一化（需要看到原始标签和 `data-*` 懒加载属性），
/// 再交给 ammonia 消毒。反过来做的话，归一化依赖的属性已经被消毒掉了。
///
/// 返回 `None` 表示消毒后没有任何可显示内容，调用方应回退到纯文本。
pub fn sanitize_reader_html(raw: &str, base_url: Option<&Url>) -> Option<String> {
    let normalized =
        normalize_html_for_live_display(&replace_wordpress_emoji_images(raw), base_url);
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
        .attribute_filter(filter_data_urls)
        .clean(&normalized)
        .to_string();

    let trimmed = sanitized.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// 白名单加 `data` 只为放行本地化后的图片（`img src="data:image/…"`）。ammonia 的 scheme
/// 白名单是全局的，`<a href="data:text/html,…">` 之类也会被一起放过，这里把它们去掉。
///
/// 只看承载 URL 的属性：`alt="data: …"` 这类文本与 URL 无关。scheme 用 `Url::parse` 判定，
/// 与 ammonia 和浏览器一致（WHATWG 会剔除 URL 中的制表与换行，只比字符串前缀会被 `da\nta:`
/// 绕过）。`srcset` 是候选列表，ammonia 不检查它的 scheme，这里逐个候选检查，只删掉不合规的。
fn filter_data_urls<'u>(element: &str, attribute: &str, value: &'u str) -> Option<Cow<'u, str>> {
    const URL_ATTRIBUTES: &[&str] =
        &["href", "src", "srcset", "action", "formaction", "ping", "poster", "cite", "data"];
    if !URL_ATTRIBUTES.contains(&attribute) {
        return Some(Cow::Borrowed(value));
    }
    let image_slot = matches!(element, "img" | "source") && matches!(attribute, "src" | "srcset");
    let allowed = |url: &str| match Url::parse(url) {
        Ok(url) if url.scheme() == "data" => image_slot && is_image_data_url(&url),
        _ => true,
    };
    if attribute != "srcset" {
        return allowed(value).then_some(Cow::Borrowed(value));
    }
    let candidates = srcset_candidates(value).collect::<Vec<_>>();
    if candidates.iter().all(|candidate| allowed(candidate.url)) {
        return Some(Cow::Borrowed(value));
    }
    let kept = candidates
        .iter()
        .filter(|candidate| allowed(candidate.url))
        .map(|candidate| candidate.with_url(candidate.url))
        .collect::<Vec<_>>();
    (!kept.is_empty()).then(|| Cow::Owned(kept.join(", ")))
}

/// `data:` URL 的媒体类型必须是完整的 `image/<subtype>`，按 Fetch 的 “data: URL processor”
/// 取：序列化整个 URL（含 query、去掉 fragment），去掉 `data:`，第一个逗号之前就是媒体类型，
/// 没有逗号即无效。`Url::path()` 不含 query：`data:image/png?x,…` 的媒体类型其实是
/// `image/png?x`，解析失败后按 `text/plain` 处理（独立评审）。`data:image/;base64,…` 同样不是图片。
fn is_image_data_url(url: &Url) -> bool {
    let Some(body) = url[..url::Position::AfterQuery].strip_prefix("data:") else {
        return false;
    };
    let Some((media_type, _data)) = body.split_once(',') else {
        return false;
    };
    let essence = media_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim_matches(|c: char| c.is_ascii_whitespace());
    let Some((kind, subtype)) = essence.split_once('/') else {
        return false;
    };
    kind.eq_ignore_ascii_case("image") && !subtype.is_empty() && subtype.bytes().all(is_token_byte)
}

/// RFC 9110 §5.6.2 的 `tchar`。
fn is_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
}

/// 把 WordPress 的 emoji 图片换回它的 `alt` 文本。
///
/// 这类站点会把每个 emoji 渲染成一张 `s.w.org` 上的小图；照原样保留会让阅读页为一堆
/// 表情符号发远程请求，离线时还会留下一片裂图。
fn replace_wordpress_emoji_images(raw: &str) -> String {
    let tags = parse_html_tags(raw);
    if tags.is_empty() {
        return raw.to_string();
    }

    let mut output = String::with_capacity(raw.len());
    let mut cursor = 0_usize;

    for tag in &tags {
        if tag.is_end_tag || tag.name != "img" {
            continue;
        }
        let Some(alt) = wordpress_emoji_alt_text(tag) else {
            continue;
        };

        output.push_str(&raw[cursor..tag.start]);
        output.push_str(&escape_html_text(alt));
        cursor = tag.end;
    }

    if cursor == 0 {
        return raw.to_string();
    }

    output.push_str(&raw[cursor..]);
    output
}

fn wordpress_emoji_alt_text(tag: &super::live_display::HtmlTag) -> Option<&str> {
    let alt = attribute_value(tag, "alt")?.trim();
    if alt.is_empty() {
        return None;
    }

    let class = attribute_value(tag, "class").unwrap_or_default();
    let src = attribute_value(tag, "src").unwrap_or_default();
    (looks_like_wordpress_emoji_class(class) || looks_like_wordpress_emoji_asset(src))
        .then_some(alt)
}

/// emoji 的 `alt` 是远端内容，替换成文本节点时必须转义，否则 `alt` 里的 `<` 会变成标签。
/// 后面还会过一遍 ammonia，这里是纵深防御。
fn escape_html_text(raw: &str) -> String {
    raw.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn looks_like_wordpress_emoji_class(raw: &str) -> bool {
    raw.split_ascii_whitespace().any(|class_name| {
        class_name.eq_ignore_ascii_case("wp-smiley")
            || class_name.eq_ignore_ascii_case("emoji")
            || class_name.eq_ignore_ascii_case("wp-emoji")
    })
}

#[cfg(test)]
mod tests {
    use url::Url;

    use super::{looks_like_html_fragment, sanitize_reader_html};

    #[test]
    fn strips_scripts_and_event_handlers() {
        let sanitized = sanitize_reader_html(
            r#"<p onclick="alert(1)">Hello</p><script>alert(2)</script>"#,
            None,
        )
        .expect("sanitized html");

        assert!(sanitized.contains("<p>Hello</p>"));
        assert!(!sanitized.contains("onclick"));
        assert!(!sanitized.contains("<script"));
    }

    #[test]
    fn replaces_wordpress_emoji_images_with_alt_text() {
        let sanitized = sanitize_reader_html(
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
    fn keeps_localized_data_url_images() {
        let sanitized = sanitize_reader_html(
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
    fn data_urls_survive_only_as_image_sources() {
        let sanitized = sanitize_reader_html(
            "<p><a href=\"data:text/html;base64,PHNjcmlwdD4=\">a</a>\
             <a href=\"da\nta:text/html,x\">b</a>\
             <a href=\"data:image/png;base64,iVBORw0KGgo=\">c</a>\
             <img src=\"data:text/html,x\" alt=\"html\">\
             <img src=\"data:image/svg+xml;base64,PHN2Zy8+\" alt=\"svg\">\
             <img srcset=\"data:image/png;base64,iVBORw0KGgo= 1x\" alt=\"data: caption\">\
             <a href=\"https://example.com/\">d</a></p>",
            None,
        )
        .expect("sanitized html");

        assert!(!sanitized.contains("text/html"), "{sanitized}");
        assert!(!sanitized.contains(r#"href="data:image"#), "{sanitized}");
        assert!(sanitized.contains(r#"src="data:image/svg+xml;base64,PHN2Zy8+""#), "{sanitized}");
        assert!(
            sanitized.contains(r#"srcset="data:image/png;base64,iVBORw0KGgo= 1x""#),
            "{sanitized}"
        );
        assert!(sanitized.contains(r#"alt="data: caption""#), "{sanitized}");
        assert!(sanitized.contains(r#"href="https://example.com/""#), "{sanitized}");
        assert_eq!(sanitized.matches("<a").count(), 4, "{sanitized}");
    }

    /// srcset 逐个候选检查：非图片的 data 候选被删掉，其余候选保留（独立评审的反例）；
    /// 媒体类型必须是完整的 `image/<subtype>`。
    #[test]
    fn srcset_candidates_and_media_types_are_checked_one_by_one() {
        let sanitized = sanitize_reader_html(
            r#"<p><picture><source srcset="https://e.test/a.png 1x, data:text/html,evil 2x"><img src="https://e.test/a.png" alt="a"></picture>
               <picture><source srcset="data:image/png;base64,AAAA 1x, data:text/html,x 2x"><img src="https://e.test/b.png" alt="b"></picture>
               <img src="data:image/;base64,WA==" alt="empty subtype">
               <img src="data:image/png?x,evil" alt="query in media type">
               <img src="data:image/png" alt="no data comma">
               <picture><source srcset="https://e.test/d.png 1x, data:image/png?x,evil 2x"><img src="https://e.test/d.png" alt="d"></picture>
               <picture><source srcset="data:text/plain,x 1x"><img src="https://e.test/c.png" alt="c"></picture></p>"#,
            None,
        )
        .expect("sanitized html");

        assert!(!sanitized.contains("text/html"), "{sanitized}");
        assert!(!sanitized.contains("text/plain"), "{sanitized}");
        assert!(!sanitized.contains("data:image/;"), "{sanitized}");
        assert!(sanitized.contains(r#"srcset="https://e.test/a.png 1x""#), "{sanitized}");
        assert!(sanitized.contains(r#"srcset="data:image/png;base64,AAAA 1x""#), "{sanitized}");
        assert!(sanitized.contains(r#"alt="empty subtype""#), "{sanitized}");
        assert!(!sanitized.contains("evil"), "{sanitized}");
        assert!(!sanitized.contains(r#"src="data:image/png""#), "{sanitized}");
        assert!(sanitized.contains(r#"srcset="https://e.test/d.png 1x""#), "{sanitized}");
        assert_eq!(sanitized.matches("srcset=").count(), 3, "{sanitized}");
    }

    #[test]
    fn resolves_relative_lazy_images_against_document_base() {
        let article = Url::parse("https://example.com/posts/entry").expect("article");
        let sanitized = sanitize_reader_html(
            r#"<base href="https://cdn.example.com/assets/"><p><img src="/blank.gif" data-src="hero.png" data-srcset="hero.png 1x, hero@2x.png 2x"></p>"#,
            Some(&article),
        )
        .expect("sanitized html");

        assert!(sanitized.contains(r#"src="https://cdn.example.com/assets/hero.png""#));
        assert!(!sanitized.contains(r#"src="/blank.gif""#));
    }

    #[test]
    fn returns_none_when_nothing_displayable_survives() {
        assert_eq!(sanitize_reader_html("<script>alert(1)</script>", None), None);
    }

    #[test]
    fn detects_html_fragments_in_text_fields() {
        assert!(looks_like_html_fragment("<p>Summary fallback</p>"));
        assert!(!looks_like_html_fragment("plain summary text"));
        assert!(!looks_like_html_fragment("2 > 1 and 1 < 2"));
    }
}

//! 阅读页正文的消毒与归一化。
//!
//! 这些逻辑此前住在 `rssr-app/src/pages/reader_page/support.rs`，连带一整套与
//! [`super::live_display`] 重复的标签解析器和 HTML 实体解码。页面层只应该负责显示，
//! 解析和消毒是基础设施职责，因此收敛到这里并复用同一个解析器。

use url::Url;

use super::live_display::{
    attribute_value, looks_like_wordpress_emoji_asset, normalize_html_for_live_display,
    parse_html_tags,
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
        .clean(&normalized)
        .to_string();

    let trimmed = sanitized.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
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

//! 阅读页的显示辅助。
//!
//! 这里只做「显示哪一份正文」这一个选择。正文的解析、地址归一化与消毒全部在
//! `rssr_infra::html`：本文件曾经自带一整套标签解析器、HTML 实体解码和 WordPress emoji
//! 启发式（与 infra 里的实现重复）。时间格式化已并入 `crate::datetime`。

use rssr_infra::html::{looks_like_html_fragment, sanitize_reader_html};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReaderBody {
    Html(String),
    Text(String),
}

/// 按优先级挑选可显示的正文：完整 HTML > 看起来是 HTML 的文本字段 > 摘要 > 纯文本兜底。
///
/// 不少 feed 只在 `summary` 里放完整 HTML，所以文本字段也要过一遍 HTML 判定，否则会把标签
/// 当字面量显示出来。
pub(crate) fn select_reader_body(
    content_html: Option<String>,
    content_text: Option<String>,
    summary: Option<String>,
    base_url: Option<&Url>,
) -> ReaderBody {
    if let Some(html) = content_html.as_deref().and_then(|raw| sanitize_reader_html(raw, base_url))
    {
        return ReaderBody::Html(html);
    }

    for candidate in [content_text.as_deref(), summary.as_deref()] {
        if let Some(html) = candidate
            .filter(|raw| looks_like_html_fragment(raw))
            .and_then(|raw| sanitize_reader_html(raw, base_url))
        {
            return ReaderBody::Html(html);
        }
    }

    ReaderBody::Text(content_text.or(summary).unwrap_or_else(|| "暂无正文".to_string()))
}

#[cfg(test)]
mod tests {
    use super::{ReaderBody, select_reader_body};

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
    fn reader_falls_back_to_plain_text_when_nothing_survives_sanitizing() {
        let body = select_reader_body(
            Some("<script>alert(1)</script>".to_string()),
            Some("plain text body".to_string()),
            None,
            None,
        );

        assert_eq!(body, ReaderBody::Text("plain text body".to_string()));
    }
}

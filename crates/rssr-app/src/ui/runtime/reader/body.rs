//! Reader 命令的正文选择规则，只接收和返回普通 Rust 值。
//!
//! 这里只决定显示哪一份正文，解析、地址归一化与清洗复用 `rssr_infra::html`。
//! 结果仍是当前渲染所需的 HTML 或文本；页面状态与宿主操作由调用方装配。

use rssr_infra::html::{looks_like_html_fragment, sanitize_reader_html};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ReaderBody {
    Html(String),
    Text(String),
}

/// 按优先级挑选可显示的正文：完整 HTML > 看起来是 HTML 的文本字段 > 摘要 > 纯文本兜底。
///
/// 不少 feed 只在 `summary` 里放完整 HTML，所以文本字段也要过一遍 HTML 判定，否则会把标签
/// 当字面量显示出来。
pub(super) fn select_reader_body(
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

    #[test]
    fn reader_preserves_candidate_priority_and_missing_vs_empty_fields() {
        let cases = [
            (None, None, None, ReaderBody::Text("暂无正文".into())),
            (None, None, Some("summary"), ReaderBody::Text("summary".into())),
            (None, Some("text"), Some("summary"), ReaderBody::Text("text".into())),
            (None, Some(""), Some("summary"), ReaderBody::Text(String::new())),
            (None, Some("  \n"), Some("summary"), ReaderBody::Text("  \n".into())),
            (None, None, Some(""), ReaderBody::Text(String::new())),
            (Some(" \n"), None, None, ReaderBody::Text("暂无正文".into())),
            (Some("<p></p>"), Some("text"), None, ReaderBody::Html("<p></p>".into())),
            (None, Some("text"), Some("<p>summary</p>"), ReaderBody::Html("<p>summary</p>".into())),
            (None, Some(""), Some("<p>summary</p>"), ReaderBody::Html("<p>summary</p>".into())),
            (
                None,
                Some("<p>text</p>"),
                Some("<p>summary</p>"),
                ReaderBody::Html("<p>text</p>".into()),
            ),
            (
                Some("<script>hidden</script>"),
                Some("text"),
                Some("<p>summary</p>"),
                ReaderBody::Html("<p>summary</p>".into()),
            ),
            (
                None,
                Some("<script><p>hidden</p></script>"),
                Some("<p>summary</p>"),
                ReaderBody::Html("<p>summary</p>".into()),
            ),
            // An unsuccessful HTML attempt does not rewrite the eventual plain-text fallback.
            (
                None,
                Some("<script><p>hidden</p></script>"),
                Some("summary"),
                ReaderBody::Text("<script><p>hidden</p></script>".into()),
            ),
            // Preserve the existing HTML heuristic instead of interpreting every text field as HTML.
            (
                None,
                Some("text <p>literal</p>"),
                None,
                ReaderBody::Text("text <p>literal</p>".into()),
            ),
        ];
        for (html, text, summary, expected) in cases {
            assert_eq!(
                select_reader_body(
                    html.map(str::to_owned),
                    text.map(str::to_owned),
                    summary.map(str::to_owned),
                    None
                ),
                expected,
                "html={html:?}, text={text:?}, summary={summary:?}",
            );
        }
    }

    #[test]
    fn reader_prepares_images_and_sanitizes_whichever_html_field_wins() {
        let raw = concat!(
            r#"<p><a href="../next" onclick="bad()">Next</a>"#,
            r#"<img data-src="/media/p.png" alt="Pixel"></p><script>bad()</script>"#,
        );
        let base = url::Url::parse("https://example.test/articles/current").unwrap();
        for (html, text, summary) in
            [(Some(raw), None, None), (None, Some(raw), None), (None, None, Some(raw))]
        {
            let ReaderBody::Html(body) = select_reader_body(
                html.map(str::to_owned),
                text.map(str::to_owned),
                summary.map(str::to_owned),
                Some(&base),
            ) else {
                panic!("expected prepared HTML")
            };
            // Image URLs are normalized by infra; ordinary links keep their existing form.
            assert!(body.contains(r#"href="../next""#), "{body}");
            assert!(body.contains(r#"src="https://example.test/media/p.png""#), "{body}");
            assert!(body.contains(r#"alt="Pixel""#), "{body}");
            assert!(!body.contains("onclick") && !body.contains("<script"), "{body}");
        }
    }

    #[test]
    fn reader_recognizes_mathml_in_text_and_summary() {
        let raw = "<math><mfrac><mi>a</mi><mi>b</mi></mfrac></math>";
        for (text, summary) in [(Some(raw), None), (None, Some(raw))] {
            assert_eq!(
                select_reader_body(None, text.map(str::to_owned), summary.map(str::to_owned), None),
                ReaderBody::Html(raw.into()),
            );
        }
    }

    #[test]
    fn reader_keeps_budget_downgrade_instead_of_falling_back_to_another_field() {
        let raw = format!("{}a &lt;b&gt; c{}", "<div>".repeat(10_000), "</div>".repeat(10_000));
        assert_eq!(
            select_reader_body(
                Some(raw),
                Some("fallback text".into()),
                Some("<p>summary</p>".into()),
                None
            ),
            ReaderBody::Html("<p>a &lt;b&gt; c</p>".into()),
        );
    }
}

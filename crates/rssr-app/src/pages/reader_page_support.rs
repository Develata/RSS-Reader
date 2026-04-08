use time::{OffsetDateTime, UtcOffset, macros::format_description};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReaderBody {
    Html(String),
    Text(String),
}

pub(crate) fn select_reader_body(
    content_html: Option<String>,
    content_text: Option<String>,
    summary: Option<String>,
) -> ReaderBody {
    if let Some(html) = content_html.as_deref().and_then(sanitize_remote_html) {
        return ReaderBody::Html(html);
    }

    ReaderBody::Text(content_text.or(summary).unwrap_or_else(|| "暂无正文".to_string()))
}

pub(crate) fn sanitize_remote_html(raw: &str) -> Option<String> {
    let sanitized = ammonia::clean(raw);
    let trimmed = sanitized.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
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

    use super::{ReaderBody, format_reader_datetime_utc, select_reader_body};

    #[test]
    fn reader_prefers_full_html_over_summary_text() {
        let body = select_reader_body(
            Some("<article><p>Full body</p></article>".to_string()),
            Some("Summary teaser".to_string()),
            Some("Summary teaser".to_string()),
        );

        assert_eq!(body, ReaderBody::Html("<article><p>Full body</p></article>".to_string()));
    }

    #[test]
    fn reader_sanitizes_remote_html() {
        let body = select_reader_body(
            Some(r#"<p onclick="alert(1)">Hello</p><script>alert(2)</script>"#.to_string()),
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
}

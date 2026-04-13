use rssr_domain::FeedSummary;
use time::{OffsetDateTime, UtcOffset, macros::format_description};

pub(super) fn feed_refresh_status_text(feed: &FeedSummary) -> String {
    match (feed.last_success_at, feed.last_fetched_at) {
        (Some(last_success_at), Some(last_fetched_at)) if last_fetched_at > last_success_at => {
            format!(
                "最近尝试：{} · 最近成功：{}",
                format_feed_datetime_utc(Some(last_fetched_at))
                    .unwrap_or_else(|| "未知".to_string()),
                format_feed_datetime_utc(Some(last_success_at))
                    .unwrap_or_else(|| "未知".to_string())
            )
        }
        (Some(last_success_at), _) => format!(
            "最近成功：{}",
            format_feed_datetime_utc(Some(last_success_at)).unwrap_or_else(|| "未知".to_string())
        ),
        (None, Some(last_fetched_at)) => format!(
            "最近尝试：{}",
            format_feed_datetime_utc(Some(last_fetched_at)).unwrap_or_else(|| "未知".to_string())
        ),
        (None, None) => "尚未刷新".to_string(),
    }
}

pub(super) fn feed_refresh_state_attr(feed: &FeedSummary) -> &'static str {
    if feed.fetch_error.is_some() {
        "failed"
    } else {
        match (feed.last_success_at, feed.last_fetched_at) {
            (Some(last_success_at), Some(last_fetched_at)) if last_fetched_at > last_success_at => {
                "attempted"
            }
            (Some(_), _) => "success",
            (None, Some(_)) => "attempted",
            (None, None) => "never",
        }
    }
}

fn format_feed_datetime_utc(value: Option<OffsetDateTime>) -> Option<String> {
    const FEED_DATE_TIME_FORMAT: &[time::format_description::FormatItem<'static>] =
        format_description!("[year]-[month]-[day] [hour]:[minute] UTC");

    value.and_then(|timestamp| {
        timestamp.to_offset(UtcOffset::UTC).format(FEED_DATE_TIME_FORMAT).ok()
    })
}

#[cfg(test)]
mod tests {
    use super::feed_refresh_state_attr;
    use rssr_domain::FeedSummary;
    use time::OffsetDateTime;

    fn summary() -> FeedSummary {
        FeedSummary {
            id: 1,
            title: "Feed".to_string(),
            url: "https://example.com/feed.xml".to_string(),
            entry_count: 0,
            unread_count: 0,
            last_fetched_at: None,
            last_success_at: None,
            fetch_error: None,
        }
    }

    #[test]
    fn feed_refresh_state_is_never_before_any_attempt() {
        assert_eq!(feed_refresh_state_attr(&summary()), "never");
    }

    #[test]
    fn feed_refresh_state_is_success_after_successful_refresh() {
        let mut feed = summary();
        let now = OffsetDateTime::now_utc();
        feed.last_fetched_at = Some(now);
        feed.last_success_at = Some(now);

        assert_eq!(feed_refresh_state_attr(&feed), "success");
    }

    #[test]
    fn feed_refresh_state_is_failed_when_fetch_error_exists() {
        let mut feed = summary();
        let now = OffsetDateTime::now_utc();
        feed.last_fetched_at = Some(now);
        feed.fetch_error = Some("network timeout".to_string());

        assert_eq!(feed_refresh_state_attr(&feed), "failed");
    }
}

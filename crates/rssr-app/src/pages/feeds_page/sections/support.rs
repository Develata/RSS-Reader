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

fn format_feed_datetime_utc(value: Option<OffsetDateTime>) -> Option<String> {
    const FEED_DATE_TIME_FORMAT: &[time::format_description::FormatItem<'static>] =
        format_description!("[year]-[month]-[day] [hour]:[minute] UTC");

    value.and_then(|timestamp| {
        timestamp.to_offset(UtcOffset::UTC).format(FEED_DATE_TIME_FORMAT).ok()
    })
}

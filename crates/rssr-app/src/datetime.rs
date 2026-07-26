//! 界面上显示时间的唯一格式化入口。
//!
//! 这两种格式此前在四个地方各写了一遍——文章卡片、分组树、阅读页、订阅页——格式串逐字相同，
//! 只是函数名不同。合并到一处不改变任何输出，测试把两种格式的结果钉死，防止后续在某一处
//! 悄悄改掉格式后另外几处不同步。
//!
//! 全部按 UTC 呈现，与各页面原有行为一致。

use time::{OffsetDateTime, UtcOffset, format_description::FormatItem, macros::format_description};

const DATE_FORMAT: &[FormatItem<'static>] = format_description!("[year]-[month]-[day]");
const DATETIME_FORMAT: &[FormatItem<'static>] =
    format_description!("[year]-[month]-[day] [hour]:[minute] UTC");

/// `2026-03-29`：文章卡片上的日期，也是时间分组树的日期分桶键。
pub(crate) fn format_date_utc(value: Option<OffsetDateTime>) -> Option<String> {
    format_in_utc(value, DATE_FORMAT)
}

/// `2026-03-29 11:45 UTC`：阅读页的发布时间与订阅页的刷新时间。
pub(crate) fn format_datetime_utc(value: Option<OffsetDateTime>) -> Option<String> {
    format_in_utc(value, DATETIME_FORMAT)
}

fn format_in_utc(value: Option<OffsetDateTime>, format: &[FormatItem<'static>]) -> Option<String> {
    value.and_then(|timestamp| timestamp.to_offset(UtcOffset::UTC).format(format).ok())
}

#[cfg(test)]
mod tests {
    use super::{format_date_utc, format_datetime_utc};
    use time::{OffsetDateTime, format_description::well_known::Rfc3339};

    fn at(raw: &str) -> Option<OffsetDateTime> {
        Some(OffsetDateTime::parse(raw, &Rfc3339).expect("parse rfc3339"))
    }

    /// 合并前四处用的就是这两个格式串，输出必须逐字节不变。
    #[test]
    fn formats_stay_byte_identical_to_the_previous_per_page_helpers() {
        assert_eq!(format_date_utc(at("2026-03-29T19:45:33+08:00")).as_deref(), Some("2026-03-29"));
        assert_eq!(
            format_datetime_utc(at("2026-03-29T19:45:33+08:00")).as_deref(),
            Some("2026-03-29 11:45 UTC")
        );
    }

    /// 归一化到 UTC 会改变日期本身，不只是时分。分组树用日期做分桶键，
    /// 少了这一步同一篇文章可能落进相邻的另一个日期分组。
    #[test]
    fn dates_are_taken_after_converting_to_utc_not_before() {
        assert_eq!(format_date_utc(at("2026-03-30T02:30:00+08:00")).as_deref(), Some("2026-03-29"));
        assert_eq!(
            format_datetime_utc(at("2026-03-30T02:30:00+08:00")).as_deref(),
            Some("2026-03-29 18:30 UTC")
        );
    }

    #[test]
    fn missing_timestamps_render_nothing() {
        assert_eq!(format_date_utc(None), None);
        assert_eq!(format_datetime_utc(None), None);
    }
}

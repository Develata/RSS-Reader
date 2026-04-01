use time::{Date, Month, OffsetDateTime, PrimitiveDateTime, UtcOffset};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub id: i64,
    pub feed_id: i64,
    pub external_id: String,
    pub dedup_key: String,
    pub url: Option<Url>,
    pub title: String,
    pub author: Option<String>,
    pub summary: Option<String>,
    pub content_html: Option<String>,
    pub content_text: Option<String>,
    pub published_at: Option<OffsetDateTime>,
    pub updated_at_source: Option<OffsetDateTime>,
    pub first_seen_at: OffsetDateTime,
    pub content_hash: Option<String>,
    pub is_read: bool,
    pub is_starred: bool,
    pub read_at: Option<OffsetDateTime>,
    pub starred_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntrySummary {
    pub id: i64,
    pub feed_id: i64,
    pub title: String,
    pub feed_title: String,
    pub published_at: Option<OffsetDateTime>,
    pub is_read: bool,
    pub is_starred: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EntryQuery {
    pub feed_id: Option<i64>,
    pub unread_only: bool,
    pub starred_only: bool,
    pub search_title: Option<String>,
    pub limit: Option<u32>,
}

pub fn is_entry_archived(
    published_at: Option<OffsetDateTime>,
    archive_after_months: u32,
    now: OffsetDateTime,
) -> bool {
    let Some(published_at) = published_at else {
        return false;
    };
    if archive_after_months == 0 {
        return false;
    }

    published_at.to_offset(UtcOffset::UTC) < archive_cutoff(now, archive_after_months)
}

fn archive_cutoff(now: OffsetDateTime, archive_after_months: u32) -> OffsetDateTime {
    let now = now.to_offset(UtcOffset::UTC);
    let total_month_index =
        now.year() * 12 + (month_to_number(now.month()) as i32 - 1) - archive_after_months as i32;
    let cutoff_year = total_month_index.div_euclid(12);
    let cutoff_month = (total_month_index.rem_euclid(12) + 1) as u8;
    let cutoff_month = month_from_number(cutoff_month);
    let cutoff_day = now.day().min(last_day_of_month(cutoff_year, cutoff_month));
    let cutoff_date =
        Date::from_calendar_date(cutoff_year, cutoff_month, cutoff_day).expect("valid cutoff");

    PrimitiveDateTime::new(cutoff_date, now.time()).assume_utc()
}

fn last_day_of_month(year: i32, month: Month) -> u8 {
    for day in (28..=31).rev() {
        if Date::from_calendar_date(year, month, day).is_ok() {
            return day;
        }
    }
    28
}

fn month_to_number(month: Month) -> u8 {
    match month {
        Month::January => 1,
        Month::February => 2,
        Month::March => 3,
        Month::April => 4,
        Month::May => 5,
        Month::June => 6,
        Month::July => 7,
        Month::August => 8,
        Month::September => 9,
        Month::October => 10,
        Month::November => 11,
        Month::December => 12,
    }
}

fn month_from_number(month: u8) -> Month {
    match month {
        1 => Month::January,
        2 => Month::February,
        3 => Month::March,
        4 => Month::April,
        5 => Month::May,
        6 => Month::June,
        7 => Month::July,
        8 => Month::August,
        9 => Month::September,
        10 => Month::October,
        11 => Month::November,
        12 => Month::December,
        _ => unreachable!("month must be within 1..=12"),
    }
}

#[cfg(test)]
mod tests {
    use super::is_entry_archived;
    use time::{OffsetDateTime, format_description::well_known::Rfc3339};

    #[test]
    fn marks_entries_older_than_archive_cutoff_as_archived() {
        let now = OffsetDateTime::parse("2026-04-01T10:00:00Z", &Rfc3339).expect("parse now");
        let old_entry =
            OffsetDateTime::parse("2025-12-31T09:59:59Z", &Rfc3339).expect("parse old entry");
        let recent_entry =
            OffsetDateTime::parse("2026-01-01T10:00:00Z", &Rfc3339).expect("parse recent entry");

        assert!(is_entry_archived(Some(old_entry), 3, now));
        assert!(!is_entry_archived(Some(recent_entry), 3, now));
        assert!(!is_entry_archived(None, 3, now));
    }

    #[test]
    fn archive_cutoff_clamps_to_valid_day_in_shorter_months() {
        let now = OffsetDateTime::parse("2026-05-31T12:00:00Z", &Rfc3339).expect("parse now");
        let february_entry =
            OffsetDateTime::parse("2026-02-28T11:59:59Z", &Rfc3339).expect("parse february entry");
        let boundary_entry =
            OffsetDateTime::parse("2026-02-28T12:00:00Z", &Rfc3339).expect("parse boundary");

        assert!(is_entry_archived(Some(february_entry), 3, now));
        assert!(!is_entry_archived(Some(boundary_entry), 3, now));
    }
}

//! UI display only. List timestamps are localized once when loading the UI snapshot;
//! cards and all grouping levels consume that same value. Storage retains its original instant.
use time::{OffsetDateTime, UtcOffset, macros::format_description};

pub(crate) fn local_timestamp(value: OffsetDateTime) -> OffsetDateTime {
    crate::bootstrap::local_time::at(value)
}

/// Expects an already localized list timestamp; also used by date bucket labels.
pub(crate) fn format_date(value: Option<OffsetDateTime>) -> Option<String> {
    value.and_then(|at| at.format(format_description!("[year]-[month]-[day]")).ok())
}

pub(crate) fn format_datetime(value: Option<OffsetDateTime>) -> Option<String> {
    value.and_then(|at| format_full(local_timestamp(at)))
}

fn format_full(at: OffsetDateTime) -> Option<String> {
    let date = at.format(format_description!("[year]-[month]-[day] [hour]:[minute]")).ok()?;
    let offset = at.offset();
    if offset == UtcOffset::UTC {
        return Some(format!("{date} UTC"));
    }
    let seconds = offset.whole_seconds();
    let magnitude = seconds.unsigned_abs();
    let mut suffix = format!(
        "UTC{}{:02}:{:02}",
        if seconds < 0 { '-' } else { '+' },
        magnitude / 3600,
        magnitude / 60 % 60
    );
    if !magnitude.is_multiple_of(60) {
        suffix.push_str(&format!(":{:02}", magnitude % 60));
    }
    Some(format!("{date} {suffix}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::format_description::well_known::Rfc3339;

    #[test]
    fn offsets_cross_days_and_preserve_fractional_hours() {
        let instant = OffsetDateTime::parse("2026-09-25T01:30:00Z", &Rfc3339).unwrap();
        for (seconds, date, full) in [
            (-10800, "2026-09-24", "2026-09-24 22:30 UTC-03:00"),
            (19800, "2026-09-25", "2026-09-25 07:00 UTC+05:30"),
            (45900, "2026-09-25", "2026-09-25 14:15 UTC+12:45"),
            (0, "2026-09-25", "2026-09-25 01:30 UTC"),
        ] {
            let local = instant.to_offset(UtcOffset::from_whole_seconds(seconds).unwrap());
            assert_eq!(format_date(Some(local)).as_deref(), Some(date));
            assert_eq!(format_full(local).as_deref(), Some(full));
        }
    }

    #[test]
    fn missing_timestamp_has_no_label() {
        assert_eq!(format_date(None), None);
        assert_eq!(format_datetime(None), None);
    }
}

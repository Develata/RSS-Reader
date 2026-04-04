use std::collections::BTreeMap;

use rssr_domain::EntrySummary;
use time::{OffsetDateTime, UtcOffset};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryMonthGroup {
    pub(crate) anchor_id: String,
    pub(crate) title: String,
    pub(crate) subtitle: String,
    pub(crate) dates: Vec<EntryDateGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntrySourceGroup {
    pub(crate) anchor_id: String,
    pub(crate) title: String,
    pub(crate) subtitle: String,
    pub(crate) months: Vec<EntrySourceMonthGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntrySourceMonthGroup {
    pub(crate) anchor_id: String,
    pub(crate) title: String,
    pub(crate) subtitle: String,
    pub(crate) entries: Vec<EntrySummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryDateGroup {
    pub(crate) anchor_id: String,
    pub(crate) title: String,
    pub(crate) subtitle: String,
    pub(crate) sources: Vec<EntryDateSourceGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryDateSourceGroup {
    pub(crate) anchor_id: String,
    pub(crate) title: String,
    pub(crate) subtitle: String,
    pub(crate) entries: Vec<EntrySummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryGroupNavItem {
    pub(crate) anchor_id: String,
    pub(crate) title: String,
    pub(crate) subtitle: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryDirectoryMonth {
    pub(crate) anchor_id: String,
    pub(crate) title: String,
    pub(crate) subtitle: String,
    pub(crate) dates: Vec<EntryDirectoryDate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryDirectorySource {
    pub(crate) anchor_id: String,
    pub(crate) title: String,
    pub(crate) subtitle: String,
    pub(crate) months: Vec<EntryDirectoryMonth>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryDirectoryDate {
    pub(crate) anchor_id: String,
    pub(crate) title: String,
    pub(crate) subtitle: String,
}

pub(crate) fn group_entries_by_time_tree(entries: &[EntrySummary]) -> Vec<EntryMonthGroup> {
    let mut groups: BTreeMap<(i32, u8), Vec<EntrySummary>> = BTreeMap::new();
    let mut undated_entries = Vec::new();

    for entry in entries {
        if let Some(published_at) = entry.published_at {
            let published_at = published_at.to_offset(UtcOffset::UTC);
            groups
                .entry((published_at.year(), published_at.month() as u8))
                .or_default()
                .push(entry.clone());
        } else {
            undated_entries.push(entry.clone());
        }
    }

    let mut grouped = groups
        .into_iter()
        .rev()
        .map(|((year, month), mut items)| {
            items.sort_by_key(|entry| {
                std::cmp::Reverse(entry.published_at.unwrap_or(OffsetDateTime::UNIX_EPOCH))
            });
            let title = format!("{year} 年 {month:02} 月");
            EntryMonthGroup {
                anchor_id: group_anchor_id(&title),
                title,
                subtitle: format!("{} 篇文章", items.len()),
                dates: group_date_buckets(&items),
            }
        })
        .collect::<Vec<_>>();

    if !undated_entries.is_empty() {
        undated_entries.sort_by_key(|entry| {
            std::cmp::Reverse(entry.published_at.unwrap_or(OffsetDateTime::UNIX_EPOCH))
        });
        let title = "未标注日期".to_string();
        grouped.push(EntryMonthGroup {
            anchor_id: group_anchor_id(&title),
            title,
            subtitle: format!("{} 篇文章", undated_entries.len()),
            dates: group_date_buckets(&undated_entries),
        });
    }

    grouped
}

pub(crate) fn group_entries_by_source_tree(entries: &[EntrySummary]) -> Vec<EntrySourceGroup> {
    let mut groups: BTreeMap<String, Vec<EntrySummary>> = BTreeMap::new();
    let mut latest_seen: BTreeMap<String, Option<OffsetDateTime>> = BTreeMap::new();

    for entry in entries {
        groups.entry(entry.feed_title.clone()).or_default().push(entry.clone());
        let latest = latest_seen.entry(entry.feed_title.clone()).or_insert(None);
        if latest.is_none() || entry.published_at > *latest {
            *latest = entry.published_at;
        }
    }

    let mut grouped = groups
        .into_iter()
        .map(|(feed_title, mut items)| {
            items.sort_by_key(|entry| {
                std::cmp::Reverse(entry.published_at.unwrap_or(OffsetDateTime::UNIX_EPOCH))
            });
            let latest = latest_seen.get(&feed_title).and_then(|value| *value);
            (
                latest,
                EntrySourceGroup {
                    anchor_id: group_anchor_id(&feed_title),
                    title: feed_title,
                    subtitle: format!("{} 篇文章", items.len()),
                    months: group_source_months(&items),
                },
            )
        })
        .collect::<Vec<_>>();

    grouped.sort_by(|(left_latest, left_group), (right_latest, right_group)| {
        right_latest.cmp(left_latest).then_with(|| left_group.title.cmp(&right_group.title))
    });

    grouped.into_iter().map(|(_, group)| group).collect()
}

fn group_date_buckets(entries: &[EntrySummary]) -> Vec<EntryDateGroup> {
    let mut groups: BTreeMap<String, Vec<EntrySummary>> = BTreeMap::new();

    for entry in entries {
        let key =
            format_entry_date_utc(entry.published_at).unwrap_or_else(|| "未标注日期".to_string());
        groups.entry(key).or_default().push(entry.clone());
    }

    groups
        .into_iter()
        .rev()
        .map(|(date, mut items)| {
            items.sort_by_key(|entry| {
                std::cmp::Reverse(entry.published_at.unwrap_or(OffsetDateTime::UNIX_EPOCH))
            });
            let anchor_id = group_anchor_id(&format!("{}-{}", date, items[0].id));
            EntryDateGroup {
                anchor_id,
                title: date,
                subtitle: format!("{} 篇文章", items.len()),
                sources: group_date_sources(&items),
            }
        })
        .collect()
}

fn group_date_sources(entries: &[EntrySummary]) -> Vec<EntryDateSourceGroup> {
    let mut groups: BTreeMap<String, Vec<EntrySummary>> = BTreeMap::new();

    for entry in entries {
        groups.entry(entry.feed_title.clone()).or_default().push(entry.clone());
    }

    groups
        .into_iter()
        .map(|(feed_title, mut items)| {
            items.sort_by_key(|entry| {
                std::cmp::Reverse(entry.published_at.unwrap_or(OffsetDateTime::UNIX_EPOCH))
            });
            let anchor_id = group_anchor_id(&format!("{}-{}", feed_title, items[0].id));
            EntryDateSourceGroup {
                anchor_id,
                title: feed_title,
                subtitle: format!("{} 篇文章", items.len()),
                entries: items,
            }
        })
        .collect()
}

fn group_source_months(entries: &[EntrySummary]) -> Vec<EntrySourceMonthGroup> {
    let mut groups: BTreeMap<(i32, u8), Vec<EntrySummary>> = BTreeMap::new();
    let mut undated_entries = Vec::new();

    for entry in entries {
        if let Some(published_at) = entry.published_at {
            let published_at = published_at.to_offset(UtcOffset::UTC);
            groups
                .entry((published_at.year(), published_at.month() as u8))
                .or_default()
                .push(entry.clone());
        } else {
            undated_entries.push(entry.clone());
        }
    }

    let mut months = groups
        .into_iter()
        .rev()
        .map(|((year, month), mut items)| {
            items.sort_by_key(|entry| {
                std::cmp::Reverse(entry.published_at.unwrap_or(OffsetDateTime::UNIX_EPOCH))
            });
            let title = format!("{year} 年 {month:02} 月");
            let anchor_id = group_anchor_id(&format!("{}-{}", title, items[0].id));
            EntrySourceMonthGroup {
                anchor_id,
                title,
                subtitle: format!("{} 篇文章", items.len()),
                entries: items,
            }
        })
        .collect::<Vec<_>>();

    if !undated_entries.is_empty() {
        undated_entries.sort_by_key(|entry| {
            std::cmp::Reverse(entry.published_at.unwrap_or(OffsetDateTime::UNIX_EPOCH))
        });
        let title = "未标注日期".to_string();
        let anchor_id = group_anchor_id(&format!("{}-{}", title, undated_entries[0].id));
        months.push(EntrySourceMonthGroup {
            anchor_id,
            title,
            subtitle: format!("{} 篇文章", undated_entries.len()),
            entries: undated_entries,
        });
    }

    months
}

pub(crate) fn build_directory_months(groups: &[EntryMonthGroup]) -> Vec<EntryDirectoryMonth> {
    groups
        .iter()
        .map(|month| EntryDirectoryMonth {
            anchor_id: month.anchor_id.clone(),
            title: month.title.clone(),
            subtitle: month.subtitle.clone(),
            dates: month
                .dates
                .iter()
                .map(|date| EntryDirectoryDate {
                    anchor_id: date.anchor_id.clone(),
                    title: date.title.clone(),
                    subtitle: date.subtitle.clone(),
                })
                .collect(),
        })
        .collect()
}

pub(crate) fn build_month_nav_items(groups: &[EntryMonthGroup]) -> Vec<EntryGroupNavItem> {
    groups
        .iter()
        .map(|group| EntryGroupNavItem {
            anchor_id: group.anchor_id.clone(),
            title: group.title.clone(),
            subtitle: group.subtitle.clone(),
        })
        .collect()
}

pub(crate) fn build_directory_sources(groups: &[EntrySourceGroup]) -> Vec<EntryDirectorySource> {
    groups
        .iter()
        .map(|group| EntryDirectorySource {
            anchor_id: group.anchor_id.clone(),
            title: group.title.clone(),
            subtitle: group.subtitle.clone(),
            months: group
                .months
                .iter()
                .map(|month| EntryDirectoryMonth {
                    anchor_id: month.anchor_id.clone(),
                    title: month.title.clone(),
                    subtitle: month.subtitle.clone(),
                    dates: Vec::new(),
                })
                .collect(),
        })
        .collect()
}

pub(crate) fn build_group_nav_items(groups: &[EntrySourceGroup]) -> Vec<EntryGroupNavItem> {
    groups
        .iter()
        .map(|group| EntryGroupNavItem {
            anchor_id: group.anchor_id.clone(),
            title: group.title.clone(),
            subtitle: group.subtitle.clone(),
        })
        .collect()
}

pub(crate) fn group_anchor_id(title: &str) -> String {
    let slug = title
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else if ch.is_whitespace() || matches!(ch, '-' | '_' | '/' | '.') {
                '-'
            } else {
                ch
            }
        })
        .collect::<String>();
    format!("entry-group-{}", slug.trim_matches('-'))
}

fn format_entry_date_utc(published_at: Option<OffsetDateTime>) -> Option<String> {
    const ENTRY_DATE_FORMAT: &[time::format_description::FormatItem<'static>] =
        time::macros::format_description!("[year]-[month]-[day]");

    published_at.and_then(|value| value.to_offset(UtcOffset::UTC).format(ENTRY_DATE_FORMAT).ok())
}

#[cfg(test)]
mod tests {
    use super::{group_entries_by_source_tree, group_entries_by_time_tree};
    use rssr_domain::EntrySummary;
    use time::{OffsetDateTime, format_description::well_known::Rfc3339};

    fn entry(id: i64, feed_title: &str, title: &str, published_at: Option<&str>) -> EntrySummary {
        EntrySummary {
            id,
            feed_id: id,
            title: title.to_string(),
            feed_title: feed_title.to_string(),
            published_at: published_at
                .map(|value| OffsetDateTime::parse(value, &Rfc3339).expect("parse published_at")),
            is_read: false,
            is_starred: false,
        }
    }

    #[test]
    fn groups_entries_by_time_in_descending_month_order() {
        let entries = vec![
            entry(1, "Alpha", "March one", Some("2026-03-21T08:00:00Z")),
            entry(2, "Beta", "April one", Some("2026-04-02T08:00:00Z")),
            entry(4, "Beta", "April two", Some("2026-04-02T09:00:00Z")),
            entry(3, "Beta", "No date", None),
        ];

        let groups = group_entries_by_time_tree(&entries);

        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].title, "2026 年 04 月");
        assert_eq!(groups[0].dates[0].title, "2026-04-02");
        assert_eq!(groups[0].dates[0].sources[0].title, "Beta");
        assert_eq!(groups[0].dates[0].sources[0].entries[0].title, "April two");
        assert_eq!(groups[1].title, "2026 年 03 月");
        assert_eq!(groups[2].title, "未标注日期");
    }

    #[test]
    fn groups_entries_by_source_using_latest_entry_order() {
        let entries = vec![
            entry(1, "Alpha", "Older alpha", Some("2026-03-21T08:00:00Z")),
            entry(2, "Beta", "Newest beta", Some("2026-04-02T08:00:00Z")),
            entry(3, "Alpha", "Newest alpha", Some("2026-04-01T08:00:00Z")),
        ];

        let groups = group_entries_by_source_tree(&entries);

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].title, "Beta");
        assert_eq!(groups[1].title, "Alpha");
        assert_eq!(groups[1].months[0].entries[0].title, "Newest alpha");
    }
}

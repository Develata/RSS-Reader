use std::{collections::BTreeMap, sync::Arc};

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
    pub(crate) entries: Vec<Arc<EntrySummary>>,
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
    pub(crate) entries: Vec<Arc<EntrySummary>>,
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

pub(crate) fn group_entries_by_time_tree(entries: &[Arc<EntrySummary>]) -> Vec<EntryMonthGroup> {
    let mut groups: BTreeMap<(i32, u8), Vec<Arc<EntrySummary>>> = BTreeMap::new();
    let mut undated_entries = Vec::new();

    for entry in entries {
        if let Some(published_at) = entry.published_at {
            let published_at = published_at.to_offset(UtcOffset::UTC);
            groups
                .entry((published_at.year(), published_at.month() as u8))
                .or_default()
                .push(Arc::clone(entry));
        } else {
            undated_entries.push(Arc::clone(entry));
        }
    }

    let mut grouped = groups
        .into_iter()
        .rev()
        .map(|((year, month), items)| {
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

pub(crate) fn group_entries_by_source_tree(entries: &[Arc<EntrySummary>]) -> Vec<EntrySourceGroup> {
    let mut groups: BTreeMap<String, Vec<Arc<EntrySummary>>> = BTreeMap::new();
    let mut latest_seen: BTreeMap<String, Option<OffsetDateTime>> = BTreeMap::new();

    for entry in entries {
        groups
            .entry(entry.feed_title.clone())
            .or_default()
            .push(Arc::clone(entry));
        let latest = latest_seen.entry(entry.feed_title.clone()).or_insert(None);
        if latest.is_none() || entry.published_at > *latest {
            *latest = entry.published_at;
        }
    }

    let mut grouped = groups
        .into_iter()
        .map(|(feed_title, items)| {
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

pub(crate) fn limit_time_groups(
    groups: &[EntryMonthGroup],
    mut remaining_entries: usize,
) -> Vec<EntryMonthGroup> {
    if remaining_entries == usize::MAX {
        return groups.to_vec();
    }

    let mut limited_groups = Vec::new();
    for month in groups {
        if remaining_entries == 0 {
            break;
        }
        let dates = limit_date_groups(&month.dates, &mut remaining_entries);
        if dates.is_empty() {
            continue;
        }
        limited_groups.push(EntryMonthGroup {
            anchor_id: month.anchor_id.clone(),
            title: month.title.clone(),
            subtitle: format!("{} 篇文章", count_month_entries(&dates)),
            dates,
        });
    }

    limited_groups
}

pub(crate) fn limit_source_groups(
    groups: &[EntrySourceGroup],
    mut remaining_entries: usize,
) -> Vec<EntrySourceGroup> {
    if remaining_entries == usize::MAX {
        return groups.to_vec();
    }

    let mut limited_groups = Vec::new();
    for source in groups {
        if remaining_entries == 0 {
            break;
        }
        let months = limit_source_month_groups(&source.months, &mut remaining_entries);
        if months.is_empty() {
            continue;
        }
        limited_groups.push(EntrySourceGroup {
            anchor_id: source.anchor_id.clone(),
            title: source.title.clone(),
            subtitle: format!("{} 篇文章", count_source_group_entries(&months)),
            months,
        });
    }

    limited_groups
}

fn limit_date_groups(
    groups: &[EntryDateGroup],
    remaining_entries: &mut usize,
) -> Vec<EntryDateGroup> {
    let mut limited_groups = Vec::new();
    for date in groups {
        if *remaining_entries == 0 {
            break;
        }
        let sources = limit_date_source_groups(&date.sources, remaining_entries);
        if sources.is_empty() {
            continue;
        }
        limited_groups.push(EntryDateGroup {
            anchor_id: date.anchor_id.clone(),
            title: date.title.clone(),
            subtitle: format!("{} 篇文章", count_date_entries(&sources)),
            sources,
        });
    }
    limited_groups
}

fn limit_date_source_groups(
    groups: &[EntryDateSourceGroup],
    remaining_entries: &mut usize,
) -> Vec<EntryDateSourceGroup> {
    let mut limited_groups = Vec::new();
    for source in groups {
        if *remaining_entries == 0 {
            break;
        }
        let take_count = source.entries.len().min(*remaining_entries);
        if take_count == 0 {
            continue;
        }
        *remaining_entries -= take_count;
        limited_groups.push(EntryDateSourceGroup {
            anchor_id: source.anchor_id.clone(),
            title: source.title.clone(),
            subtitle: format!("{take_count} 篇文章"),
            entries: source.entries.iter().take(take_count).cloned().collect(),
        });
    }
    limited_groups
}

fn limit_source_month_groups(
    groups: &[EntrySourceMonthGroup],
    remaining_entries: &mut usize,
) -> Vec<EntrySourceMonthGroup> {
    let mut limited_groups = Vec::new();
    for month in groups {
        if *remaining_entries == 0 {
            break;
        }
        let take_count = month.entries.len().min(*remaining_entries);
        if take_count == 0 {
            continue;
        }
        *remaining_entries -= take_count;
        limited_groups.push(EntrySourceMonthGroup {
            anchor_id: month.anchor_id.clone(),
            title: month.title.clone(),
            subtitle: format!("{take_count} 篇文章"),
            entries: month.entries.iter().take(take_count).cloned().collect(),
        });
    }
    limited_groups
}

fn group_date_buckets(entries: &[Arc<EntrySummary>]) -> Vec<EntryDateGroup> {
    let mut groups: BTreeMap<String, Vec<Arc<EntrySummary>>> = BTreeMap::new();

    for entry in entries {
        let key =
            format_entry_date_utc(entry.published_at).unwrap_or_else(|| "未标注日期".to_string());
        groups.entry(key).or_default().push(Arc::clone(entry));
    }

    groups
        .into_iter()
        .rev()
        .map(|(date, items)| {
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

fn group_date_sources(entries: &[Arc<EntrySummary>]) -> Vec<EntryDateSourceGroup> {
    let mut groups: BTreeMap<String, Vec<Arc<EntrySummary>>> = BTreeMap::new();

    for entry in entries {
        groups
            .entry(entry.feed_title.clone())
            .or_default()
            .push(Arc::clone(entry));
    }

    groups
        .into_iter()
        .map(|(feed_title, items)| {
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

fn group_source_months(entries: &[Arc<EntrySummary>]) -> Vec<EntrySourceMonthGroup> {
    let mut groups: BTreeMap<(i32, u8), Vec<Arc<EntrySummary>>> = BTreeMap::new();
    let mut undated_entries = Vec::new();

    for entry in entries {
        if let Some(published_at) = entry.published_at {
            let published_at = published_at.to_offset(UtcOffset::UTC);
            groups
                .entry((published_at.year(), published_at.month() as u8))
                .or_default()
                .push(Arc::clone(entry));
        } else {
            undated_entries.push(Arc::clone(entry));
        }
    }

    let mut months = groups
        .into_iter()
        .rev()
        .map(|((year, month), items)| {
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

fn count_month_entries(groups: &[EntryDateGroup]) -> usize {
    groups.iter().map(|group| count_date_entries(&group.sources)).sum()
}

fn count_date_entries(groups: &[EntryDateSourceGroup]) -> usize {
    groups.iter().map(|group| group.entries.len()).sum()
}

fn count_source_group_entries(groups: &[EntrySourceMonthGroup]) -> usize {
    groups.iter().map(|group| group.entries.len()).sum()
}

fn format_entry_date_utc(published_at: Option<OffsetDateTime>) -> Option<String> {
    const ENTRY_DATE_FORMAT: &[time::format_description::FormatItem<'static>] =
        time::macros::format_description!("[year]-[month]-[day]");

    published_at.and_then(|value| value.to_offset(UtcOffset::UTC).format(ENTRY_DATE_FORMAT).ok())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{
        group_entries_by_source_tree, group_entries_by_time_tree, limit_source_groups,
        limit_time_groups,
    };
    use rssr_domain::EntrySummary;
    use time::{OffsetDateTime, format_description::well_known::Rfc3339};

    fn entry(id: i64, feed_title: &str, title: &str, published_at: Option<&str>) -> Arc<EntrySummary> {
        Arc::new(EntrySummary {
            id,
            feed_id: id,
            title: title.to_string(),
            feed_title: feed_title.to_string(),
            published_at: published_at
                .map(|value| OffsetDateTime::parse(value, &Rfc3339).expect("parse published_at")),
            is_read: false,
            is_starred: false,
        })
    }

    #[test]
    fn groups_entries_by_time_in_descending_month_order() {
        let entries = vec![
            entry(4, "Beta", "April two", Some("2026-04-02T09:00:00Z")),
            entry(2, "Beta", "April one", Some("2026-04-02T08:00:00Z")),
            entry(1, "Alpha", "March one", Some("2026-03-21T08:00:00Z")),
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

    #[test]
    fn limits_time_groups_by_total_entry_budget() {
        let entries = vec![
            entry(1, "Alpha", "April three", Some("2026-04-03T08:00:00Z")),
            entry(2, "Alpha", "April two", Some("2026-04-02T08:00:00Z")),
            entry(3, "Beta", "April one", Some("2026-04-01T08:00:00Z")),
        ];

        let groups = group_entries_by_time_tree(&entries);
        let limited = limit_time_groups(&groups, 2);
        let rendered_total = limited[0]
            .dates
            .iter()
            .flat_map(|date| date.sources.iter())
            .map(|source| source.entries.len())
            .sum::<usize>();

        assert_eq!(limited.len(), 1);
        assert_eq!(limited[0].subtitle, "2 篇文章");
        assert_eq!(rendered_total, 2);
    }

    #[test]
    fn limits_source_groups_by_total_entry_budget() {
        let entries = vec![
            entry(1, "Alpha", "Alpha three", Some("2026-04-03T08:00:00Z")),
            entry(2, "Alpha", "Alpha two", Some("2026-04-02T08:00:00Z")),
            entry(3, "Beta", "Beta one", Some("2026-04-01T08:00:00Z")),
        ];

        let groups = group_entries_by_source_tree(&entries);
        let limited = limit_source_groups(&groups, 2);

        assert_eq!(limited.len(), 1);
        assert_eq!(limited[0].title, "Alpha");
        assert_eq!(limited[0].months[0].entries.len(), 2);
    }
}

use std::{collections::BTreeMap, sync::Arc};

use rssr_domain::EntrySummary;
use time::{OffsetDateTime, UtcOffset};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryMonthGroup {
    pub(crate) anchor_id: String,
    pub(crate) title: String,
    pub(crate) subtitle: String,
    pub(crate) target_page: u32,
    pub(crate) dates: Vec<EntryDateGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntrySourceGroup {
    pub(crate) anchor_id: String,
    pub(crate) title: String,
    pub(crate) subtitle: String,
    pub(crate) target_page: u32,
    pub(crate) months: Vec<EntrySourceMonthGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntrySourceMonthGroup {
    pub(crate) anchor_id: String,
    pub(crate) title: String,
    pub(crate) subtitle: String,
    pub(crate) target_page: u32,
    pub(crate) entries: Vec<Arc<EntrySummary>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryDateGroup {
    pub(crate) anchor_id: String,
    pub(crate) title: String,
    pub(crate) subtitle: String,
    pub(crate) target_page: u32,
    pub(crate) sources: Vec<EntryDateSourceGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryDateSourceGroup {
    pub(crate) anchor_id: String,
    pub(crate) title: String,
    pub(crate) subtitle: String,
    pub(crate) target_page: u32,
    pub(crate) entries: Vec<Arc<EntrySummary>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryGroupNavItem {
    pub(crate) anchor_id: String,
    pub(crate) title: String,
    pub(crate) subtitle: String,
    pub(crate) target_page: u32,
    pub(crate) is_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryDirectoryMonth {
    pub(crate) anchor_id: String,
    pub(crate) title: String,
    pub(crate) subtitle: String,
    pub(crate) target_page: u32,
    pub(crate) is_active: bool,
    pub(crate) dates: Vec<EntryDirectoryDate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryDirectorySource {
    pub(crate) anchor_id: String,
    pub(crate) title: String,
    pub(crate) subtitle: String,
    pub(crate) target_page: u32,
    pub(crate) is_active: bool,
    pub(crate) months: Vec<EntryDirectoryMonth>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryDirectoryDate {
    pub(crate) anchor_id: String,
    pub(crate) title: String,
    pub(crate) subtitle: String,
    pub(crate) target_page: u32,
    pub(crate) is_active: bool,
}

pub(crate) fn group_entries_by_time_tree(
    entries: &[Arc<EntrySummary>],
    page_size: usize,
) -> Vec<EntryMonthGroup> {
    let mut groups: BTreeMap<(i32, u8), Vec<(usize, Arc<EntrySummary>)>> = BTreeMap::new();
    let mut undated_entries = Vec::new();

    for (index, entry) in entries.iter().enumerate() {
        if let Some(published_at) = entry.published_at {
            let published_at = published_at.to_offset(UtcOffset::UTC);
            groups
                .entry((published_at.year(), published_at.month() as u8))
                .or_default()
                .push((index, Arc::clone(entry)));
        } else {
            undated_entries.push((index, Arc::clone(entry)));
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
                target_page: page_for_index(items[0].0, page_size),
                dates: group_date_buckets(&items, page_size),
            }
        })
        .collect::<Vec<_>>();

    if !undated_entries.is_empty() {
        let title = "未标注日期".to_string();
        grouped.push(EntryMonthGroup {
            anchor_id: group_anchor_id(&title),
            title,
            subtitle: format!("{} 篇文章", undated_entries.len()),
            target_page: page_for_index(undated_entries[0].0, page_size),
            dates: group_date_buckets(&undated_entries, page_size),
        });
    }

    grouped
}

pub(crate) fn group_entries_by_source_tree(
    entries: &[Arc<EntrySummary>],
    page_size: usize,
) -> Vec<EntrySourceGroup> {
    let mut groups: BTreeMap<String, Vec<(usize, Arc<EntrySummary>)>> = BTreeMap::new();
    let mut latest_seen: BTreeMap<String, Option<OffsetDateTime>> = BTreeMap::new();

    for (index, entry) in entries.iter().enumerate() {
        groups
            .entry(entry.feed_title.clone())
            .or_default()
            .push((index, Arc::clone(entry)));
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
                    target_page: page_for_index(items[0].0, page_size),
                    months: group_source_months(&items, page_size),
                },
            )
        })
        .collect::<Vec<_>>();

    grouped.sort_by(|(left_latest, left_group), (right_latest, right_group)| {
        right_latest.cmp(left_latest).then_with(|| left_group.title.cmp(&right_group.title))
    });

    grouped.into_iter().map(|(_, group)| group).collect()
}

pub(crate) fn find_active_time_anchors(
    groups: &[EntryMonthGroup],
    current_entry_id: Option<i64>,
) -> (Option<String>, Option<String>) {
    let Some(current_entry_id) = current_entry_id else {
        return (None, None);
    };

    for month in groups {
        for date in &month.dates {
            for source in &date.sources {
                if source.entries.iter().any(|entry| entry.id == current_entry_id) {
                    return (Some(month.anchor_id.clone()), Some(date.anchor_id.clone()));
                }
            }
        }
    }

    (None, None)
}

pub(crate) fn find_active_source_anchors(
    groups: &[EntrySourceGroup],
    current_entry_id: Option<i64>,
) -> (Option<String>, Option<String>) {
    let Some(current_entry_id) = current_entry_id else {
        return (None, None);
    };

    for source in groups {
        for month in &source.months {
            if month.entries.iter().any(|entry| entry.id == current_entry_id) {
                return (Some(source.anchor_id.clone()), Some(month.anchor_id.clone()));
            }
        }
    }

    (None, None)
}

pub(crate) fn build_directory_months(
    groups: &[EntryMonthGroup],
    active_group_anchor: Option<&str>,
    active_directory_anchor: Option<&str>,
) -> Vec<EntryDirectoryMonth> {
    groups
        .iter()
        .map(|month| EntryDirectoryMonth {
            anchor_id: month.anchor_id.clone(),
            title: month.title.clone(),
            subtitle: month.subtitle.clone(),
            target_page: month.target_page,
            is_active: active_group_anchor == Some(month.anchor_id.as_str()),
            dates: month
                .dates
                .iter()
                .map(|date| EntryDirectoryDate {
                    anchor_id: date.anchor_id.clone(),
                    title: date.title.clone(),
                    subtitle: date.subtitle.clone(),
                    target_page: date.target_page,
                    is_active: active_directory_anchor == Some(date.anchor_id.as_str()),
                })
                .collect(),
        })
        .collect()
}

pub(crate) fn build_month_nav_items(
    groups: &[EntryMonthGroup],
    active_group_anchor: Option<&str>,
) -> Vec<EntryGroupNavItem> {
    groups
        .iter()
        .map(|group| EntryGroupNavItem {
            anchor_id: group.anchor_id.clone(),
            title: group.title.clone(),
            subtitle: group.subtitle.clone(),
            target_page: group.target_page,
            is_active: active_group_anchor == Some(group.anchor_id.as_str()),
        })
        .collect()
}

pub(crate) fn build_directory_sources(
    groups: &[EntrySourceGroup],
    active_group_anchor: Option<&str>,
    active_directory_anchor: Option<&str>,
) -> Vec<EntryDirectorySource> {
    groups
        .iter()
        .map(|group| EntryDirectorySource {
            anchor_id: group.anchor_id.clone(),
            title: group.title.clone(),
            subtitle: group.subtitle.clone(),
            target_page: group.target_page,
            is_active: active_group_anchor == Some(group.anchor_id.as_str()),
            months: group
                .months
                .iter()
                .map(|month| EntryDirectoryMonth {
                    anchor_id: month.anchor_id.clone(),
                    title: month.title.clone(),
                    subtitle: month.subtitle.clone(),
                    target_page: month.target_page,
                    is_active: active_directory_anchor == Some(month.anchor_id.as_str()),
                    dates: Vec::new(),
                })
                .collect(),
        })
        .collect()
}

pub(crate) fn build_group_nav_items(
    groups: &[EntrySourceGroup],
    active_group_anchor: Option<&str>,
) -> Vec<EntryGroupNavItem> {
    groups
        .iter()
        .map(|group| EntryGroupNavItem {
            anchor_id: group.anchor_id.clone(),
            title: group.title.clone(),
            subtitle: group.subtitle.clone(),
            target_page: group.target_page,
            is_active: active_group_anchor == Some(group.anchor_id.as_str()),
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

fn group_date_buckets(
    entries: &[(usize, Arc<EntrySummary>)],
    page_size: usize,
) -> Vec<EntryDateGroup> {
    let mut groups: BTreeMap<String, Vec<(usize, Arc<EntrySummary>)>> = BTreeMap::new();

    for (index, entry) in entries {
        let key =
            format_entry_date_utc(entry.published_at).unwrap_or_else(|| "未标注日期".to_string());
        groups.entry(key).or_default().push((*index, Arc::clone(entry)));
    }

    groups
        .into_iter()
        .rev()
        .map(|(date, items)| {
            let anchor_id = group_anchor_id(&format!("{}-{}", date, items[0].1.id));
            EntryDateGroup {
                anchor_id,
                title: date,
                subtitle: format!("{} 篇文章", items.len()),
                target_page: page_for_index(items[0].0, page_size),
                sources: group_date_sources(&items, page_size),
            }
        })
        .collect()
}

fn group_date_sources(
    entries: &[(usize, Arc<EntrySummary>)],
    page_size: usize,
) -> Vec<EntryDateSourceGroup> {
    let mut groups: BTreeMap<String, Vec<(usize, Arc<EntrySummary>)>> = BTreeMap::new();

    for (index, entry) in entries {
        groups
            .entry(entry.feed_title.clone())
            .or_default()
            .push((*index, Arc::clone(entry)));
    }

    groups
        .into_iter()
        .map(|(feed_title, items)| {
            let anchor_id = group_anchor_id(&format!("{}-{}", feed_title, items[0].1.id));
            EntryDateSourceGroup {
                anchor_id,
                title: feed_title,
                subtitle: format!("{} 篇文章", items.len()),
                target_page: page_for_index(items[0].0, page_size),
                entries: items.into_iter().map(|(_, entry)| entry).collect(),
            }
        })
        .collect()
}

fn group_source_months(
    entries: &[(usize, Arc<EntrySummary>)],
    page_size: usize,
) -> Vec<EntrySourceMonthGroup> {
    let mut groups: BTreeMap<(i32, u8), Vec<(usize, Arc<EntrySummary>)>> = BTreeMap::new();
    let mut undated_entries = Vec::new();

    for (index, entry) in entries {
        if let Some(published_at) = entry.published_at {
            let published_at = published_at.to_offset(UtcOffset::UTC);
            groups
                .entry((published_at.year(), published_at.month() as u8))
                .or_default()
                .push((*index, Arc::clone(entry)));
        } else {
            undated_entries.push((*index, Arc::clone(entry)));
        }
    }

    let mut months = groups
        .into_iter()
        .rev()
        .map(|((year, month), items)| {
            let title = format!("{year} 年 {month:02} 月");
            let anchor_id = group_anchor_id(&format!("{}-{}", title, items[0].1.id));
            EntrySourceMonthGroup {
                anchor_id,
                title,
                subtitle: format!("{} 篇文章", items.len()),
                target_page: page_for_index(items[0].0, page_size),
                entries: items.into_iter().map(|(_, entry)| entry).collect(),
            }
        })
        .collect::<Vec<_>>();

    if !undated_entries.is_empty() {
        let title = "未标注日期".to_string();
        let anchor_id = group_anchor_id(&format!("{}-{}", title, undated_entries[0].1.id));
        months.push(EntrySourceMonthGroup {
            anchor_id,
            title,
            subtitle: format!("{} 篇文章", undated_entries.len()),
            target_page: page_for_index(undated_entries[0].0, page_size),
            entries: undated_entries.into_iter().map(|(_, entry)| entry).collect(),
        });
    }

    months
}

fn page_for_index(index: usize, page_size: usize) -> u32 {
    (index / page_size.max(1)) as u32 + 1
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
        build_directory_months, build_group_nav_items, build_month_nav_items,
        find_active_source_anchors, find_active_time_anchors, group_entries_by_source_tree,
        group_entries_by_time_tree,
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

        let groups = group_entries_by_time_tree(&entries, 100);

        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].title, "2026 年 04 月");
        assert_eq!(groups[0].dates[0].title, "2026-04-02");
        assert_eq!(groups[0].dates[0].sources[0].title, "Beta");
        assert_eq!(groups[0].dates[0].sources[0].entries[0].title, "April two");
        assert_eq!(groups[0].target_page, 1);
    }

    #[test]
    fn source_groups_keep_first_entry_target_page() {
        let entries = vec![
            entry(1, "Beta", "Beta newest", Some("2026-04-05T08:00:00Z")),
            entry(2, "Alpha", "Alpha newest", Some("2026-04-04T08:00:00Z")),
            entry(3, "Alpha", "Alpha second", Some("2026-04-03T08:00:00Z")),
            entry(4, "Beta", "Beta second", Some("2026-04-02T08:00:00Z")),
        ];

        let groups = group_entries_by_source_tree(&entries, 2);

        assert_eq!(groups[0].title, "Beta");
        assert_eq!(groups[0].target_page, 1);
        assert_eq!(groups[1].title, "Alpha");
        assert_eq!(groups[1].target_page, 1);
    }

    #[test]
    fn finds_active_time_anchors_for_current_page_entry() {
        let entries = vec![
            entry(1, "Alpha", "April one", Some("2026-04-03T08:00:00Z")),
            entry(2, "Alpha", "April two", Some("2026-04-02T08:00:00Z")),
        ];
        let groups = group_entries_by_time_tree(&entries, 100);

        let (group_anchor, directory_anchor) = find_active_time_anchors(&groups, Some(2));

        assert_eq!(group_anchor.as_deref(), Some(groups[0].anchor_id.as_str()));
        assert_eq!(directory_anchor.as_deref(), Some(groups[0].dates[1].anchor_id.as_str()));
    }

    #[test]
    fn builds_active_nav_items_for_time_groups() {
        let entries = vec![
            entry(1, "Alpha", "April one", Some("2026-04-03T08:00:00Z")),
            entry(2, "Alpha", "March one", Some("2026-03-02T08:00:00Z")),
        ];
        let groups = group_entries_by_time_tree(&entries, 100);
        let nav = build_month_nav_items(&groups, Some(groups[0].anchor_id.as_str()));
        let directory = build_directory_months(
            &groups,
            Some(groups[0].anchor_id.as_str()),
            Some(groups[0].dates[0].anchor_id.as_str()),
        );

        assert!(nav[0].is_active);
        assert!(directory[0].is_active);
        assert!(directory[0].dates[0].is_active);
    }

    #[test]
    fn builds_active_nav_items_for_source_groups() {
        let entries = vec![
            entry(1, "Alpha", "Alpha", Some("2026-04-03T08:00:00Z")),
            entry(2, "Beta", "Beta", Some("2026-04-02T08:00:00Z")),
        ];
        let groups = group_entries_by_source_tree(&entries, 100);
        let (group_anchor, _) = find_active_source_anchors(&groups, Some(1));
        let nav = build_group_nav_items(&groups, group_anchor.as_deref());

        assert!(nav.iter().any(|item| item.is_active));
    }
}

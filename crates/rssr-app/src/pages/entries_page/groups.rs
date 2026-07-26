use std::collections::BTreeMap;

use time::{OffsetDateTime, UtcOffset};

use crate::datetime::format_date_utc;

/// 分组键：条目在**完整可见列表**中的下标，加上分组真正依赖的那几个字段。
///
/// 刻意不含 `title` / `is_read` / `is_starred`：分组结构与它们无关，把它们挡在类型之外，
/// 分组树在结构上就无法携带会过期的字段——叶子只保留 [`EntryCardRef`]，卡片渲染时再解析
/// （见 `EntriesPageFacade::entry_at`）。这是标记切换不必重建分组树的前提。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EntryGroupKey<'a> {
    /// 在完整可见列表中的下标。分页树是全量键切片出来的子切片，因此两棵树的下标都是绝对下标。
    pub(crate) index: usize,
    pub(crate) id: i64,
    pub(crate) feed_title: &'a str,
    pub(crate) published_at: Option<OffsetDateTime>,
}

/// 分组树叶子里指向一条条目的引用。
///
/// `index` 是完整可见列表中的下标，`id` 用来校验它。两者都带上是刻意的：
/// 分组树来自 memo 缓存，它跟上状态写入的第一跳（状态信号 → 投影 memo）依赖 dioxus 调度器的
/// 任务次序，只按下标解析在这一跳失效的那一帧会把**另一篇文章**渲染到这个位置上。
/// 带上 `id` 后最坏情况退化成「这张卡片这一帧没渲染」。
/// 完整论证见 `EntriesPageFacade::entry_at`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EntryCardRef {
    pub(crate) index: usize,
    pub(crate) id: i64,
}

impl EntryCardRef {
    fn from_key(key: &EntryGroupKey<'_>) -> Self {
        Self { index: key.index, id: key.id }
    }
}

type MonthKeyedEntries<'a> = BTreeMap<(i32, u8), Vec<EntryGroupKey<'a>>>;

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
    /// 指向条目的引用，按完整可见列表中的下标升序。
    pub(crate) entry_cards: Vec<EntryCardRef>,
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
    /// 指向条目的引用，按完整可见列表中的下标升序。
    pub(crate) entry_cards: Vec<EntryCardRef>,
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
    entries: &[EntryGroupKey<'_>],
    page_size: usize,
) -> Vec<EntryMonthGroup> {
    let mut groups = MonthKeyedEntries::new();
    let mut undated_entries = Vec::new();

    for entry in entries {
        if let Some(published_at) = entry.published_at {
            let published_at = published_at.to_offset(UtcOffset::UTC);
            groups
                .entry((published_at.year(), published_at.month() as u8))
                .or_default()
                .push(*entry);
        } else {
            undated_entries.push(*entry);
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
                target_page: page_for_index(items[0].index, page_size),
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
            target_page: page_for_index(undated_entries[0].index, page_size),
            dates: group_date_buckets(&undated_entries, page_size),
        });
    }

    grouped
}

pub(crate) fn group_entries_by_source_tree<'a>(
    entries: &[EntryGroupKey<'a>],
    page_size: usize,
) -> Vec<EntrySourceGroup> {
    let mut groups: BTreeMap<&'a str, Vec<EntryGroupKey<'a>>> = BTreeMap::new();
    let mut latest_seen: BTreeMap<&'a str, Option<OffsetDateTime>> = BTreeMap::new();

    for entry in entries {
        groups.entry(entry.feed_title).or_default().push(*entry);
        let latest = latest_seen.entry(entry.feed_title).or_insert(None);
        if latest.is_none() || entry.published_at > *latest {
            *latest = entry.published_at;
        }
    }

    let mut grouped = groups
        .into_iter()
        .map(|(feed_title, items)| {
            let latest = latest_seen.get(feed_title).and_then(|value| *value);
            (
                latest,
                EntrySourceGroup {
                    anchor_id: group_anchor_id(feed_title),
                    title: feed_title.to_string(),
                    subtitle: format!("{} 篇文章", items.len()),
                    target_page: page_for_index(items[0].index, page_size),
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

/// 找出当前页首条所在的月 / 日锚点。
///
/// `current_entry_index` 是**完整可见列表**中的下标，与叶子里存的下标同一套。
pub(crate) fn find_active_time_anchors(
    groups: &[EntryMonthGroup],
    current_entry_index: Option<usize>,
) -> (Option<String>, Option<String>) {
    let Some(current_entry_index) = current_entry_index else {
        return (None, None);
    };

    for month in groups {
        for date in &month.dates {
            for source in &date.sources {
                if source.entry_cards.iter().any(|card| card.index == current_entry_index) {
                    return (Some(month.anchor_id.clone()), Some(date.anchor_id.clone()));
                }
            }
        }
    }

    (None, None)
}

/// 找出当前页首条所在的来源 / 月锚点。下标语义同 [`find_active_time_anchors`]。
pub(crate) fn find_active_source_anchors(
    groups: &[EntrySourceGroup],
    current_entry_index: Option<usize>,
) -> (Option<String>, Option<String>) {
    let Some(current_entry_index) = current_entry_index else {
        return (None, None);
    };

    for source in groups {
        for month in &source.months {
            if month.entry_cards.iter().any(|card| card.index == current_entry_index) {
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

fn group_date_buckets(entries: &[EntryGroupKey<'_>], page_size: usize) -> Vec<EntryDateGroup> {
    let mut groups: BTreeMap<String, Vec<EntryGroupKey<'_>>> = BTreeMap::new();

    for entry in entries {
        let key = format_date_utc(entry.published_at).unwrap_or_else(|| "未标注日期".to_string());
        groups.entry(key).or_default().push(*entry);
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
                target_page: page_for_index(items[0].index, page_size),
                sources: group_date_sources(&items, page_size),
            }
        })
        .collect()
}

fn group_date_sources<'a>(
    entries: &[EntryGroupKey<'a>],
    page_size: usize,
) -> Vec<EntryDateSourceGroup> {
    let mut groups: BTreeMap<&'a str, Vec<EntryGroupKey<'a>>> = BTreeMap::new();

    for entry in entries {
        groups.entry(entry.feed_title).or_default().push(*entry);
    }

    groups
        .into_iter()
        .map(|(feed_title, items)| {
            let anchor_id = group_anchor_id(&format!("{}-{}", feed_title, items[0].id));
            EntryDateSourceGroup {
                anchor_id,
                title: feed_title.to_string(),
                subtitle: format!("{} 篇文章", items.len()),
                target_page: page_for_index(items[0].index, page_size),
                entry_cards: items.iter().map(EntryCardRef::from_key).collect(),
            }
        })
        .collect()
}

fn group_source_months(
    entries: &[EntryGroupKey<'_>],
    page_size: usize,
) -> Vec<EntrySourceMonthGroup> {
    let mut groups = MonthKeyedEntries::new();
    let mut undated_entries = Vec::new();

    for entry in entries {
        if let Some(published_at) = entry.published_at {
            let published_at = published_at.to_offset(UtcOffset::UTC);
            groups
                .entry((published_at.year(), published_at.month() as u8))
                .or_default()
                .push(*entry);
        } else {
            undated_entries.push(*entry);
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
                target_page: page_for_index(items[0].index, page_size),
                entry_cards: items.iter().map(EntryCardRef::from_key).collect(),
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
            target_page: page_for_index(undated_entries[0].index, page_size),
            entry_cards: undated_entries.iter().map(EntryCardRef::from_key).collect(),
        });
    }

    months
}

fn page_for_index(index: usize, page_size: usize) -> u32 {
    (index / page_size.max(1)) as u32 + 1
}

#[cfg(test)]
mod tests {
    use super::{
        EntryCardRef, EntryGroupKey, build_directory_months, build_group_nav_items,
        build_month_nav_items, find_active_source_anchors, find_active_time_anchors,
        group_entries_by_source_tree, group_entries_by_time_tree,
    };
    use time::{OffsetDateTime, format_description::well_known::Rfc3339};

    fn key<'a>(
        index: usize,
        id: i64,
        feed_title: &'a str,
        published_at: Option<&str>,
    ) -> EntryGroupKey<'a> {
        EntryGroupKey {
            index,
            id,
            feed_title,
            published_at: published_at
                .map(|value| OffsetDateTime::parse(value, &Rfc3339).expect("parse published_at")),
        }
    }

    /// 把叶子拍平成 `(下标, id)`，方便一次断言两者。
    fn cards(entry_cards: &[EntryCardRef]) -> Vec<(usize, i64)> {
        entry_cards.iter().map(|card| (card.index, card.id)).collect()
    }

    #[test]
    fn groups_entries_by_time_in_descending_month_order() {
        let entries = vec![
            key(0, 4, "Beta", Some("2026-04-02T09:00:00Z")),
            key(1, 2, "Beta", Some("2026-04-02T08:00:00Z")),
            key(2, 1, "Alpha", Some("2026-03-21T08:00:00Z")),
            key(3, 3, "Beta", None),
        ];

        let groups = group_entries_by_time_tree(&entries, 100);

        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].title, "2026 年 04 月");
        assert_eq!(groups[0].dates[0].title, "2026-04-02");
        assert_eq!(groups[0].dates[0].sources[0].title, "Beta");
        assert_eq!(cards(&groups[0].dates[0].sources[0].entry_cards), vec![(0, 4), (1, 2)]);
        assert_eq!(groups[0].target_page, 1);
        assert_eq!(groups[2].title, "未标注日期");
        assert_eq!(cards(&groups[2].dates[0].sources[0].entry_cards), vec![(3, 3)]);
    }

    /// 叶子里存的必须是传入键携带的下标与 id，而不是在切片里的位置——
    /// 分页树是全量键的子切片，若改用切片位置，卡片会解析到别的条目上。
    #[test]
    fn leaf_refs_come_from_the_key_not_the_slice_position() {
        let entries = vec![
            key(7, 1, "Alpha", Some("2026-04-03T08:00:00Z")),
            key(8, 2, "Alpha", Some("2026-04-02T08:00:00Z")),
        ];

        let time_groups = group_entries_by_time_tree(&entries, 100);
        assert_eq!(cards(&time_groups[0].dates[0].sources[0].entry_cards), vec![(7, 1)]);
        assert_eq!(cards(&time_groups[0].dates[1].sources[0].entry_cards), vec![(8, 2)]);

        let source_groups = group_entries_by_source_tree(&entries, 100);
        assert_eq!(cards(&source_groups[0].months[0].entry_cards), vec![(7, 1), (8, 2)]);
    }

    #[test]
    fn source_groups_keep_first_entry_target_page() {
        let entries = vec![
            key(0, 1, "Beta", Some("2026-04-05T08:00:00Z")),
            key(1, 2, "Alpha", Some("2026-04-04T08:00:00Z")),
            key(2, 3, "Alpha", Some("2026-04-03T08:00:00Z")),
            key(3, 4, "Beta", Some("2026-04-02T08:00:00Z")),
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
            key(0, 1, "Alpha", Some("2026-04-03T08:00:00Z")),
            key(1, 2, "Alpha", Some("2026-04-02T08:00:00Z")),
        ];
        let groups = group_entries_by_time_tree(&entries, 100);

        let (group_anchor, directory_anchor) = find_active_time_anchors(&groups, Some(1));

        assert_eq!(group_anchor.as_deref(), Some(groups[0].anchor_id.as_str()));
        assert_eq!(directory_anchor.as_deref(), Some(groups[0].dates[1].anchor_id.as_str()));
    }

    #[test]
    fn builds_active_nav_items_for_time_groups() {
        let entries = vec![
            key(0, 1, "Alpha", Some("2026-04-03T08:00:00Z")),
            key(1, 2, "Alpha", Some("2026-03-02T08:00:00Z")),
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
            key(0, 1, "Alpha", Some("2026-04-03T08:00:00Z")),
            key(1, 2, "Beta", Some("2026-04-02T08:00:00Z")),
        ];
        let groups = group_entries_by_source_tree(&entries, 100);
        let (group_anchor, _) = find_active_source_anchors(&groups, Some(0));
        let nav = build_group_nav_items(&groups, group_anchor.as_deref());

        assert!(nav.iter().any(|item| item.is_active));
    }
}

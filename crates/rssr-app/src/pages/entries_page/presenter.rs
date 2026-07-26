use std::{collections::BTreeSet, sync::Arc};

use rssr_domain::{EntrySummary, FeedSummary};

use super::{
    groups::{
        EntryDirectoryMonth, EntryDirectorySource, EntryGroupNavItem, EntryMonthGroup,
        EntrySourceGroup, build_directory_months, build_directory_sources, build_group_nav_items,
        build_month_nav_items, find_active_source_anchors, find_active_time_anchors,
        group_entries_by_source_tree, group_entries_by_time_tree,
    },
    state::{EntriesPageState, EntryGroupingMode},
};

/// presenter 真正依赖的那部分状态。
///
/// Dioxus 的信号订阅粒度是「整个信号」，读任何一个字段都会订阅整份 `EntriesPageState`，
/// 于是 `SetStatus`、`SetControlsHidden` 这类与分组毫无关系的 intent 也会让 presenter 失效、
/// 重建两棵分组树。把依赖收窄成这个投影后，可以用 memo 链：投影 memo 每次状态变化都会重算，
/// 但它的值没变时，presenter memo 不会重算。
///
/// 代价说明（别把它当成零成本）：重算一次投影是 N 次 `Arc` 指针拷贝加一次 `feeds` 深拷贝
/// （`FeedSummary` 含三个 `String`，订阅数量级通常是几十）。比较则基本是指针比较——
/// `EntrySummary` 派生了 `Eq`，`Arc<T: Eq>` 的相等判断走 `ptr_eq` 短路。
/// 换掉的是两棵分组树的重建，因此净赚，但不是免费。
///
/// 注意这里**不包含** `status` / `status_tone` / `controls_hidden` / `show_archived` /
/// 读取与收藏筛选 / `selected_feed_urls` / `archive_after_months` / `preferences_loaded`——
/// 归档筛选已经下沉到查询层，筛选变化会走一次重新加载并以 `SetEntries` 收尾。
///
/// `archived_count` 在这里不是因为它影响分组（presenter 只是原样透传给
/// `facade.archived_entry_count()`），而是因为 presenter 要暴露它。
#[derive(Clone, PartialEq)]
pub(crate) struct EntriesPresenterInput {
    entries: Vec<Arc<EntrySummary>>,
    archived_count: usize,
    entries_page_size: u32,
    current_page: u32,
    feeds: Vec<FeedSummary>,
    grouping_mode: EntryGroupingMode,
    /// 保留在投影里，好让「按订阅浏览 ⇒ 无来源筛选项」这条不变量继续由 `from_input` 强制执行，
    /// 而不是退化成一句「调用方保证 feeds 已经是空的」的注释。标量，只随路由变化。
    feed_id: Option<i64>,
}

impl EntriesPresenterInput {
    pub(crate) fn from_state(state: &EntriesPageState, feed_id: Option<i64>) -> Self {
        Self {
            // 只是 Arc 指针拷贝。
            entries: state.entries.clone(),
            archived_count: state.archived_count,
            entries_page_size: state.entries_page_size,
            current_page: state.current_page,
            // 订阅列表只在 bootstrap 时变化；参与比较是为了让来源筛选项跟着更新。
            feeds: state.feeds.clone(),
            grouping_mode: state.grouping_mode,
            feed_id,
        }
    }

    fn page_size(&self) -> usize {
        self.entries_page_size.max(1) as usize
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct EntriesPagePresenter {
    pub(crate) archived_count: usize,
    pub(crate) visible_entries_len: usize,
    pub(crate) page_size: usize,
    pub(crate) current_page: u32,
    pub(crate) total_pages: u32,
    pub(crate) page_start: usize,
    pub(crate) page_end: usize,
    pub(crate) source_filter_options: Vec<(i64, String, String)>,
    pub(crate) source_grouped_entries: Vec<EntrySourceGroup>,
    pub(crate) time_grouped_entries: Vec<EntryMonthGroup>,
    pub(crate) directory_months: Vec<EntryDirectoryMonth>,
    pub(crate) directory_sources: Vec<EntryDirectorySource>,
    pub(crate) default_expanded_directory_sections: BTreeSet<String>,
    pub(crate) group_nav_items: Vec<EntryGroupNavItem>,
    pub(crate) active_group_anchor: Option<String>,
    pub(crate) active_directory_anchor: Option<String>,
}

impl EntriesPagePresenter {
    /// 从 [`EntriesPresenterInput`] 派生这一屏要渲染的内容。
    ///
    /// 归档筛选与计数已经由存储层完成（见 `EntriesPageState::entry_query` 与
    /// `EntriesListService::list_entries`），因此这里拿到的条目就是可显示集合，
    /// 不再需要当前时间。输入是一个窄投影，因此本函数只在真正影响输出的字段变化时才会重跑。
    pub(crate) fn from_input(input: &EntriesPresenterInput) -> Self {
        let archived_count = input.archived_count;
        // 借用即可：输入本身是借来的，条目只以 `&[_]` 形式传给分组函数。
        let visible_entries = &input.entries;

        let visible_entries_len = visible_entries.len();
        let page_size = input.page_size();
        let total_pages = if visible_entries_len == 0 {
            0
        } else {
            ((visible_entries_len - 1) / page_size) as u32 + 1
        };
        let current_page =
            if total_pages == 0 { 1 } else { input.current_page.min(total_pages).max(1) };
        let page_start_index =
            if total_pages == 0 { 0 } else { ((current_page - 1) as usize) * page_size };
        let page_end_index = visible_entries_len.min(page_start_index.saturating_add(page_size));
        let paged_entries = visible_entries[page_start_index..page_end_index].to_vec();
        let page_start = if visible_entries_len == 0 { 0 } else { page_start_index + 1 };
        let page_end = if visible_entries_len == 0 { 0 } else { page_end_index };
        // 按订阅浏览时没有来源筛选项——这条不变量在使用点强制执行。
        let source_filter_options = if input.feed_id.is_some() {
            Vec::new()
        } else {
            input
                .feeds
                .iter()
                .map(|feed| (feed.id, feed.title.clone(), feed.url.clone()))
                .collect::<Vec<_>>()
        };

        let current_entry_id = paged_entries.first().map(|entry| entry.id);

        let (
            time_grouped_entries,
            source_grouped_entries,
            directory_months,
            directory_sources,
            default_expanded_directory_sections,
            group_nav_items,
            active_group_anchor,
            active_directory_anchor,
        ) = match input.grouping_mode {
            EntryGroupingMode::Time => {
                let all_groups = group_entries_by_time_tree(visible_entries, page_size);
                let paged_groups = group_entries_by_time_tree(&paged_entries, page_size);
                let default_expanded_directory_sections =
                    paged_groups.iter().map(|group| group.anchor_id.clone()).collect();
                let (active_group_anchor, active_directory_anchor) =
                    find_active_time_anchors(&all_groups, current_entry_id);
                let directory_months = build_directory_months(
                    &all_groups,
                    active_group_anchor.as_deref(),
                    active_directory_anchor.as_deref(),
                );
                let group_nav_items =
                    build_month_nav_items(&all_groups, active_group_anchor.as_deref());
                (
                    paged_groups,
                    Vec::new(),
                    directory_months,
                    Vec::new(),
                    default_expanded_directory_sections,
                    group_nav_items,
                    active_group_anchor,
                    active_directory_anchor,
                )
            }
            EntryGroupingMode::Source => {
                let all_groups = group_entries_by_source_tree(visible_entries, page_size);
                let paged_groups = group_entries_by_source_tree(&paged_entries, page_size);
                let default_expanded_directory_sections =
                    paged_groups.iter().map(|group| group.anchor_id.clone()).collect();
                let (active_group_anchor, active_directory_anchor) =
                    find_active_source_anchors(&all_groups, current_entry_id);
                let directory_sources = build_directory_sources(
                    &all_groups,
                    active_group_anchor.as_deref(),
                    active_directory_anchor.as_deref(),
                );
                let group_nav_items =
                    build_group_nav_items(&all_groups, active_group_anchor.as_deref());
                (
                    Vec::new(),
                    paged_groups,
                    Vec::new(),
                    directory_sources,
                    default_expanded_directory_sections,
                    group_nav_items,
                    active_group_anchor,
                    active_directory_anchor,
                )
            }
        };

        Self {
            archived_count,
            visible_entries_len,
            page_size,
            current_page,
            total_pages,
            page_start,
            page_end,
            source_filter_options,
            source_grouped_entries,
            time_grouped_entries,
            directory_months,
            directory_sources,
            default_expanded_directory_sections,
            group_nav_items,
            active_group_anchor,
            active_directory_anchor,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rssr_domain::{EntrySummary, FeedSummary};
    use time::{OffsetDateTime, format_description::well_known::Rfc3339};

    use super::{EntriesPagePresenter, EntriesPresenterInput};
    use crate::pages::entries_page::state::{EntriesPageState, EntryGroupingMode};

    fn parse_datetime(raw: &str) -> OffsetDateTime {
        OffsetDateTime::parse(raw, &Rfc3339).expect("parse datetime")
    }

    fn entry(id: i64, feed_id: i64, feed_title: &str, published_at: &str) -> Arc<EntrySummary> {
        Arc::new(EntrySummary {
            id,
            feed_id,
            title: format!("Entry {id}"),
            feed_title: feed_title.to_string(),
            published_at: Some(parse_datetime(published_at)),
            is_read: false,
            is_starred: false,
        })
    }

    fn feed(id: i64, title: &str, url: &str) -> FeedSummary {
        FeedSummary {
            id,
            title: title.to_string(),
            url: url.to_string(),
            unread_count: 0,
            entry_count: 0,
            last_fetched_at: None,
            last_success_at: None,
            fetch_error: None,
        }
    }

    /// memo 链能省下重建的前提：不参与渲染派生的状态变化必须让投影保持**相等**。
    /// 这些字段一旦被误加进 `EntriesPresenterInput`，分组树就会为状态提示之类的变化重建。
    #[test]
    fn presenter_input_excludes_state_that_does_not_change_rendering() {
        let mut state = EntriesPageState::new(true);
        state.entries = vec![entry(1, 1, "Alpha", "2026-04-04T08:00:00Z")];
        let baseline = EntriesPresenterInput::from_state(&state, None);

        // 每次切换已读/收藏都会附带发一条 SetStatus。
        state.status = "已将《X》标记为已读。".to_string();
        state.status_tone = "info".to_string();
        state.controls_hidden = !state.controls_hidden;
        state.show_archived = !state.show_archived;
        state.read_filter = rssr_domain::ReadFilter::UnreadOnly;
        state.starred_filter = rssr_domain::StarredFilter::StarredOnly;
        state.selected_feed_urls = vec!["https://example.com/a.xml".to_string()];
        state.archive_after_months = 12;
        state.preferences_loaded = true;

        assert!(
            baseline == EntriesPresenterInput::from_state(&state, None),
            "与分组无关的状态变化不应让 presenter 投影发生变化，否则分组树会被无谓重建"
        );
    }

    /// 反面：投影里的每个字段变化都必须让投影不相等，否则界面不会更新。
    ///
    /// 注意 `archived_count` 并不影响**分组**，它只是被 presenter 原样透传给
    /// `facade.archived_entry_count()`；它在投影里是因为 presenter 要暴露它，
    /// 所以这里断言的是「投影字段变化能被看见」，不是「它影响分组」。
    #[test]
    fn presenter_input_includes_every_field_that_changes_rendering() {
        let mut state = EntriesPageState::new(true);
        state.entries = vec![entry(1, 1, "Alpha", "2026-04-04T08:00:00Z")];
        let baseline = EntriesPresenterInput::from_state(&state, None);

        let mut paged = state.clone();
        paged.current_page = 2;
        assert!(baseline != EntriesPresenterInput::from_state(&paged, None));

        let mut resized = state.clone();
        resized.entries_page_size = 5;
        assert!(baseline != EntriesPresenterInput::from_state(&resized, None));

        let mut regrouped = state.clone();
        regrouped.grouping_mode = EntryGroupingMode::Source;
        assert!(baseline != EntriesPresenterInput::from_state(&regrouped, None));

        let mut counted = state.clone();
        counted.archived_count = 3;
        assert!(baseline != EntriesPresenterInput::from_state(&counted, None));

        // 已读标记变化必须被看见：卡片的已读/未读文案来自条目本身。
        let mut flagged = state.clone();
        flagged.entries =
            vec![Arc::new(EntrySummary { is_read: true, ..(*state.entries[0]).clone() })];
        assert!(baseline != EntriesPresenterInput::from_state(&flagged, None));

        // 订阅列表变化必须被看见，否则来源筛选下拉框不会刷新——这正是本组测试要防的症状。
        let mut refeeded = state.clone();
        refeeded.feeds = vec![feed(1, "Alpha", "https://example.com/alpha.xml")];
        assert!(baseline != EntriesPresenterInput::from_state(&refeeded, None));
    }

    /// 「按订阅浏览 ⇒ 无来源筛选项」这条不变量由 `from_input` 强制执行，
    /// 因此 `feed_id` 必须留在投影里，且两条路由要给出不同结果。
    #[test]
    fn browsing_a_single_feed_hides_the_source_filter() {
        let mut state = EntriesPageState::new(true);
        state.entries = vec![entry(1, 1, "Alpha", "2026-04-04T08:00:00Z")];
        state.feeds = vec![feed(1, "Alpha", "https://example.com/alpha.xml")];

        let all_feeds = EntriesPresenterInput::from_state(&state, None);
        let single_feed = EntriesPresenterInput::from_state(&state, Some(1));
        assert!(all_feeds != single_feed, "路由不同必须让投影不同");

        assert_eq!(EntriesPagePresenter::from_input(&all_feeds).source_filter_options.len(), 1);
        assert!(
            EntriesPagePresenter::from_input(&single_feed).source_filter_options.is_empty(),
            "按订阅浏览时不应再给出来源筛选项"
        );
    }

    #[test]
    fn presenter_uses_page_slice_for_rendering_and_full_scope_for_directory() {
        let mut state = EntriesPageState::new(true);
        state.entries_page_size = 2;
        state.current_page = 2;
        state.grouping_mode = EntryGroupingMode::Time;
        state.entries = vec![
            entry(1, 1, "Alpha", "2026-04-04T08:00:00Z"),
            entry(2, 1, "Alpha", "2026-04-03T08:00:00Z"),
            entry(3, 2, "Beta", "2026-04-02T08:00:00Z"),
            entry(4, 2, "Beta", "2026-04-01T08:00:00Z"),
        ];
        state.feeds = vec![
            feed(1, "Alpha", "https://example.com/alpha.xml"),
            feed(2, "Beta", "https://example.com/beta.xml"),
        ];

        let presenter =
            EntriesPagePresenter::from_input(&EntriesPresenterInput::from_state(&state, None));

        assert_eq!(presenter.visible_entries_len, 4);
        assert_eq!(presenter.current_page, 2);
        assert_eq!(presenter.total_pages, 2);
        assert_eq!(presenter.page_start, 3);
        assert_eq!(presenter.page_end, 4);
        assert!(
            presenter
                .default_expanded_directory_sections
                .contains(&presenter.time_grouped_entries[0].anchor_id)
        );
        let rendered_total = presenter
            .time_grouped_entries
            .iter()
            .flat_map(|month| month.dates.iter())
            .flat_map(|date| date.sources.iter())
            .map(|source| source.entries.len())
            .sum::<usize>();
        assert_eq!(rendered_total, 2);
        assert!(!presenter.directory_months.is_empty());
    }

    #[test]
    fn presenter_marks_directory_item_for_current_page_first_entry() {
        let mut state = EntriesPageState::new(true);
        state.entries_page_size = 1;
        state.current_page = 2;
        state.grouping_mode = EntryGroupingMode::Source;
        state.entries = vec![
            entry(1, 1, "Alpha", "2026-04-04T08:00:00Z"),
            entry(2, 2, "Beta", "2026-04-03T08:00:00Z"),
        ];

        let presenter =
            EntriesPagePresenter::from_input(&EntriesPresenterInput::from_state(&state, None));

        assert_eq!(presenter.active_group_anchor.as_deref(), Some("entry-group-beta"));
        assert!(presenter.group_nav_items.iter().any(|item| item.is_active));
        assert!(presenter.directory_sources.iter().any(|item| item.is_active));
        assert!(presenter.default_expanded_directory_sections.contains("entry-group-beta"));
    }
}

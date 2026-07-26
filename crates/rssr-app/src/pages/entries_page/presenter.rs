use std::{collections::BTreeSet, sync::Arc};

use rssr_domain::{EntrySummary, FeedSummary};

use super::{
    groups::{
        EntryDirectoryMonth, EntryDirectorySource, EntryGroupKey, EntryGroupNavItem,
        EntryMonthGroup, EntrySourceGroup, build_directory_months, build_directory_sources,
        build_group_nav_items, build_month_nav_items, find_active_source_anchors,
        find_active_time_anchors, group_entries_by_source_tree, group_entries_by_time_tree,
    },
    state::{EntriesPageState, EntryGroupingMode},
};

/// 投影里持有的条目集合。
///
/// 只对外暴露分组键（下标 + `id` + `feed_title` + `published_at`），**取不到** `title` /
/// `is_read` / `is_starred`：因为本类型的相等性刻意忽略这些字段，memo 里缓存的这份数据在
/// 标记切换后就是过期的。卡片需要的完整条目在渲染时从最新状态解析
/// （`EntriesPageFacade::entry_at`），不经过这份缓存。
///
/// 这层封装是刻意的：直接放 `Vec<Arc<EntrySummary>>` 的话，日后往分组标题里加一句
/// 「未读 N 篇」就会读到过期标记，而且不会有任何编译错误提示。
#[derive(Clone)]
pub(crate) struct GroupingEntries(Vec<Arc<EntrySummary>>);

impl GroupingEntries {
    /// 只做 `Arc` 指针拷贝。
    fn new(entries: &[Arc<EntrySummary>]) -> Self {
        Self(entries.to_vec())
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    /// 借出分组键。下标是**完整可见列表**中的绝对下标。
    fn grouping_keys(&self) -> Vec<EntryGroupKey<'_>> {
        self.0
            .iter()
            .enumerate()
            .map(|(index, entry)| EntryGroupKey {
                index,
                id: entry.id,
                feed_title: entry.feed_title.as_str(),
                published_at: entry.published_at,
            })
            .collect()
    }
}

impl PartialEq for GroupingEntries {
    /// 相等 ⟺ 由这批条目派生出的分组树完全一致。
    ///
    /// 只比较分组真正依赖的字段（`id` / `feed_title` / `published_at`）以及数量和顺序。
    /// `title` / `is_read` / `is_starred` **刻意不比较**：它们只出现在卡片上，而卡片是渲染时
    /// 按下标从最新状态解析的，不受这份缓存影响。切换已读/收藏因此不再重建分组树。
    ///
    /// 由此得到一条被 `EntriesPageFacade::entry_at` 依赖的不变量：
    /// **两份投影相等 ⇒ 条目数量相同，且每个下标处仍是同一条目**，
    /// 所以缓存里那棵没重算的分组树，它的下标依旧有效。
    fn eq(&self, other: &Self) -> bool {
        self.0.len() == other.0.len()
            && self.0.iter().zip(other.0.iter()).all(|(left, right)| {
                // 没被改动的条目共享同一个 Arc，指针相等即可短路。
                Arc::ptr_eq(left, right)
                    || (left.id == right.id
                        && left.published_at == right.published_at
                        && left.feed_title == right.feed_title)
            })
    }
}

/// presenter 真正依赖的那部分状态。
///
/// Dioxus 的信号订阅粒度是「整个信号」，读任何一个字段都会订阅整份 `EntriesPageState`，
/// 于是 `SetStatus`、`SetControlsHidden` 这类与分组毫无关系的 intent 也会让 presenter 失效、
/// 重建两棵分组树。把依赖收窄成这个投影后，可以用 memo 链：投影 memo 每次状态变化都会重算，
/// 但它的值没变时，presenter memo 不会重算。
///
/// 代价说明（别把它当成零成本）：重算一次投影是 N 次 `Arc` 指针拷贝加一次 `feeds` 深拷贝
/// （`FeedSummary` 含三个 `String`，订阅数量级通常是几十）。条目比较基本是指针比较——
/// 未改动的条目共享 `Arc`，`GroupingEntries::eq` 以 `ptr_eq` 短路。
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
    entries: GroupingEntries,
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
            entries: GroupingEntries::new(&state.entries),
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
    ///
    /// 输出的分组树只携带条目引用（下标 + id），不携带条目本身：渲染时再解析，
    /// 这样这棵树对已读/收藏切换免疫。
    pub(crate) fn from_input(input: &EntriesPresenterInput) -> Self {
        let archived_count = input.archived_count;

        let visible_entries_len = input.entries.len();
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

        // 分组只需要键。渲染树的键是全量键的**子切片**，因此两棵树共用同一套绝对下标，
        // 不存在「相对下标」这种会让卡片解析错条目的表示。
        //
        // 前置条件：`current_page <= total_pages` ⇒ `page_start_index < visible_entries_len`
        // （`visible_entries_len > 0` 时），因此下面的切片区间总是合法的。
        let grouping_keys = input.entries.grouping_keys();
        let paged_keys = &grouping_keys[page_start_index..page_end_index];

        let grouping = match input.grouping_mode {
            EntryGroupingMode::Time => {
                GroupingOutcome::by_time(&grouping_keys, paged_keys, page_size)
            }
            EntryGroupingMode::Source => {
                GroupingOutcome::by_source(&grouping_keys, paged_keys, page_size)
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
            source_grouped_entries: grouping.source_grouped_entries,
            time_grouped_entries: grouping.time_grouped_entries,
            directory_months: grouping.directory_months,
            directory_sources: grouping.directory_sources,
            default_expanded_directory_sections: grouping.default_expanded_directory_sections,
            group_nav_items: grouping.group_nav_items,
            active_group_anchor: grouping.active_group_anchor,
            active_directory_anchor: grouping.active_directory_anchor,
        }
    }
}

/// 一种分组模式派生出的全部渲染数据。
///
/// 两种模式产出的树类型不同（月 / 来源），但流程一模一样：建全量树 → 建渲染树 →
/// 取默认展开的目录节 → 定位活动锚点 → 建目录 → 建导航项。把两条流程各自收进一个构造函数，
/// 是为了让「改了一条忘了改另一条」变成看得见的事——本轮改下标语义时两条就必须同步改。
struct GroupingOutcome {
    time_grouped_entries: Vec<EntryMonthGroup>,
    source_grouped_entries: Vec<EntrySourceGroup>,
    directory_months: Vec<EntryDirectoryMonth>,
    directory_sources: Vec<EntryDirectorySource>,
    default_expanded_directory_sections: BTreeSet<String>,
    group_nav_items: Vec<EntryGroupNavItem>,
    active_group_anchor: Option<String>,
    active_directory_anchor: Option<String>,
}

impl GroupingOutcome {
    /// `all_keys` 是完整可见列表的键，`paged_keys` 是它当页那一段的子切片。
    /// 目录与导航项一律从**全量**树派生，渲染树只用当页那棵。
    fn by_time(
        all_keys: &[EntryGroupKey<'_>],
        paged_keys: &[EntryGroupKey<'_>],
        page_size: usize,
    ) -> Self {
        let all_groups = group_entries_by_time_tree(all_keys, page_size);
        let paged_groups = group_entries_by_time_tree(paged_keys, page_size);
        let default_expanded_directory_sections =
            paged_groups.iter().map(|group| group.anchor_id.clone()).collect();
        let (active_group_anchor, active_directory_anchor) =
            find_active_time_anchors(&all_groups, current_entry_index(paged_keys));
        let directory_months = build_directory_months(
            &all_groups,
            active_group_anchor.as_deref(),
            active_directory_anchor.as_deref(),
        );
        let group_nav_items = build_month_nav_items(&all_groups, active_group_anchor.as_deref());

        Self {
            time_grouped_entries: paged_groups,
            source_grouped_entries: Vec::new(),
            directory_months,
            directory_sources: Vec::new(),
            default_expanded_directory_sections,
            group_nav_items,
            active_group_anchor,
            active_directory_anchor,
        }
    }

    /// 结构同 [`GroupingOutcome::by_time`]，只是树按来源分组。
    fn by_source(
        all_keys: &[EntryGroupKey<'_>],
        paged_keys: &[EntryGroupKey<'_>],
        page_size: usize,
    ) -> Self {
        let all_groups = group_entries_by_source_tree(all_keys, page_size);
        let paged_groups = group_entries_by_source_tree(paged_keys, page_size);
        let default_expanded_directory_sections =
            paged_groups.iter().map(|group| group.anchor_id.clone()).collect();
        let (active_group_anchor, active_directory_anchor) =
            find_active_source_anchors(&all_groups, current_entry_index(paged_keys));
        let directory_sources = build_directory_sources(
            &all_groups,
            active_group_anchor.as_deref(),
            active_directory_anchor.as_deref(),
        );
        let group_nav_items = build_group_nav_items(&all_groups, active_group_anchor.as_deref());

        Self {
            time_grouped_entries: Vec::new(),
            source_grouped_entries: paged_groups,
            directory_months: Vec::new(),
            directory_sources,
            default_expanded_directory_sections,
            group_nav_items,
            active_group_anchor,
            active_directory_anchor,
        }
    }
}

/// 当页首条在完整可见列表中的下标；空列表时为 `None`。目录高亮以它为准。
fn current_entry_index(paged_keys: &[EntryGroupKey<'_>]) -> Option<usize> {
    paged_keys.first().map(|key| key.index)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rssr_domain::{EntrySummary, FeedSummary};
    use time::{OffsetDateTime, format_description::well_known::Rfc3339};

    use super::{EntriesPagePresenter, EntriesPresenterInput};
    use crate::pages::entries_page::{
        groups::EntryCardRef,
        state::{EntriesPageState, EntryGroupingMode},
    };

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

    /// 模拟 `PatchEntryFlags`：`Arc::make_mut` 只会换掉被点那一条的 `Arc`。
    fn replace_entry(state: &mut EntriesPageState, position: usize, entry: EntrySummary) {
        state.entries[position] = Arc::new(entry);
    }

    /// 收集分组树叶子里的引用（两种分组模式各只有一棵树是非空的）。
    fn leaf_cards(presenter: &EntriesPagePresenter) -> Vec<EntryCardRef> {
        presenter
            .time_grouped_entries
            .iter()
            .flat_map(|month| month.dates.iter())
            .flat_map(|date| date.sources.iter())
            .flat_map(|source| source.entry_cards.iter().copied())
            .chain(
                presenter
                    .source_grouped_entries
                    .iter()
                    .flat_map(|group| group.months.iter())
                    .flat_map(|month| month.entry_cards.iter().copied()),
            )
            .collect()
    }

    fn leaf_indices(presenter: &EntriesPagePresenter) -> Vec<usize> {
        leaf_cards(presenter).into_iter().map(|card| card.index).collect()
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

    /// 条目字段里只有**影响分组结构**的那些才该让投影失效。
    ///
    /// `is_read` / `is_starred` / `title` 只出现在卡片上，而卡片按下标从最新状态解析
    /// （`facade.entry_at`），因此它们变化时应当复用分组树。这正是本轮改动要拿到的收益：
    /// 切换已读/收藏不再重建两棵树。
    #[test]
    fn only_grouping_relevant_entry_fields_invalidate_the_projection() {
        let mut state = EntriesPageState::new(true);
        state.entries = vec![entry(1, 1, "Alpha", "2026-04-04T08:00:00Z")];
        let baseline = EntriesPresenterInput::from_state(&state, None);
        let original = (*state.entries[0]).clone();

        let mut flagged = state.clone();
        replace_entry(
            &mut flagged,
            0,
            EntrySummary { is_read: true, is_starred: true, ..original.clone() },
        );
        assert!(
            baseline == EntriesPresenterInput::from_state(&flagged, None),
            "标记切换不应让投影失效，否则分组树会被重建"
        );

        let mut retitled = state.clone();
        replace_entry(
            &mut retitled,
            0,
            EntrySummary { title: "改过的标题".to_string(), ..original.clone() },
        );
        assert!(
            baseline == EntriesPresenterInput::from_state(&retitled, None),
            "标题不参与分组，刷新改写标题不该重建分组树"
        );

        let mut resourced = state.clone();
        replace_entry(
            &mut resourced,
            0,
            EntrySummary { feed_title: "Beta".to_string(), ..original.clone() },
        );
        assert!(
            baseline != EntriesPresenterInput::from_state(&resourced, None),
            "来源标题决定分组与分组标题，必须让投影失效"
        );

        let mut republished = state.clone();
        replace_entry(
            &mut republished,
            0,
            EntrySummary {
                published_at: Some(parse_datetime("2026-03-04T08:00:00Z")),
                ..original.clone()
            },
        );
        assert!(
            baseline != EntriesPresenterInput::from_state(&republished, None),
            "发布时间决定时间分组，必须让投影失效"
        );

        let mut reidentified = state.clone();
        replace_entry(&mut reidentified, 0, EntrySummary { id: 9, ..original.clone() });
        assert!(
            baseline != EntriesPresenterInput::from_state(&reidentified, None),
            "条目 id 参与锚点生成，必须让投影失效"
        );

        // PatchEntryFlags 里的 retain 会因筛选把条目移出列表，那是真正的结构变化。
        let mut shortened = state.clone();
        shortened.entries.clear();
        assert!(
            baseline != EntriesPresenterInput::from_state(&shortened, None),
            "条目数量变化必须让投影失效"
        );
    }

    /// 反面：投影里除条目以外的每个字段变化都必须让投影不相等，否则界面不会更新。
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

        // 订阅列表变化必须被看见，否则来源筛选下拉框不会刷新——这正是本组测试要防的症状。
        let mut refeeded = state.clone();
        refeeded.feeds = vec![feed(1, "Alpha", "https://example.com/alpha.xml")];
        assert!(baseline != EntriesPresenterInput::from_state(&refeeded, None));
    }

    /// `EntriesPageFacade::entry_at` 依赖的不变量：投影相等 ⇒ 缓存里那棵没重算的分组树，
    /// 它的下标仍然指向同一批条目。这条测试就是这条不变量的守卫。
    #[test]
    fn reusing_the_cached_tree_keeps_indices_pointing_at_the_same_entries() {
        let mut state = EntriesPageState::new(true);
        state.entries_page_size = 10;
        state.entries = vec![
            entry(1, 1, "Alpha", "2026-04-04T08:00:00Z"),
            entry(2, 1, "Alpha", "2026-04-03T08:00:00Z"),
            entry(3, 2, "Beta", "2026-04-02T08:00:00Z"),
            entry(4, 2, "Beta", "2026-04-01T08:00:00Z"),
        ];
        let ids_before = state.entries.iter().map(|entry| entry.id).collect::<Vec<_>>();

        let before = EntriesPresenterInput::from_state(&state, None);
        let cached = EntriesPagePresenter::from_input(&before);

        let toggled = (*state.entries[2]).clone();
        replace_entry(&mut state, 2, EntrySummary { is_starred: true, ..toggled });
        let after = EntriesPresenterInput::from_state(&state, None);
        assert!(before == after, "收藏切换不该让投影失效");

        // 这就是 `entry_at` 的快路径前提：叶子里的 (下标, id) 在新列表上依然自洽。
        let cards = leaf_cards(&cached);
        assert_eq!(cards.len(), state.entries.len(), "叶子必须覆盖当前页的全部条目");
        for card in cards {
            assert!(card.index < state.entries.len(), "缓存下标越界，entry_at 会退化到按 id 查找");
            assert_eq!(
                state.entries[card.index].id, ids_before[card.index],
                "同一下标必须仍指向同一条目，否则卡片会串位"
            );
            assert_eq!(state.entries[card.index].id, card.id, "叶子里的 id 必须与下标处的条目一致");
        }
        assert!(cached == EntriesPagePresenter::from_input(&after), "复用的分组树必须与重建的一致");
    }

    /// 空列表：`page_start_index == page_end_index == 0`，切片 `&keys[0..0]` 必须合法且不 panic，
    /// 活动锚点为 `None`。分页相关标量在这里也有独立取值（`total_pages == 0`、`page_start == 0`）。
    #[test]
    fn empty_entry_list_produces_an_empty_presenter() {
        let mut state = EntriesPageState::new(true);
        state.current_page = 3;

        for mode in [EntryGroupingMode::Time, EntryGroupingMode::Source] {
            state.grouping_mode = mode;
            let presenter =
                EntriesPagePresenter::from_input(&EntriesPresenterInput::from_state(&state, None));

            assert_eq!(presenter.visible_entries_len, 0);
            assert_eq!(presenter.total_pages, 0);
            assert_eq!(presenter.current_page, 1, "空列表必须把越界的当前页夹回第 1 页");
            assert_eq!(presenter.page_start, 0);
            assert_eq!(presenter.page_end, 0);
            assert!(leaf_cards(&presenter).is_empty());
            assert!(presenter.group_nav_items.is_empty());
            assert!(presenter.active_group_anchor.is_none());
            assert!(presenter.active_directory_anchor.is_none());
        }
    }

    /// 渲染树是全量键的子切片，叶子必须是**绝对下标**；
    /// 若退化成页内相对下标，第 2 页的卡片会解析到第 1 页的条目上。
    #[test]
    fn paged_group_leaves_carry_absolute_indices() {
        let mut state = EntriesPageState::new(true);
        state.entries_page_size = 2;
        state.current_page = 2;
        state.entries = vec![
            entry(1, 1, "Alpha", "2026-04-04T08:00:00Z"),
            entry(2, 1, "Alpha", "2026-04-03T08:00:00Z"),
            entry(3, 2, "Beta", "2026-04-02T08:00:00Z"),
            entry(4, 2, "Beta", "2026-04-01T08:00:00Z"),
        ];

        let time_presenter =
            EntriesPagePresenter::from_input(&EntriesPresenterInput::from_state(&state, None));
        assert_eq!(leaf_indices(&time_presenter), vec![2, 3]);

        state.grouping_mode = EntryGroupingMode::Source;
        let source_presenter =
            EntriesPagePresenter::from_input(&EntriesPresenterInput::from_state(&state, None));
        assert_eq!(leaf_indices(&source_presenter), vec![2, 3]);
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
            .map(|source| source.entry_cards.len())
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

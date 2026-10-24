use std::{collections::BTreeSet, sync::Arc};

use rssr_domain::{
    ArchiveFilter, EntriesWorkspaceState, EntryGroupingPreference, EntryQuery, EntrySummary,
    FeedSummary, ReadFilter, StarredFilter, UserSettings,
};

pub(crate) const FIRST_PAGE_NUMBER: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntryGroupingMode {
    Time,
    Source,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) enum PreferencesLoadState {
    #[default]
    Pending,
    Loaded,
    // Failed preferences may fall back for reading, but must never be saved over
    // the persisted values that could not be read.
    Unavailable(String),
}

impl PreferencesLoadState {
    pub(crate) fn can_load_entries(&self) -> bool {
        !matches!(self, Self::Pending)
    }

    pub(crate) fn is_loaded(&self) -> bool {
        matches!(self, Self::Loaded)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EntriesPageState {
    pub(crate) selected_entry_ids: Arc<BTreeSet<i64>>,
    pub(crate) pending_entry_writes: Arc<BTreeSet<i64>>,
    pub(crate) bulk_busy: bool,
    pub(crate) bulk_writing: bool,
    pub(crate) bulk_generation: u64,
    pub(crate) reload_revision: u64,
    pub(crate) flags_revision: u64,
    pub(crate) bulk_notice: Option<String>,

    /// Snapshots and presenter inputs share the immutable collection. Flag changes copy
    /// its pointer vector only when necessary, then clone only the changed EntrySummary.
    pub(crate) entries: Arc<Vec<Arc<EntrySummary>>>,
    /// 被归档筛选排除掉的条目数，由存储层 COUNT 得出。
    pub(crate) archived_count: usize,
    pub(crate) feeds: Arc<Vec<FeedSummary>>,
    pub(crate) read_filter: ReadFilter,
    pub(crate) starred_filter: StarredFilter,
    pub(crate) selected_feed_urls: Vec<String>,
    pub(crate) show_archived: bool,
    pub(crate) grouping_mode: EntryGroupingMode,
    pub(crate) archive_after_months: u32,
    pub(crate) entries_page_size: u32,
    pub(crate) current_page: u32,
    pub(crate) entries_loaded: bool,
    pub(crate) status: String,
    pub(crate) status_tone: String,
    pub(crate) preferences_load: PreferencesLoadState,
}

impl EntriesPageState {
    pub(crate) fn new() -> Self {
        let settings = UserSettings::default();
        let workspace = EntriesWorkspaceState::default();
        Self {
            selected_entry_ids: Arc::default(),
            pending_entry_writes: Arc::default(),
            bulk_busy: false,
            bulk_writing: false,
            bulk_generation: 0,
            reload_revision: 0,
            flags_revision: 0,
            bulk_notice: None,
            entries: Arc::default(),
            archived_count: 0,
            feeds: Arc::default(),
            read_filter: workspace.read_filter,
            starred_filter: workspace.starred_filter,
            selected_feed_urls: workspace.selected_feed_urls,
            show_archived: workspace.show_archived,
            grouping_mode: entry_grouping_mode_from_preference(workspace.grouping_mode),
            archive_after_months: settings.archive_after_months,
            entries_page_size: settings.entries_page_size.max(1),
            current_page: FIRST_PAGE_NUMBER,
            entries_loaded: false,
            status: "正在加载文章列表…".to_string(),
            status_tone: "info".to_string(),
            preferences_load: PreferencesLoadState::Pending,
        }
    }

    /// 构造不依赖时钟的查询范围。归档分界由页面 memo 在范围或刷新代次
    /// 变化时补入；选择、目录和状态文字重绘不能改变查询的时间参数。
    pub(crate) fn entry_query_scope(
        &self,
        feed_id: Option<i64>,
        search_title: Option<String>,
    ) -> EntryQuery {
        EntryQuery {
            feed_id,
            read_filter: self.read_filter,
            starred_filter: self.starred_filter,
            archive_filter: ArchiveFilter::All,
            feed_ids: if feed_id.is_some() {
                Vec::new()
            } else {
                map_selected_feed_urls_to_ids(&self.feeds, &self.selected_feed_urls)
            },
            search_title,
            limit: None,
        }
    }

    pub(crate) fn page_size(&self) -> usize {
        self.entries_page_size.max(1) as usize
    }

    pub(crate) fn current_page_entries(&self) -> &[Arc<EntrySummary>] {
        let start = ((self.current_page.saturating_sub(1) as usize)
            .saturating_mul(self.page_size()))
        .min(self.entries.len());
        let end = start.saturating_add(self.page_size()).min(self.entries.len());
        &self.entries[start..end]
    }

    pub(crate) fn position_key(&self, feed_id: Option<i64>, search: Option<&str>) -> String {
        format!(
            "list:{feed_id:?}:{:?}:{:?}:{:?}:{}:{}:{}:{:?}:{:?}",
            self.read_filter,
            self.starred_filter,
            self.grouping_mode,
            self.show_archived,
            self.archive_after_months,
            self.entries_page_size,
            self.selected_feed_urls,
            search.unwrap_or_default().trim()
        )
    }
}

pub(crate) fn entry_grouping_mode_from_preference(
    preference: EntryGroupingPreference,
) -> EntryGroupingMode {
    match preference {
        EntryGroupingPreference::Time => EntryGroupingMode::Time,
        EntryGroupingPreference::Source => EntryGroupingMode::Source,
    }
}

pub(crate) fn grouping_mode_preference(mode: EntryGroupingMode) -> EntryGroupingPreference {
    match mode {
        EntryGroupingMode::Time => EntryGroupingPreference::Time,
        EntryGroupingMode::Source => EntryGroupingPreference::Source,
    }
}

pub(crate) fn map_selected_feed_urls_to_ids(
    feeds: &[FeedSummary],
    selected_feed_urls: &[String],
) -> Vec<i64> {
    if selected_feed_urls.is_empty() {
        return Vec::new();
    }

    let selected = selected_feed_urls.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let ids = feeds
        .iter()
        .filter(|feed| selected.contains(feed.url.as_str()))
        .map(|feed| feed.id)
        .collect::<Vec<_>>();
    // An obsolete URL must match no subscriptions, never broaden to the empty/all scope.
    if ids.is_empty() { vec![-1] } else { ids }
}

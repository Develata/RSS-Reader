use std::{collections::BTreeSet, sync::Arc};

use rssr_domain::{
    ArchiveFilter, EntriesWorkspaceState, EntryGroupingPreference, EntryQuery, EntrySummary,
    FeedSummary, ReadFilter, StarredFilter, UserSettings,
};
use time::OffsetDateTime;

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
    pub(crate) bulk_preview: Option<rssr_domain::MarkReadPreview>,
    pub(crate) bulk_busy: bool,
    pub(crate) bulk_writing: bool,
    pub(crate) bulk_generation: u64,
    pub(crate) bulk_revision: u64,
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
    pub(crate) controls_hidden: bool,
    pub(crate) current_page: u32,
    pub(crate) entries_loaded: bool,
    pub(crate) status: String,
    pub(crate) status_tone: String,
    pub(crate) preferences_load: PreferencesLoadState,
    /// 是否有列表查询在途（最新一代尚未落地）。
    pub(crate) query_in_flight: bool,
    /// 在途查询被写入作废时递增，页面据此重新发出列表查询（审计 A3）。
    pub(crate) requery_revision: u64,
}

/// 见 [`EntriesPageState::query_clock_trigger`]。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QueryClockTrigger {
    refresh_revision: u64,
    bulk_revision: u64,
    feed_id: Option<i64>,
    search_title: Option<String>,
    show_archived: bool,
    read_filter: ReadFilter,
    starred_filter: StarredFilter,
    selected_feed_urls: Vec<String>,
    archive_after_months: u32,
}

impl EntriesPageState {
    pub(crate) fn new(initial_controls_hidden: bool) -> Self {
        let settings = UserSettings::default();
        let workspace = EntriesWorkspaceState::default();
        Self {
            bulk_preview: None,
            bulk_busy: false,
            bulk_writing: false,
            bulk_generation: 0,
            bulk_revision: 0,
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
            controls_hidden: initial_controls_hidden,
            current_page: FIRST_PAGE_NUMBER,
            entries_loaded: false,
            status: "正在加载文章列表…".to_string(),
            status_tone: "info".to_string(),
            preferences_load: PreferencesLoadState::Pending,
            query_in_flight: false,
            requery_revision: 0,
        }
    }

    /// 构造这一屏对应的查询。
    ///
    /// 归档筛选放在查询里由存储层执行：页面层不再把已归档文章一并读回来再在内存里过滤。
    /// `now` 由调用方传入，便于测试固定时间。
    pub(crate) fn entry_query(
        &self,
        feed_id: Option<i64>,
        search_title: Option<String>,
        now: OffsetDateTime,
    ) -> EntryQuery {
        EntryQuery {
            feed_id,
            read_filter: self.read_filter,
            starred_filter: self.starred_filter,
            archive_filter: if self.show_archived {
                ArchiveFilter::All
            } else {
                ArchiveFilter::exclude_archived(now, self.archive_after_months)
            },
            feed_ids: if feed_id.is_some() {
                Vec::new()
            } else {
                map_selected_feed_urls_to_ids(&self.feeds, &self.selected_feed_urls)
            },
            search_title,
            limit: None,
        }
    }

    /// 归档时钟的采样触发条件（审计 A4）：查询条件变化、批量已读或手动刷新完成时重新采样；
    /// 状态提示、查询结果这类不改变查询条件的状态变化不触发。
    pub(crate) fn query_clock_trigger(
        &self,
        feed_id: Option<i64>,
        search_title: Option<String>,
        refresh_revision: u64,
    ) -> QueryClockTrigger {
        QueryClockTrigger {
            refresh_revision,
            bulk_revision: self.bulk_revision,
            feed_id,
            search_title,
            show_archived: self.show_archived,
            read_filter: self.read_filter,
            starred_filter: self.starred_filter,
            selected_feed_urls: self.selected_feed_urls.clone(),
            archive_after_months: self.archive_after_months,
        }
    }

    pub(crate) fn page_size(&self) -> usize {
        self.entries_page_size.max(1) as usize
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
    feeds
        .iter()
        .filter(|feed| selected.contains(feed.url.as_str()))
        .map(|feed| feed.id)
        .collect::<Vec<_>>()
}

#[cfg(test)]
mod tests {
    use super::{EntriesPageState, ReadFilter, StarredFilter};

    /// 查询条件变化时重新采样归档时钟；状态提示、查询结果等普通变化不采样（审计 A4）。
    #[test]
    fn archive_clock_resamples_on_query_changes_only() {
        let state = EntriesPageState::new(true);
        let base = state.query_clock_trigger(None, None, 0);

        let mut unrelated = state.clone();
        unrelated.status = "共 3 篇文章。".into();
        unrelated.entries_loaded = true;
        unrelated.current_page = 2;
        unrelated.controls_hidden = !unrelated.controls_hidden;
        assert_eq!(unrelated.query_clock_trigger(None, None, 0), base);

        let mut changed = Vec::new();
        for edit in [
            (|s: &mut EntriesPageState| s.read_filter = ReadFilter::UnreadOnly)
                as fn(&mut EntriesPageState),
            |s| s.starred_filter = StarredFilter::StarredOnly,
            |s| s.show_archived = !s.show_archived,
            |s| s.selected_feed_urls = vec!["https://example.com/feed.xml".into()],
            |s| s.archive_after_months += 1,
            |s| s.bulk_revision += 1,
        ] {
            let mut edited = state.clone();
            edit(&mut edited);
            changed.push(edited.query_clock_trigger(None, None, 0));
        }
        changed.push(state.query_clock_trigger(Some(1), None, 0));
        changed.push(state.query_clock_trigger(None, Some("中文".into()), 0));
        changed.push(state.query_clock_trigger(None, None, 1));
        for trigger in changed {
            assert_ne!(trigger, base);
        }
    }
}

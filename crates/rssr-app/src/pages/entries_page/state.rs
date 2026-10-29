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
    // the persisted values that could not be read. The failure's message and tone stay with
    // it (review C4B-A-R5-01).
    Unavailable {
        message: String,
        tone: &'static str,
    },
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
    /// 匹配的文章多于列表返回的（`MAX_LISTED_ENTRIES`）。
    pub(crate) entries_truncated: bool,
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
    /// 是否有列表查询在途（最新一代尚未落地）。
    pub(crate) query_in_flight: bool,
    /// 在途查询被写入作废时递增，页面据此重新发出列表查询（审计 A3）。
    pub(crate) requery_revision: u64,
    /// The browsing preferences the library holds, as far as the page knows: read at start, or
    /// saved by it. The page saves only what differs from them, so a read (at start or again)
    /// never writes (C4b item 2). `None` before the first read, and after a save whose outcome is
    /// unknown until a read tells again (review C4B-B-R2-04).
    pub(crate) stored_workspace: Option<EntriesWorkspaceState>,
    /// The status is a write's failure or unknown outcome: a list read does not replace it
    /// (C4b item 2: what is read again only updates what is shown).
    pub(crate) status_kept: bool,
    /// A read of the preferences and subscriptions (the start's or a re-read) is on its way.
    pub(crate) bootstrap_in_flight: bool,
    /// A save of the browsing preferences is on its way: saves go one at a time.
    pub(crate) workspace_saving: bool,
    /// The user's latest choice made while a save was on its way, saved after it (review
    /// C4B-B-05). Reads never set it.
    pub(crate) workspace_wanted: Option<EntriesWorkspaceState>,
    /// Filters the user chose while the preferences could not be read, kept shown when a later
    /// read recovers them. That read does not save them by itself: the save effect's next firing
    /// (the page becoming ready) skips them once; the user's changes after it are saved (reviews
    /// C4B-B-R2-06, C4B-B-R3-01).
    pub(crate) recovered_edits: Option<EntriesWorkspaceState>,
}

/// 见 [`EntriesPageState::query_clock_trigger`]。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QueryClockTrigger {
    library_revision: u64,
    reload_revision: u64,
    feed_id: Option<i64>,
    search_title: Option<String>,
    show_archived: bool,
    read_filter: ReadFilter,
    starred_filter: StarredFilter,
    selected_feed_urls: Vec<String>,
    archive_after_months: u32,
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
            entries_truncated: false,
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
            query_in_flight: false,
            requery_revision: 0,
            stored_workspace: None,
            status_kept: false,
            bootstrap_in_flight: false,
            workspace_saving: false,
            workspace_wanted: None,
            recovered_edits: None,
        }
    }

    /// The browsing preferences the page shows.
    pub(crate) fn workspace(&self) -> EntriesWorkspaceState {
        EntriesWorkspaceState {
            grouping_mode: grouping_mode_preference(self.grouping_mode),
            show_archived: self.show_archived,
            read_filter: self.read_filter,
            starred_filter: self.starred_filter,
            selected_feed_urls: self.selected_feed_urls.clone(),
        }
    }

    /// 构造不依赖时钟的查询范围。归档分界由页面 memo 在范围或刷新代次
    /// 变化时补入；选择、目录和状态文字重绘不能改变查询的时间参数。
    pub(crate) fn entry_query(
        &self,
        feed_id: Option<i64>,
        search_title: Option<String>,
        now: time::OffsetDateTime,
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
            feed_ids: if feed_id.is_some() || self.selected_feed_urls.is_empty() {
                None
            } else {
                // Chosen sources that are all gone match nothing, never everything.
                Some(map_selected_feed_urls_to_ids(&self.feeds, &self.selected_feed_urls))
            },
            search_title,
            limit: None,
        }
    }

    /// 归档时钟的采样触发条件（审计 A4）：查询条件变化、批量已读或只读刷新（含手动刷新完成）时
    /// 重新采样；状态提示、查询结果这类不改变查询条件的状态变化不触发。
    pub(crate) fn query_clock_trigger(
        &self,
        feed_id: Option<i64>,
        search_title: Option<String>,
        library_revision: u64,
    ) -> QueryClockTrigger {
        QueryClockTrigger {
            library_revision,
            reload_revision: self.reload_revision,
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
        let state = EntriesPageState::new();
        let base = state.query_clock_trigger(None, None, 0);

        let mut unrelated = state.clone();
        unrelated.status = "共 3 篇文章。".into();
        unrelated.entries_loaded = true;
        unrelated.current_page = 2;
        unrelated.selected_entry_ids = std::sync::Arc::new([1].into_iter().collect());
        assert_eq!(unrelated.query_clock_trigger(None, None, 0), base);

        let mut changed = Vec::new();
        for edit in [
            (|s: &mut EntriesPageState| s.read_filter = ReadFilter::UnreadOnly)
                as fn(&mut EntriesPageState),
            |s| s.starred_filter = StarredFilter::StarredOnly,
            |s| s.show_archived = !s.show_archived,
            |s| s.selected_feed_urls = vec!["https://example.com/feed.xml".into()],
            |s| s.archive_after_months += 1,
            |s| s.reload_revision += 1,
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

    /// No chosen sources means every subscription; chosen sources that are all gone match
    /// nothing, so a bulk mark-read cannot widen to every subscription (B2-D6).
    #[test]
    fn chosen_sources_that_are_gone_match_nothing() {
        let now = time::OffsetDateTime::UNIX_EPOCH;
        let mut state = EntriesPageState::new();
        assert_eq!(state.entry_query(None, None, now).feed_ids, None);

        state.feeds = std::sync::Arc::new(vec![rssr_domain::FeedSummary {
            id: 7,
            title: "Kept".into(),
            url: "https://example.com/kept.xml".into(),
            unread_count: 0,
            entry_count: 0,
            last_fetched_at: None,
            last_success_at: None,
            fetch_error: None,
        }]);
        state.selected_feed_urls = vec!["https://example.com/kept.xml".into()];
        assert_eq!(state.entry_query(None, None, now).feed_ids, Some(vec![7]));
        state.selected_feed_urls = vec!["https://example.com/gone.xml".into()];
        assert_eq!(state.entry_query(None, None, now).feed_ids, Some(Vec::new()));
        assert_eq!(
            state.entry_query(Some(7), None, now).feed_ids,
            None,
            "a feed page ignores them"
        );
    }
}

use std::{collections::BTreeSet, sync::Arc};

use crate::ui::AppShellState;
use rssr_domain::{EntrySummary, ReadFilter, StarredFilter};

use super::{
    browser_interactions::scroll_to_entry_group,
    groups::{
        EntryCardRef, EntryDirectoryMonth, EntryDirectorySource, EntryGroupNavItem,
        EntryMonthGroup, EntrySourceGroup,
    },
    intent::EntriesPageIntent,
    presenter::EntriesPagePresenter,
    session::EntriesPageSession,
    state::{EntriesPageState, EntryGroupingMode},
};

// snapshot/presenter 以 Arc 持有：facade 克隆必须保持 O(1)，防止再次出现
// 按卡片深拷贝全量文章列表导致的内存放大。
#[derive(Clone)]
pub(crate) struct EntriesPageFacade {
    ui: AppShellState,
    session: EntriesPageSession,
    snapshot: Arc<EntriesPageState>,
    presenter: Arc<EntriesPagePresenter>,
}

impl EntriesPageFacade {
    /// presenter 由调用方通过 `use_memo` 缓存后传入：它是 state 的纯函数，没必要每次重绘
    /// 都重建一遍分组树。
    pub(crate) fn new(
        ui: AppShellState,
        session: EntriesPageSession,
        snapshot: Arc<EntriesPageState>,
        presenter: Arc<EntriesPagePresenter>,
    ) -> Self {
        Self { ui, session, snapshot, presenter }
    }

    pub(crate) fn entry_search(&self) -> String {
        self.ui.entry_search()
    }

    pub(crate) fn set_entry_search(&self, value: String) {
        self.ui.set_entry_search(value);
    }

    pub(crate) fn controls_hidden(&self) -> bool {
        self.snapshot.controls_hidden
    }

    pub(crate) fn grouping_mode(&self) -> EntryGroupingMode {
        self.snapshot.grouping_mode
    }

    pub(crate) fn show_archived(&self) -> bool {
        self.snapshot.show_archived
    }

    pub(crate) fn archive_after_months(&self) -> u32 {
        self.snapshot.archive_after_months
    }

    pub(crate) fn read_filter(&self) -> ReadFilter {
        self.snapshot.read_filter
    }

    pub(crate) fn starred_filter(&self) -> StarredFilter {
        self.snapshot.starred_filter
    }

    pub(crate) fn selected_feed_urls(&self) -> &[String] {
        &self.snapshot.selected_feed_urls
    }

    pub(crate) fn status_message(&self) -> &str {
        &self.snapshot.status
    }

    pub(crate) fn status_tone(&self) -> &str {
        &self.snapshot.status_tone
    }

    pub(crate) fn has_status_message(&self) -> bool {
        !self.status_message().is_empty()
    }

    pub(crate) fn entries_is_empty(&self) -> bool {
        self.snapshot.entries.is_empty()
    }

    pub(crate) fn visible_entries_is_empty(&self) -> bool {
        self.presenter.visible_entries_len == 0
    }

    pub(crate) fn visible_entries_len(&self) -> usize {
        self.presenter.visible_entries_len
    }

    /// 按分组树里的引用解析条目。卡片的 `title` / `is_read` / `is_starred` 一律走这里
    /// 从最新 snapshot 取，因此分组树不必为标记切换重建。
    ///
    /// 正常情况下 `card.index` 直接命中：投影相等（presenter memo 因此复用旧树）就意味着
    /// 条目数量与每个下标处的条目都没变，见 `GroupingEntries` 的 `PartialEq`。
    ///
    /// 但下标只是**快路径**，`card.id` 才是判据。分组树取自 memo 缓存，而它要跟上一次状态写入
    /// 需要两跳，两跳的保证强度不同（已核对 dioxus 0.7.9 源码）：
    ///
    /// 1. **状态信号 → 投影 memo**：靠调度器次序。写入只把投影 memo 的 `dirty` 置真并唤醒它的
    ///    重算任务，`dirty` 不会自己传到 presenter memo 上。`dioxus-core/src/scheduler.rs:203-222`
    ///    在脏 scope 与脏 task 的 `ScopeOrder` 相等时让 `Work::PollTask` 胜出，因此投影 memo
    ///    先重算、写回自己的信号，presenter memo 才被标脏。**这一跳依赖调度器内部次序。**
    /// 2. **投影 memo → presenter memo**：无条件成立。`dioxus-signals/src/memo.rs:166-196`
    ///    在读取时 `swap` 掉 `dirty` 并就地同步重算，所以第 1 跳标脏后，这里读到的一定是新值。
    ///    注意反过来不成立：**没被标脏的 memo 直接返回缓存值，不会去看上游**（同处 else 分支），
    ///    所以第 2 跳救不了第 1 跳。
    ///
    /// 于是第 1 跳一旦在某个路径上不成立，只按下标解析就会把**另一篇文章**渲染到这个位置上
    /// （连 `key` 也会跟着错）。校验 id 之后，最坏情况退化成「这张卡片这一帧没渲染」，
    /// 下一帧投影追上后自然恢复。这就是这里不省这次整数比较的原因。
    pub(crate) fn entry_at(&self, card: EntryCardRef) -> Option<Arc<EntrySummary>> {
        let entries = &self.snapshot.entries;
        if let Some(entry) = entries.get(card.index)
            && entry.id == card.id
        {
            return Some(Arc::clone(entry));
        }
        // 兜底：O(N) 但极罕见；条目已被移出列表时返回 None。
        entries.iter().find(|entry| entry.id == card.id).map(Arc::clone)
    }

    pub(crate) fn session(&self) -> EntriesPageSession {
        self.session
    }

    pub(crate) fn archived_entry_count(&self) -> usize {
        self.presenter.archived_count
    }

    pub(crate) fn page_size(&self) -> usize {
        self.presenter.page_size
    }

    pub(crate) fn current_page(&self) -> u32 {
        self.presenter.current_page
    }

    pub(crate) fn total_pages(&self) -> u32 {
        self.presenter.total_pages
    }

    pub(crate) fn page_start(&self) -> usize {
        self.presenter.page_start
    }

    pub(crate) fn page_end(&self) -> usize {
        self.presenter.page_end
    }

    pub(crate) fn can_go_previous_page(&self) -> bool {
        self.current_page() > 1
    }

    pub(crate) fn can_go_next_page(&self) -> bool {
        self.current_page() < self.total_pages()
    }

    pub(crate) fn active_directory_anchor(&self) -> Option<&str> {
        self.presenter
            .active_directory_anchor
            .as_deref()
            .or(self.presenter.active_group_anchor.as_deref())
    }

    pub(crate) fn archived_entries_message(&self) -> String {
        format!(
            "当前已自动归档 {} 篇较旧文章，可勾选“显示已归档文章”查看。",
            self.archived_entry_count()
        )
    }

    pub(crate) fn source_filter_options(&self) -> &[(i64, String, String)] {
        &self.presenter.source_filter_options
    }

    pub(crate) fn group_nav_items(&self) -> &[EntryGroupNavItem] {
        &self.presenter.group_nav_items
    }

    pub(crate) fn time_grouped_entries(&self) -> &[EntryMonthGroup] {
        &self.presenter.time_grouped_entries
    }

    pub(crate) fn source_grouped_entries(&self) -> &[EntrySourceGroup] {
        &self.presenter.source_grouped_entries
    }

    pub(crate) fn directory_months(&self) -> &[EntryDirectoryMonth] {
        &self.presenter.directory_months
    }

    pub(crate) fn directory_sources(&self) -> &[EntryDirectorySource] {
        &self.presenter.directory_sources
    }

    pub(crate) fn default_expanded_directory_sections(&self) -> &BTreeSet<String> {
        &self.presenter.default_expanded_directory_sections
    }

    pub(crate) fn empty_entries_message(&self) -> String {
        if self.session.feed_id().is_some() {
            "这个订阅下还没有可显示的文章，先尝试刷新该 feed。".to_string()
        } else {
            "没有可显示的文章，先去订阅页添加并刷新 feed。".to_string()
        }
    }

    pub(crate) fn archived_entries_state_message(&self) -> &'static str {
        "当前结果中的文章都已被自动归档，勾选“显示已归档文章”即可查看。"
    }

    pub(crate) fn set_controls_hidden(&self, hidden: bool) {
        self.session.dispatch(EntriesPageIntent::SetControlsHidden(hidden));
    }

    pub(crate) fn set_grouping_mode(&self, mode: EntryGroupingMode) {
        self.session.dispatch(EntriesPageIntent::SetGroupingMode(mode));
    }

    pub(crate) fn set_show_archived(&self, value: bool) {
        self.session.dispatch(EntriesPageIntent::SetShowArchived(value));
    }

    pub(crate) fn set_read_filter(&self, value: ReadFilter) {
        self.session.dispatch(EntriesPageIntent::SetReadFilter(value));
    }

    pub(crate) fn set_starred_filter(&self, value: StarredFilter) {
        self.session.dispatch(EntriesPageIntent::SetStarredFilter(value));
    }

    pub(crate) fn set_selected_feed_urls(&self, value: Vec<String>) {
        self.session.dispatch(EntriesPageIntent::SetSelectedFeedUrls(value));
    }

    pub(crate) fn go_to_previous_page(&self) {
        self.session.dispatch(EntriesPageIntent::GoToPreviousPage);
    }

    pub(crate) fn go_to_next_page(&self) {
        self.session.dispatch(EntriesPageIntent::GoToNextPage);
    }

    pub(crate) fn navigate_to_directory_target(&self, target_page: u32, anchor_id: String) {
        self.session.dispatch(EntriesPageIntent::SetCurrentPage(target_page));
        scroll_to_entry_group(&anchor_id);
    }
}

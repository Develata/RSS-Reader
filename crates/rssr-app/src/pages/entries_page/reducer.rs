use std::sync::Arc;

use dioxus::prelude::*;
use rssr_domain::{ReadFilter, StarredFilter};

use super::{
    intent::EntriesPageIntent, state::EntriesPageState, state::FIRST_PAGE_NUMBER,
    state::PreferencesLoadState, state::entry_grouping_mode_from_preference,
};

pub(crate) fn dispatch_entries_page_intent(
    mut state: Signal<EntriesPageState>,
    intent: EntriesPageIntent,
) {
    state.with_mut(|state| reduce_entries_page_intent(state, intent));
}

pub(crate) fn reduce_entries_page_intent(state: &mut EntriesPageState, intent: EntriesPageIntent) {
    let previous_page = state.current_page;
    if matches!(&intent, EntriesPageIntent::SetGroupingMode(mode) if *mode == state.grouping_mode) {
        return;
    }
    if matches!(
        &intent,
        EntriesPageIntent::ClearSelection
            | EntriesPageIntent::ClearFilters
            | EntriesPageIntent::SetGroupingMode(_)
            | EntriesPageIntent::SetShowArchived(_)
            | EntriesPageIntent::SetReadFilter(_)
            | EntriesPageIntent::SetStarredFilter(_)
            | EntriesPageIntent::SetSelectedFeedUrls(_)
            | EntriesPageIntent::ApplyLoadedWorkspaceState(_)
    ) {
        Arc::make_mut(&mut state.selected_entry_ids).clear();
    }
    match intent {
        EntriesPageIntent::EntryWriteStarted(id) => {
            Arc::make_mut(&mut state.pending_entry_writes).insert(id);
        }
        EntriesPageIntent::EntryWriteFinished(id) => {
            Arc::make_mut(&mut state.pending_entry_writes).remove(&id);
        }
        EntriesPageIntent::ReloadEntries => {
            state.reload_revision = state.reload_revision.wrapping_add(1);
        }
        EntriesPageIntent::ClearSelection => {}
        EntriesPageIntent::ToggleSelected(id) => {
            if !state.bulk_busy && state.current_page_entries().iter().any(|entry| entry.id == id) {
                let selected = Arc::make_mut(&mut state.selected_entry_ids);
                if !selected.remove(&id) {
                    selected.insert(id);
                }
            }
        }
        EntriesPageIntent::TogglePageSelection => {
            if !state.bulk_busy {
                let ids = state
                    .current_page_entries()
                    .iter()
                    .map(|entry| entry.id)
                    .collect::<std::collections::BTreeSet<_>>();
                state.selected_entry_ids = Arc::new(if ids == *state.selected_entry_ids {
                    Default::default()
                } else {
                    ids
                });
            }
        }
        EntriesPageIntent::SelectionApplied { ids, action, count } => {
            state.bulk_busy = false;
            state.bulk_writing = false;
            state.reload_revision = state.reload_revision.wrapping_add(1);
            state.flags_revision = state.flags_revision.wrapping_add(1);
            let selected = Arc::make_mut(&mut state.selected_entry_ids);
            for id in ids {
                selected.remove(&id);
            }
            let verb = match action {
                rssr_domain::EntryBatchAction::MarkRead => "标为已读",
                rssr_domain::EntryBatchAction::Archive => "归档",
            };
            let message = format!("已{verb} {count} 篇所选文章。");
            state.bulk_notice = Some(message.clone());
            state.status = message;
            state.status_tone = "success".into();
            state.status_kept = false;
        }
        EntriesPageIntent::BeginBulk { writing } => {
            state.bulk_generation = state.bulk_generation.wrapping_add(1);
            state.bulk_busy = true;
            state.bulk_writing = writing;
        }
        EntriesPageIntent::CancelBulk => {
            state.status_kept = false;
            state.bulk_notice = None;
            if !state.bulk_writing {
                state.bulk_generation = state.bulk_generation.wrapping_add(1);
                state.bulk_busy = false;
            }
        }
        EntriesPageIntent::BulkFailed(failure) => {
            state.bulk_busy = false;
            state.bulk_writing = false;
            if failure.is_unknown() {
                // The confirmed write may have applied: the preview no longer says what is
                // unread (C4b item 1).
                Arc::make_mut(&mut state.selected_entry_ids).clear();
            }
            state.status = failure.message();
            state.status_tone = failure.tone().into();
            state.status_kept = true;
            state.bulk_notice = None;
        }

        EntriesPageIntent::ApplyLoadedSettings(settings) => {
            state.archive_after_months = settings.archive_after_months;
            state.entries_page_size = settings.entries_page_size.max(1);
            clamp_current_page(state);
        }
        EntriesPageIntent::ApplyLoadedWorkspaceState(workspace) => {
            state.read_filter = workspace.read_filter;
            state.starred_filter = workspace.starred_filter;
            state.selected_feed_urls = workspace.selected_feed_urls.clone();
            state.show_archived = workspace.show_archived;
            state.grouping_mode = entry_grouping_mode_from_preference(workspace.grouping_mode);
            state.current_page = FIRST_PAGE_NUMBER;
            state.stored_workspace = Some(workspace);
        }
        EntriesPageIntent::PreferencesReread { settings, workspace } => {
            reread_preferences(state, settings, workspace);
        }
        EntriesPageIntent::PreferencesRereadFailed(failure) => {
            let intent = if matches!(state.preferences_load, PreferencesLoadState::Pending) {
                // It replaced the start's read: the list must not wait for it any longer.
                EntriesPageIntent::PreferencesUnavailable(failure)
            } else {
                EntriesPageIntent::ReadFailed(failure)
            };
            reduce_entries_page_intent(state, intent);
        }
        EntriesPageIntent::WorkspaceSaved(workspace) => state.stored_workspace = Some(workspace),
        EntriesPageIntent::WorkspaceSaveStarted => {
            state.workspace_saving = true;
            state.workspace_wanted = None;
            state.recovered_edits = None;
        }
        EntriesPageIntent::WorkspaceSaveUnknown => state.stored_workspace = None,
        EntriesPageIntent::RecoveredEditsHandled => state.recovered_edits = None,
        EntriesPageIntent::WorkspaceSaveEnded => {
            state.workspace_saving = false;
            state.workspace_wanted = None;
        }
        EntriesPageIntent::WorkspaceWanted(workspace) => state.workspace_wanted = Some(workspace),
        EntriesPageIntent::BootstrapStarted => state.bootstrap_in_flight = true,
        EntriesPageIntent::BootstrapFinished => state.bootstrap_in_flight = false,
        EntriesPageIntent::PreferencesLoaded => {
            if matches!(state.preferences_load, PreferencesLoadState::Unavailable { .. }) {
                state.status = format!("共 {} 篇文章。", state.entries.len());
                state.status_tone = "info".to_string();
            }
            state.preferences_load = PreferencesLoadState::Loaded;
        }
        EntriesPageIntent::PreferencesUnavailable(failure) => {
            let message = failure.message();
            state.status = message.clone();
            state.status_tone = failure.tone().to_string();
            state.preferences_load =
                PreferencesLoadState::Unavailable { message, tone: failure.tone() };
            state.status_kept = false;
        }
        EntriesPageIntent::SetFeeds(feeds) => state.feeds = Arc::new(feeds),
        EntriesPageIntent::EntriesQueryStarted => state.query_in_flight = true,
        EntriesPageIntent::EntriesQueryFinished => state.query_in_flight = false,
        EntriesPageIntent::SetEntries { entries, archived_count, truncated } => {
            state.entries_loaded = true;
            state.entries = Arc::new(entries.into_iter().map(Arc::new).collect());
            state.archived_count = archived_count;
            state.entries_truncated = truncated;
            let count = state.entries.len();
            if state.status_kept {
                // A later read cannot turn an uncertain write into a success receipt.
            } else if let PreferencesLoadState::Unavailable { message, tone } =
                &state.preferences_load
            {
                state.status = format!("{message}；文章偏好暂不保存，共 {count} 篇文章。");
                state.status_tone = tone.to_string();
            } else {
                state.status = format!("共 {count} 篇文章。");
                state.status_tone = "info".to_string();
            }
            // The bulk mark-read's own notice, unless a later status replaced it (review
            // C4B-B-04).
            if !state.status_kept
                && let Some(message) = state.bulk_notice.take()
            {
                state.status = message;
                state.status_tone = "success".into();
            }
            clamp_current_page(state);
        }
        EntriesPageIntent::PatchEntryFlags { entry_id, is_read, is_starred } => {
            state.flags_revision = state.flags_revision.wrapping_add(1);
            let listed = state.entries.iter().any(|entry| entry.id == entry_id);
            apply_flag_patch(state, entry_id, is_read, is_starred);
            if state.query_in_flight || !listed {
                state.requery_revision = state.requery_revision.wrapping_add(1);
            }
        }
        EntriesPageIntent::SetStatus { message, tone } => {
            state.status = message;
            state.status_tone = tone;
            state.status_kept = false;
            state.bulk_notice = None;
        }
        EntriesPageIntent::Failed(failure) => {
            state.status = failure.message();
            state.status_tone = failure.tone().to_string();
            state.status_kept = true;
            state.bulk_notice = None;
        }
        EntriesPageIntent::ReadFailed(failure) => {
            state.status = failure.message();
            state.status_tone = failure.tone().to_string();
            state.status_kept = false;
        }
        EntriesPageIntent::SetGroupingMode(grouping_mode) => {
            state.grouping_mode = grouping_mode;
            state.current_page = FIRST_PAGE_NUMBER;
        }
        EntriesPageIntent::SetShowArchived(show_archived) => {
            state.show_archived = show_archived;
            state.current_page = FIRST_PAGE_NUMBER;
        }
        EntriesPageIntent::SetReadFilter(read_filter) => {
            state.read_filter = read_filter;
            state.current_page = FIRST_PAGE_NUMBER;
        }
        EntriesPageIntent::SetStarredFilter(starred_filter) => {
            state.starred_filter = starred_filter;
            state.current_page = FIRST_PAGE_NUMBER;
        }
        EntriesPageIntent::SetSelectedFeedUrls(selected_feed_urls) => {
            state.selected_feed_urls = selected_feed_urls;
            state.current_page = FIRST_PAGE_NUMBER;
        }
        EntriesPageIntent::ClearFilters => {
            state.read_filter = ReadFilter::All;
            state.starred_filter = StarredFilter::All;
            state.selected_feed_urls.clear();
            state.current_page = FIRST_PAGE_NUMBER;
        }
        EntriesPageIntent::SetCurrentPage(page) => {
            state.current_page = page.max(FIRST_PAGE_NUMBER);
            clamp_current_page(state);
        }
        EntriesPageIntent::GoToNextPage => {
            state.current_page = state.current_page.saturating_add(1);
            clamp_current_page(state);
        }
        EntriesPageIntent::GoToPreviousPage => {
            state.current_page = state.current_page.saturating_sub(1).max(FIRST_PAGE_NUMBER);
        }
    }
    if previous_page != state.current_page {
        Arc::make_mut(&mut state.selected_entry_ids).clear();
    } else if !state.selected_entry_ids.is_empty() {
        // Refresh/flag updates may remove rows. Never leave invisible articles selected.
        let start = ((state.current_page.saturating_sub(1) as usize) * state.page_size())
            .min(state.entries.len());
        let end = (start + state.page_size()).min(state.entries.len());
        let entries = &state.entries[start..end];
        Arc::make_mut(&mut state.selected_entry_ids)
            .retain(|id| entries.iter().any(|entry| entry.id == *id));
    }
}

/// The read-only refresh's preferences (C4b item 2). The settings (page size, archive months)
/// always apply. The browsing preferences apply only where the page shows what the library held
/// and the user has nothing pending (no save on its way, no choice waiting for one, no recovered
/// filters; review C4B-B-R2-03), without going back to the first page; the page then saves
/// nothing, since they are the stored ones. A read that replaced the start's, or one after the
/// start's failed while the page still shows the fallback, counts as the start's read. One after
/// the start's failed while the user chose filters makes the page ready, keeps those filters and
/// does not save them by itself (review C4B-B-R2-06). One after a save's unknown outcome learns
/// what the library holds, without applying it (review C4B-B-R2-04).
fn reread_preferences(
    state: &mut EntriesPageState,
    settings: rssr_domain::UserSettings,
    workspace: rssr_domain::EntriesWorkspaceState,
) {
    let first = match &state.preferences_load {
        PreferencesLoadState::Pending => true,
        PreferencesLoadState::Unavailable { .. } => {
            state.workspace() == rssr_domain::EntriesWorkspaceState::default()
        }
        PreferencesLoadState::Loaded => false,
    };
    reduce_entries_page_intent(state, EntriesPageIntent::ApplyLoadedSettings(settings));
    if first {
        reduce_entries_page_intent(state, EntriesPageIntent::ApplyLoadedWorkspaceState(workspace));
        reduce_entries_page_intent(state, EntriesPageIntent::PreferencesLoaded);
        return;
    }
    if !state.preferences_load.is_loaded() {
        state.recovered_edits = Some(state.workspace());
        state.stored_workspace = Some(workspace);
        reduce_entries_page_intent(state, EntriesPageIntent::PreferencesLoaded);
        return;
    }
    let pending = state.workspace_saving
        || state.workspace_wanted.is_some()
        || state.recovered_edits.is_some();
    if state.stored_workspace.is_none() {
        if !pending {
            state.stored_workspace = Some(workspace);
        }
        return;
    }
    let unsaved = pending || state.stored_workspace.as_ref() != Some(&state.workspace());
    if !unsaved {
        state.read_filter = workspace.read_filter;
        state.starred_filter = workspace.starred_filter;
        state.selected_feed_urls = workspace.selected_feed_urls.clone();
        state.show_archived = workspace.show_archived;
        state.grouping_mode = entry_grouping_mode_from_preference(workspace.grouping_mode);
        state.stored_workspace = Some(workspace);
        clamp_current_page(state);
    }
}

fn apply_flag_patch(
    state: &mut EntriesPageState,
    entry_id: i64,
    is_read: Option<bool>,
    is_starred: Option<bool>,
) {
    let entries = Arc::make_mut(&mut state.entries);
    if let Some(entry) = entries.iter_mut().find(|entry| entry.id == entry_id) {
        // 只有被点的那一条需要真正写入，其余条目继续共享原有 Arc。
        let entry = Arc::make_mut(entry);
        if let Some(is_read) = is_read {
            entry.is_read = is_read;
        }
        if let Some(is_starred) = is_starred {
            entry.is_starred = is_starred;
        }
    }
    let read_filter = state.read_filter;
    let starred_filter = state.starred_filter;
    entries.retain(|entry| matches_current_filters(read_filter, starred_filter, entry));
    clamp_current_page(state);
}

fn matches_current_filters(
    read_filter: ReadFilter,
    starred_filter: StarredFilter,
    entry: &rssr_domain::EntrySummary,
) -> bool {
    let matches_read = match read_filter {
        ReadFilter::All => true,
        ReadFilter::UnreadOnly => !entry.is_read,
        ReadFilter::ReadOnly => entry.is_read,
    };
    let matches_starred = match starred_filter {
        StarredFilter::All => true,
        StarredFilter::StarredOnly => entry.is_starred,
        StarredFilter::UnstarredOnly => !entry.is_starred,
    };

    matches_read && matches_starred
}

fn clamp_current_page(state: &mut EntriesPageState) {
    let page_size = state.page_size().max(1);
    let total_entries = state.entries.len();
    let total_pages = if total_entries == 0 {
        FIRST_PAGE_NUMBER
    } else {
        ((total_entries - 1) / page_size) as u32 + 1
    };
    state.current_page = state.current_page.clamp(FIRST_PAGE_NUMBER, total_pages);
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rssr_domain::{EntrySummary, ReadFilter, StarredFilter};

    use super::reduce_entries_page_intent;
    use crate::pages::entries_page::{
        intent::EntriesPageIntent,
        state::{EntriesPageState, FIRST_PAGE_NUMBER},
    };

    fn entry(id: i64, is_read: bool) -> EntrySummary {
        EntrySummary {
            id,
            feed_id: 1,
            title: format!("Entry {id}"),
            feed_title: "Feed".to_string(),
            published_at: None,
            is_read,
            is_starred: false,
        }
    }

    #[test]
    fn selection_is_page_scoped_and_same_page_directory_navigation_preserves_it() {
        let mut state = EntriesPageState::new();
        state.entries_page_size = 2;
        state.entries = (1..=4).map(|id| Arc::new(entry(id, false))).collect::<Vec<_>>().into();
        reduce_entries_page_intent(&mut state, EntriesPageIntent::ToggleSelected(3));
        assert!(state.selected_entry_ids.is_empty(), "cannot select an invisible row");
        reduce_entries_page_intent(&mut state, EntriesPageIntent::TogglePageSelection);
        assert_eq!(state.selected_entry_ids.iter().copied().collect::<Vec<_>>(), vec![1, 2]);
        reduce_entries_page_intent(&mut state, EntriesPageIntent::SetCurrentPage(1));
        reduce_entries_page_intent(
            &mut state,
            EntriesPageIntent::PatchEntryFlags {
                entry_id: 1,
                is_read: Some(true),
                is_starred: None,
            },
        );
        assert_eq!(
            state.selected_entry_ids.len(),
            2,
            "same page jump and metadata updates preserve selection"
        );
        reduce_entries_page_intent(&mut state, EntriesPageIntent::SetCurrentPage(2));
        assert!(state.selected_entry_ids.is_empty());
        reduce_entries_page_intent(&mut state, EntriesPageIntent::TogglePageSelection);
        assert_eq!(state.selected_entry_ids.iter().copied().collect::<Vec<_>>(), vec![3, 4]);
        reduce_entries_page_intent(
            &mut state,
            EntriesPageIntent::SetReadFilter(ReadFilter::UnreadOnly),
        );
        assert!(state.selected_entry_ids.is_empty());
    }

    #[test]
    fn selection_is_reconciled_when_a_selected_row_leaves_the_filter() {
        let mut state = EntriesPageState::new();
        state.read_filter = ReadFilter::UnreadOnly;
        state.entries = vec![Arc::new(entry(1, false)), Arc::new(entry(2, false))].into();
        reduce_entries_page_intent(&mut state, EntriesPageIntent::TogglePageSelection);
        reduce_entries_page_intent(
            &mut state,
            EntriesPageIntent::PatchEntryFlags {
                entry_id: 1,
                is_read: Some(true),
                is_starred: None,
            },
        );
        assert_eq!(state.selected_entry_ids.iter().copied().collect::<Vec<_>>(), vec![2]);
        reduce_entries_page_intent(&mut state, EntriesPageIntent::BeginBulk { writing: true });
        reduce_entries_page_intent(&mut state, EntriesPageIntent::ToggleSelected(2));
        assert_eq!(
            state.selected_entry_ids.len(),
            1,
            "an in-flight batch freezes selection controls"
        );
    }

    #[test]
    fn patch_entry_flags_removes_entry_that_no_longer_matches_filter() {
        let mut state = EntriesPageState::new();
        state.read_filter = ReadFilter::UnreadOnly;
        state.entries = vec![Arc::new(entry(1, false))].into();

        reduce_entries_page_intent(
            &mut state,
            EntriesPageIntent::PatchEntryFlags {
                entry_id: 1,
                is_read: Some(true),
                is_starred: None,
            },
        );

        assert!(state.entries.is_empty());
    }

    #[test]
    fn set_entries_clamps_current_page() {
        let mut state = EntriesPageState::new();
        state.current_page = 9;

        reduce_entries_page_intent(
            &mut state,
            EntriesPageIntent::SetEntries {
                entries: vec![entry(1, false)],
                archived_count: 0,
                truncated: false,
            },
        );

        assert_eq!(state.current_page, FIRST_PAGE_NUMBER);
    }

    #[test]
    fn changing_filters_resets_current_page() {
        let mut state = EntriesPageState::new();
        state.current_page = 4;

        reduce_entries_page_intent(
            &mut state,
            EntriesPageIntent::SetStarredFilter(rssr_domain::StarredFilter::StarredOnly),
        );

        assert_eq!(state.current_page, FIRST_PAGE_NUMBER);
    }

    #[test]
    fn changing_page_updates_current_page() {
        let mut state = EntriesPageState::new();
        state.entries_page_size = 1;
        state.entries = vec![Arc::new(entry(1, false)), Arc::new(entry(2, false))].into();

        reduce_entries_page_intent(&mut state, EntriesPageIntent::SetCurrentPage(2));

        assert_eq!(state.current_page, 2);
    }

    #[test]
    fn pagination_clamps_and_preserves_filters_and_source_selection() {
        let mut state = EntriesPageState::new();
        state.entries_page_size = 1;
        state.entries = vec![Arc::new(entry(1, false)), Arc::new(entry(2, false))].into();
        state.selected_feed_urls = vec!["https://example.com/中文 feed".into()];
        state.read_filter = ReadFilter::UnreadOnly;
        let workspace = state.clone();
        reduce_entries_page_intent(&mut state, EntriesPageIntent::GoToPreviousPage);
        assert_eq!(state.current_page, 1);
        for _ in 0..3 {
            reduce_entries_page_intent(&mut state, EntriesPageIntent::GoToNextPage);
        }
        assert_eq!(state.current_page, 2);
        assert_eq!(state.read_filter, workspace.read_filter);
        assert_eq!(state.selected_feed_urls, workspace.selected_feed_urls);
        assert_eq!(state.grouping_mode, workspace.grouping_mode);
    }

    #[test]
    fn flag_updates_preserve_previous_shared_snapshot() {
        let mut state = EntriesPageState::new();
        state.entries = vec![Arc::new(entry(1, false)), Arc::new(entry(2, false))].into();
        let snapshot = state.clone();
        assert!(Arc::ptr_eq(&state.entries, &snapshot.entries));
        reduce_entries_page_intent(
            &mut state,
            EntriesPageIntent::PatchEntryFlags {
                entry_id: 1,
                is_read: Some(true),
                is_starred: None,
            },
        );
        assert!(!snapshot.entries[0].is_read);
        assert!(state.entries[0].is_read);
        assert!(Arc::ptr_eq(&state.entries[1], &snapshot.entries[1]));
    }

    fn patch(entry_id: i64, is_read: Option<bool>, is_starred: Option<bool>) -> EntriesPageIntent {
        EntriesPageIntent::PatchEntryFlags { entry_id, is_read, is_starred }
    }

    /// 查询在途时的成功写入要求重查；没有在途查询时只修补列表，不排队（审计 A3）。
    #[test]
    fn writes_during_an_in_flight_query_request_a_requery() {
        let mut state = EntriesPageState::new();
        state.entries = vec![Arc::new(entry(1, false)), Arc::new(entry(2, false))].into();

        for _ in 0..3 {
            reduce_entries_page_intent(&mut state, patch(1, None, Some(true)));
            reduce_entries_page_intent(&mut state, patch(1, None, Some(false)));
        }
        assert_eq!(state.requery_revision, 0, "no query in flight, nothing to redo");

        reduce_entries_page_intent(&mut state, EntriesPageIntent::EntriesQueryStarted);
        reduce_entries_page_intent(&mut state, patch(2, Some(true), None));
        assert_eq!(state.requery_revision, 1);
        assert!(state.entries.iter().any(|e| e.id == 2 && e.is_read), "the list is patched now");

        reduce_entries_page_intent(&mut state, EntriesPageIntent::EntriesQueryFinished);
        reduce_entries_page_intent(&mut state, patch(1, Some(true), None));
        assert_eq!(state.requery_revision, 1);
        assert!(!state.query_in_flight);
    }

    /// 另一种完成顺序：点了收藏、随即切到“只看收藏”，查询先读到旧存储、返回不含这一条的
    /// 列表并结束，收藏写入随后才成功。取消收藏同理会让附带的归档计数过时。写入的条目不在
    /// 列表里就重查；在列表里的照常就地修补（独立评审）。
    #[test]
    fn a_write_that_lands_after_the_query_returned_requests_a_requery() {
        let mut state = EntriesPageState::new();
        state.starred_filter = StarredFilter::StarredOnly;
        reduce_entries_page_intent(&mut state, EntriesPageIntent::EntriesQueryStarted);
        reduce_entries_page_intent(
            &mut state,
            EntriesPageIntent::SetEntries {
                entries: Vec::new(),
                archived_count: 1,
                truncated: false,
            },
        );
        reduce_entries_page_intent(&mut state, EntriesPageIntent::EntriesQueryFinished);

        reduce_entries_page_intent(&mut state, patch(1, None, Some(true)));
        assert_eq!(state.requery_revision, 1, "the starred entry must be queried back in");
        reduce_entries_page_intent(&mut state, patch(2, None, Some(false)));
        assert_eq!(state.requery_revision, 2, "the archived count may have changed");

        state.entries = vec![Arc::new(entry(3, false))].into();
        reduce_entries_page_intent(&mut state, patch(3, Some(true), None));
        assert_eq!(state.requery_revision, 2, "a listed entry is patched in place");
    }

    fn workspace(read_filter: ReadFilter) -> rssr_domain::EntriesWorkspaceState {
        rssr_domain::EntriesWorkspaceState { read_filter, ..Default::default() }
    }

    fn settings(entries_page_size: u32) -> rssr_domain::UserSettings {
        rssr_domain::UserSettings { entries_page_size, ..Default::default() }
    }

    /// A page whose preferences were read at start: `stored` filters, two entries a page.
    fn loaded_page(stored: ReadFilter) -> EntriesPageState {
        let mut state = EntriesPageState::new();
        reduce_entries_page_intent(&mut state, EntriesPageIntent::ApplyLoadedSettings(settings(2)));
        reduce_entries_page_intent(
            &mut state,
            EntriesPageIntent::ApplyLoadedWorkspaceState(workspace(stored)),
        );
        reduce_entries_page_intent(&mut state, EntriesPageIntent::PreferencesLoaded);
        state.entries = (1..=6).map(|id| Arc::new(entry(id, false))).collect::<Vec<_>>().into();
        state.current_page = 2;
        state
    }

    fn reread(state: &mut EntriesPageState, read_filter: ReadFilter, page_size: u32) {
        reduce_entries_page_intent(
            state,
            EntriesPageIntent::PreferencesReread {
                settings: settings(page_size),
                workspace: workspace(read_filter),
            },
        );
    }

    /// The read-only refresh applies the settings, and preferences changed elsewhere where the
    /// page has none of its own unsaved, without going back to the first page; they are then the
    /// stored ones, so the page saves nothing (C4b item 2).
    #[test]
    fn a_reread_applies_settings_and_preferences_the_page_has_not_changed() {
        let mut state = loaded_page(ReadFilter::All);
        reread(&mut state, ReadFilter::UnreadOnly, 3);
        assert_eq!(state.entries_page_size, 3);
        assert_eq!(state.read_filter, ReadFilter::UnreadOnly);
        assert_eq!(state.stored_workspace, Some(state.workspace()));
        assert_eq!(state.current_page, 2, "the page stays");
        assert!(state.preferences_load.is_loaded());
    }

    /// Preferences the user changed and that are not saved yet stay; the settings still apply.
    #[test]
    fn a_reread_keeps_preferences_the_page_has_not_saved() {
        let mut state = loaded_page(ReadFilter::All);
        reduce_entries_page_intent(
            &mut state,
            EntriesPageIntent::SetReadFilter(ReadFilter::ReadOnly),
        );
        reread(&mut state, ReadFilter::UnreadOnly, 3);
        assert_eq!(state.read_filter, ReadFilter::ReadOnly);
        assert_eq!(state.stored_workspace, Some(workspace(ReadFilter::All)));
        assert_eq!(state.entries_page_size, 3);
    }

    /// A read-only refresh that superseded the start's read is the start's read; one after the
    /// start's failed is too while the page shows the fallback, not once the user chose filters.
    #[test]
    fn a_reread_completes_a_start_that_did_not_read_the_preferences() {
        let mut pending = EntriesPageState::new();
        reread(&mut pending, ReadFilter::UnreadOnly, 3);
        assert!(pending.preferences_load.is_loaded());
        assert_eq!(pending.read_filter, ReadFilter::UnreadOnly);
        assert_eq!(pending.stored_workspace, Some(workspace(ReadFilter::UnreadOnly)));

        let unavailable = |state: &mut EntriesPageState| {
            reduce_entries_page_intent(
                state,
                EntriesPageIntent::PreferencesUnavailable(crate::ui::failure::plain("读不到")),
            );
        };
        let mut fallback = EntriesPageState::new();
        unavailable(&mut fallback);
        reread(&mut fallback, ReadFilter::UnreadOnly, 3);
        assert!(fallback.preferences_load.is_loaded());
        assert_eq!(fallback.read_filter, ReadFilter::UnreadOnly);

        let mut chosen = EntriesPageState::new();
        unavailable(&mut chosen);
        reduce_entries_page_intent(
            &mut chosen,
            EntriesPageIntent::SetReadFilter(ReadFilter::ReadOnly),
        );
        reread(&mut chosen, ReadFilter::UnreadOnly, 3);
        assert!(chosen.preferences_load.is_loaded(), "ready again (review C4B-B-R2-06)");
        assert_eq!(chosen.read_filter, ReadFilter::ReadOnly, "the user's filters stay");
        assert_eq!(chosen.stored_workspace, Some(workspace(ReadFilter::UnreadOnly)));
        assert_eq!(chosen.recovered_edits, Some(chosen.workspace()), "not saved by the read");
        assert_eq!(chosen.entries_page_size, 3);
    }

    /// A failed read-only refresh releases a start it superseded; otherwise it is a read's
    /// failure, which the next read replaces.
    #[test]
    fn a_failed_reread_is_a_read_failure() {
        let failure = || crate::ui::failure::plain("读不到");
        let mut pending = EntriesPageState::new();
        reduce_entries_page_intent(
            &mut pending,
            EntriesPageIntent::PreferencesRereadFailed(failure()),
        );
        assert!(pending.preferences_load.can_load_entries(), "the list does not wait");
        assert!(!pending.preferences_load.is_loaded());

        let mut loaded = loaded_page(ReadFilter::All);
        reduce_entries_page_intent(
            &mut loaded,
            EntriesPageIntent::PreferencesRereadFailed(failure()),
        );
        assert!(loaded.preferences_load.is_loaded());
        assert_eq!(loaded.status_tone, "error");
        reduce_entries_page_intent(
            &mut loaded,
            EntriesPageIntent::SetEntries {
                entries: vec![entry(1, false)],
                archived_count: 0,
                truncated: false,
            },
        );
        assert_eq!(loaded.status, "共 1 篇文章。");
    }

    /// A write's failure or unknown outcome stays over the list read again (it only updates what
    /// is shown); the filters changing, or another status, replace it (C4b item 2).
    #[test]
    fn a_writes_failure_stays_over_a_list_read() {
        let unknown = crate::ui::failure::UiFailure::new(
            "更改已读状态",
            rssr_api::BackendError::new(rssr_api::ErrorCode::OutcomeUnknown, "结果未知"),
        );
        let list = || EntriesPageIntent::SetEntries {
            entries: vec![entry(1, true)],
            archived_count: 0,
            truncated: false,
        };
        let mut state = loaded_page(ReadFilter::All);
        reduce_entries_page_intent(&mut state, EntriesPageIntent::Failed(unknown.clone()));
        reduce_entries_page_intent(&mut state, list());
        assert_eq!(state.status, unknown.message());
        assert_eq!(state.status_tone, unknown.tone());

        reduce_entries_page_intent(&mut state, EntriesPageIntent::CancelBulk);
        reduce_entries_page_intent(&mut state, list());
        assert_eq!(state.status, "共 1 篇文章。");

        reduce_entries_page_intent(&mut state, EntriesPageIntent::BulkFailed(unknown.clone()));
        reduce_entries_page_intent(&mut state, list());
        assert_eq!(state.status, unknown.message(), "a bulk write's too");
        reduce_entries_page_intent(
            &mut state,
            EntriesPageIntent::SetStatus { message: "已收藏。".into(), tone: "info".into() },
        );
        reduce_entries_page_intent(&mut state, list());
        assert_eq!(state.status, "共 1 篇文章。");
    }

    /// A new snapshot keeps only visible selected IDs; it never selects newly arrived rows.
    #[test]
    fn a_changed_list_reconciles_selected_ids() {
        let mut state = loaded_page(ReadFilter::All);
        state.current_page = 1;
        reduce_entries_page_intent(&mut state, EntriesPageIntent::TogglePageSelection);
        let before = state.selected_entry_ids.clone();
        let shown = state.entries.iter().map(|entry| (**entry).clone()).collect();
        reduce_entries_page_intent(
            &mut state,
            EntriesPageIntent::SetEntries { entries: shown, archived_count: 0, truncated: false },
        );
        assert_eq!(state.selected_entry_ids, before);
        reduce_entries_page_intent(
            &mut state,
            EntriesPageIntent::SetEntries {
                entries: vec![entry(1, false), entry(99, false)],
                archived_count: 0,
                truncated: false,
            },
        );
        assert_eq!(*state.selected_entry_ids, [1].into_iter().collect());
    }

    /// A bulk mark-read's notice waiting for the list read does not replace a later write's
    /// unknown outcome (review C4B-B-04).
    #[test]
    fn a_later_unknown_outcome_outlasts_a_bulk_notice() {
        let unknown = crate::ui::failure::UiFailure::new(
            "更改收藏状态",
            rssr_api::BackendError::new(rssr_api::ErrorCode::OutcomeUnknown, "结果未知"),
        );
        let mut state = loaded_page(ReadFilter::All);
        reduce_entries_page_intent(
            &mut state,
            EntriesPageIntent::SelectionApplied {
                ids: vec![1, 2],
                action: rssr_domain::EntryBatchAction::MarkRead,
                count: 2,
            },
        );
        reduce_entries_page_intent(&mut state, EntriesPageIntent::Failed(unknown.clone()));
        reduce_entries_page_intent(
            &mut state,
            EntriesPageIntent::SetEntries {
                entries: vec![entry(1, true)],
                archived_count: 0,
                truncated: false,
            },
        );
        assert_eq!(state.status, unknown.message());
        assert_eq!(state.status_tone, unknown.tone());
    }
}

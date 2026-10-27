use super::entry_flags::{Flag, flag_message, flag_tone};
use crate::{
    pages::entries_page::intent::EntriesPageIntent,
    ui::{
        commands::EntriesCommand, failure::UiFailure, runtime::backend::backend, snapshot::UiIntent,
    },
};
use rssr_application::{EntriesBootstrapInput, EntriesBootstrapOutcome, EntryFlagOutcome};
use rssr_domain::EntriesWorkspaceState;

pub(super) async fn execute(command: EntriesCommand) -> Vec<UiIntent> {
    match command {
        EntriesCommand::PreviewMarkRead { query } => {
            let result = async { backend().await?.preview_mark_read(&query).await }.await;
            vec![UiIntent::EntriesPage(match result {
                Ok(preview) => EntriesPageIntent::BulkPreview(preview),
                Err(error) => EntriesPageIntent::BulkFailed(UiFailure::new("预览批量已读", error)),
            })]
        }
        EntriesCommand::ConfirmMarkRead { preview } => {
            let result = async { backend().await?.mark_read_if_unchanged(&preview).await }.await;
            vec![UiIntent::EntriesPage(match result {
                Ok(rssr_domain::MarkReadOutcome::Applied { changed_count }) => {
                    EntriesPageIntent::BulkApplied(changed_count)
                }
                Ok(rssr_domain::MarkReadOutcome::SelectionChanged { preview }) => {
                    EntriesPageIntent::BulkPreview(preview)
                }
                Err(error) => EntriesPageIntent::BulkFailed(UiFailure::new("批量标为已读", error)),
            })]
        }

        EntriesCommand::Bootstrap { load_preferences, load_feeds } => {
            let input = EntriesBootstrapInput { load_preferences, load_feeds };
            match async { backend().await?.bootstrap_entries(input).await }.await {
                Ok(outcome) => bootstrap_intents(outcome, load_preferences),
                Err(error) => {
                    bootstrap_error(UiFailure::new("加载文章页初始状态", error), load_preferences)
                }
            }
        }
        EntriesCommand::Reread => {
            let input = EntriesBootstrapInput { load_preferences: true, load_feeds: true };
            match async { backend().await?.bootstrap_entries(input).await }.await {
                Ok(outcome) => reread_intents(outcome),
                Err(error) => entries_intents(vec![EntriesPageIntent::PreferencesRereadFailed(
                    UiFailure::new("重新读取文章页", error),
                )]),
            }
        }
        EntriesCommand::RememberOpenedFeed { feed_id } => {
            match async { backend().await?.remember_opened_feed(feed_id).await }.await {
                Ok(()) => Vec::new(),
                Err(error) => entries_failed("记住上次打开的订阅", error),
            }
        }
        EntriesCommand::LoadEntries { query } => {
            match async { backend().await?.list_entries(&query).await }.await {
                Ok(outcome) => {
                    vec![UiIntent::EntriesPage(EntriesPageIntent::SetEntries {
                        entries: outcome
                            .entries
                            .into_iter()
                            .map(|mut entry| {
                                entry.published_at =
                                    entry.published_at.map(crate::datetime::local_timestamp);
                                entry
                            })
                            .collect(),
                        archived_count: outcome.archived_count as usize,
                        truncated: outcome.truncated,
                    })]
                }
                Err(error) => entries_intents(vec![EntriesPageIntent::ReadFailed(UiFailure::new(
                    "读取文章列表",
                    error,
                ))]),
            }
        }
        EntriesCommand::SetRead { entry_id, entry_title, read } => {
            let result = async { backend().await?.set_entry_read(entry_id, read).await }.await;
            entry_flag_intents(Flag::Read, entry_id, &entry_title, result)
        }
        EntriesCommand::SetStarred { entry_id, entry_title, starred } => {
            let result =
                async { backend().await?.set_entry_starred(entry_id, starred).await }.await;
            entry_flag_intents(Flag::Starred, entry_id, &entry_title, result)
        }
        EntriesCommand::SaveBrowsingPreferences {
            grouping_mode,
            show_archived,
            read_filter,
            starred_filter,
            selected_feed_urls,
        } => {
            let next = EntriesWorkspaceState {
                grouping_mode,
                show_archived,
                read_filter,
                starred_filter,
                selected_feed_urls,
            };
            let saved = next.clone();
            match async { backend().await?.save_entries_workspace_if_changed(next).await }.await {
                Ok(true) | Ok(false) => {
                    entries_intents(vec![EntriesPageIntent::WorkspaceSaved(saved)])
                }
                Err(error) => entries_failed("保存文章页偏好", error),
            }
        }
    }
}

fn bootstrap_intents(outcome: EntriesBootstrapOutcome, load_preferences: bool) -> Vec<UiIntent> {
    let mut intents = Vec::new();
    if let Some(settings) = outcome.settings {
        intents.push(EntriesPageIntent::ApplyLoadedSettings(settings));
    }
    if let Some(workspace) = outcome.workspace {
        intents.push(EntriesPageIntent::ApplyLoadedWorkspaceState(workspace));
    }
    if let Some(feeds) = outcome.feeds {
        intents.push(EntriesPageIntent::SetFeeds(feeds));
    }
    if load_preferences {
        // Open the first-query gate only after source URL-to-ID mappings and
        // every saved preference have been applied to the page.
        intents.push(EntriesPageIntent::PreferencesLoaded);
    }
    entries_intents(intents)
}

fn bootstrap_error(failure: UiFailure, load_preferences: bool) -> Vec<UiIntent> {
    if load_preferences {
        entries_intents(vec![EntriesPageIntent::PreferencesUnavailable(failure)])
    } else {
        entries_intents(vec![EntriesPageIntent::ReadFailed(failure)])
    }
}

/// The read-only refresh's reply: the preferences for the reducer to weigh, and the
/// subscriptions (C4b item 2).
fn reread_intents(outcome: EntriesBootstrapOutcome) -> Vec<UiIntent> {
    let mut intents = Vec::new();
    if let (Some(settings), Some(workspace)) = (outcome.settings, outcome.workspace) {
        intents.push(EntriesPageIntent::PreferencesReread { settings, workspace });
    }
    if let Some(feeds) = outcome.feeds {
        intents.push(EntriesPageIntent::SetFeeds(feeds));
    }
    entries_intents(intents)
}

fn entries_intents(intents: Vec<EntriesPageIntent>) -> Vec<UiIntent> {
    intents.into_iter().map(UiIntent::EntriesPage).collect()
}

fn entries_failed(action: &'static str, error: rssr_api::BackendError) -> Vec<UiIntent> {
    vec![UiIntent::Failed(UiFailure::new(action, error))]
}

/// The status and, when the entry still exists, the flag patch for a list card.
fn entry_flag_intents(
    flag: Flag,
    entry_id: i64,
    entry_title: &str,
    result: rssr_api::Result<EntryFlagOutcome>,
) -> Vec<UiIntent> {
    let outcome = match result {
        Ok(outcome) => outcome,
        Err(error) => return entries_failed(flag.action(), error),
    };
    let mut intents = vec![EntriesPageIntent::SetStatus {
        message: flag_message(flag, &format!("《{entry_title}》"), outcome),
        tone: flag_tone(outcome).to_string(),
    }];
    if let EntryFlagOutcome::Set { value, .. } = outcome {
        intents.push(EntriesPageIntent::PatchEntryFlags {
            entry_id,
            is_read: (flag == Flag::Read).then_some(value),
            is_starred: (flag == Flag::Starred).then_some(value),
        });
    }
    entries_intents(intents)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rssr_domain::UserSettings;

    #[test]
    fn bootstrap_opens_query_gate_after_settings_workspace_and_feeds() {
        let intents: Vec<_> = bootstrap_intents(
            EntriesBootstrapOutcome {
                settings: Some(UserSettings::default()),
                workspace: Some(EntriesWorkspaceState::default()),
                feeds: Some(Vec::new()),
            },
            true,
        )
        .into_iter()
        .filter_map(UiIntent::into_entries_page_intent)
        .collect();
        assert!(matches!(
            intents.as_slice(),
            [
                EntriesPageIntent::ApplyLoadedSettings(_),
                EntriesPageIntent::ApplyLoadedWorkspaceState(_),
                EntriesPageIntent::SetFeeds(_),
                EntriesPageIntent::PreferencesLoaded,
            ]
        ));
        let refresh = bootstrap_intents(
            EntriesBootstrapOutcome { settings: None, workspace: None, feeds: Some(Vec::new()) },
            false,
        );
        assert!(matches!(
            refresh.as_slice(),
            [UiIntent::EntriesPage(EntriesPageIntent::SetFeeds(_))]
        ));
    }

    #[test]
    fn bootstrap_failures_release_initial_query_gate_without_marking_preferences_loaded() {
        assert!(matches!(
            bootstrap_error(crate::ui::failure::plain("read failed"), true).as_slice(),
            [UiIntent::EntriesPage(EntriesPageIntent::PreferencesUnavailable(_))]
        ));
        assert!(matches!(
            bootstrap_error(crate::ui::failure::plain("refresh summaries failed"), false)
                .as_slice(),
            [UiIntent::EntriesPage(EntriesPageIntent::ReadFailed(_))]
        ));
    }
}

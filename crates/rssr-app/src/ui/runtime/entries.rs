use super::entry_flags::{Flag, flag_message, flag_tone};
use crate::{
    pages::entries_page::intent::EntriesPageIntent,
    ui::{commands::EntriesCommand, runtime::backend::backend, snapshot::UiIntent},
};
use rssr_application::{EntriesBootstrapInput, EntriesBootstrapOutcome, EntryFlagOutcome};
use rssr_domain::EntriesWorkspaceState;

pub(super) async fn execute(command: EntriesCommand) -> Vec<UiIntent> {
    match command {
        EntriesCommand::PreviewMarkRead { query } => {
            let result = async { backend().await?.preview_mark_read(&query).await }.await;
            vec![UiIntent::EntriesPage(match result {
                Ok(preview) => EntriesPageIntent::BulkPreview(preview),
                Err(error) => EntriesPageIntent::BulkFailed(error.to_string()),
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
                Err(error) => EntriesPageIntent::BulkFailed(error.to_string()),
            })]
        }

        EntriesCommand::Bootstrap { feed_id, load_preferences, load_feeds } => {
            match backend().await {
                Ok(services) => {
                    match services
                        .bootstrap_entries(EntriesBootstrapInput {
                            feed_id,
                            load_preferences,
                            load_feeds,
                        })
                        .await
                    {
                        Ok(outcome) => bootstrap_intents(outcome, load_preferences),
                        Err(err) => bootstrap_error(
                            format!("加载文章页初始状态失败：{err}"),
                            load_preferences,
                        ),
                    }
                }
                Err(err) => bootstrap_error(format!("初始化应用失败：{err}"), load_preferences),
            }
        }
        EntriesCommand::LoadEntries { query } => match backend().await {
            Ok(services) => match services.list_entries(&query).await {
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
                    })]
                }
                Err(err) => entries_status_error(format!("{err}")),
            },
            Err(err) => entries_status_error(format!("初始化应用失败：{err}")),
        },
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
        } => match backend().await {
            Ok(services) => {
                let next = EntriesWorkspaceState {
                    grouping_mode,
                    show_archived,
                    read_filter,
                    starred_filter,
                    selected_feed_urls,
                };
                match services.save_entries_workspace_if_changed(next).await {
                    Ok(true) | Ok(false) => Vec::new(),
                    Err(err) => entries_status_error(format!("保存文章页偏好失败：{err}")),
                }
            }
            Err(err) => entries_status_error(format!("初始化应用失败：{err}")),
        },
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

fn bootstrap_error(message: String, load_preferences: bool) -> Vec<UiIntent> {
    if load_preferences {
        entries_intents(vec![EntriesPageIntent::PreferencesUnavailable(message)])
    } else {
        entries_status_error(message)
    }
}

fn entries_intents(intents: Vec<EntriesPageIntent>) -> Vec<UiIntent> {
    intents.into_iter().map(UiIntent::EntriesPage).collect()
}

fn entries_status_error(message: impl Into<String>) -> Vec<UiIntent> {
    entries_intents(vec![EntriesPageIntent::SetStatus {
        message: message.into(),
        tone: "error".to_string(),
    }])
}

/// The status and, when the entry still exists, the flag patch for a list card.
fn entry_flag_intents(
    flag: Flag,
    entry_id: i64,
    entry_title: &str,
    result: anyhow::Result<EntryFlagOutcome>,
) -> Vec<UiIntent> {
    let outcome = match result {
        Ok(outcome) => outcome,
        Err(err) => return entries_status_error(format!("{err}")),
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
            bootstrap_error("read failed".into(), true).as_slice(),
            [UiIntent::EntriesPage(EntriesPageIntent::PreferencesUnavailable(_))]
        ));
        assert!(matches!(
            bootstrap_error("refresh summaries failed".into(), false).as_slice(),
            [UiIntent::EntriesPage(EntriesPageIntent::SetStatus { .. })]
        ));
    }
}

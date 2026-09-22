use crate::{
    pages::entries_page::intent::EntriesPageIntent,
    ui::{commands::EntriesCommand, runtime::services::UiServices, snapshot::UiIntent},
};
use rssr_application::{
    EntriesBootstrapInput, EntriesBootstrapOutcome, ToggleEntryReadInput, ToggleEntryStarredInput,
};
use rssr_domain::EntriesWorkspaceState;

pub(super) async fn execute(command: EntriesCommand) -> Vec<UiIntent> {
    match command {
        EntriesCommand::Bootstrap { feed_id, load_preferences, load_feeds } => {
            match UiServices::shared().await {
                Ok(services) => {
                    match services
                        .entries()
                        .bootstrap(EntriesBootstrapInput { feed_id, load_preferences, load_feeds })
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
        EntriesCommand::LoadEntries { query } => match UiServices::shared().await {
            Ok(services) => match services.entries().list_entries(&query).await {
                Ok(outcome) => {
                    vec![UiIntent::EntriesPage(EntriesPageIntent::SetEntries {
                        entries: outcome.entries,
                        archived_count: outcome.archived_count as usize,
                    })]
                }
                Err(err) => entries_status_error(format!("{err}")),
            },
            Err(err) => entries_status_error(format!("初始化应用失败：{err}")),
        },
        EntriesCommand::ToggleRead { entry_id, entry_title, currently_read } => {
            match UiServices::shared().await {
                Ok(services) => {
                    match services
                        .entries()
                        .toggle_read(ToggleEntryReadInput { entry_id, currently_read })
                        .await
                    {
                        Ok(outcome) => entries_intents(vec![
                            EntriesPageIntent::SetStatus {
                                message: format!(
                                    "已将《{}》{}。",
                                    entry_title,
                                    if outcome.is_read {
                                        "标记为已读"
                                    } else {
                                        "标记为未读"
                                    }
                                ),
                                tone: "info".to_string(),
                            },
                            EntriesPageIntent::PatchEntryFlags {
                                entry_id,
                                is_read: Some(outcome.is_read),
                                is_starred: None,
                            },
                        ]),
                        Err(err) => entries_status_error(format!("{err}")),
                    }
                }
                Err(err) => entries_status_error(format!("初始化应用失败：{err}")),
            }
        }
        EntriesCommand::ToggleStarred { entry_id, entry_title, currently_starred } => {
            match UiServices::shared().await {
                Ok(services) => {
                    match services
                        .entries()
                        .toggle_starred(ToggleEntryStarredInput { entry_id, currently_starred })
                        .await
                    {
                        Ok(outcome) => entries_intents(vec![
                            EntriesPageIntent::SetStatus {
                                message: format!(
                                    "已{}《{}》。",
                                    if outcome.is_starred { "收藏" } else { "取消收藏" },
                                    entry_title
                                ),
                                tone: "info".to_string(),
                            },
                            EntriesPageIntent::PatchEntryFlags {
                                entry_id,
                                is_read: None,
                                is_starred: Some(outcome.is_starred),
                            },
                        ]),
                        Err(err) => entries_status_error(format!("{err}")),
                    }
                }
                Err(err) => entries_status_error(format!("初始化应用失败：{err}")),
            }
        }
        EntriesCommand::SaveBrowsingPreferences {
            grouping_mode,
            show_archived,
            read_filter,
            starred_filter,
            selected_feed_urls,
        } => match UiServices::shared().await {
            Ok(services) => {
                let next = EntriesWorkspaceState {
                    grouping_mode,
                    show_archived,
                    read_filter,
                    starred_filter,
                    selected_feed_urls,
                };
                match services.entries().save_workspace_if_changed(next).await {
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

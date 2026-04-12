use crate::{
    pages::entries_page::intent::EntriesPageIntent,
    ui::{commands::EntriesCommand, runtime::services::UiServices, snapshot::UiIntent},
};
use rssr_application::EntriesBootstrapInput;
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
                        Ok(outcome) => {
                            let mut intents = Vec::new();
                            if let Some(settings) = outcome.settings {
                                intents.push(UiIntent::EntriesPage(
                                    EntriesPageIntent::ApplyLoadedSettings(settings),
                                ));
                            }
                            if let Some(workspace) = outcome.workspace {
                                intents.push(UiIntent::EntriesPage(
                                    EntriesPageIntent::ApplyLoadedWorkspaceState(workspace),
                                ));
                            }
                            if let Some(feeds) = outcome.feeds {
                                intents.push(UiIntent::EntriesPage(EntriesPageIntent::SetFeeds(
                                    feeds,
                                )));
                            }
                            intents
                        }
                        Err(err) => entries_status_error(format!("加载文章页初始状态失败：{err}")),
                    }
                }
                Err(err) => entries_status_error(format!("初始化应用失败：{err}")),
            }
        }
        EntriesCommand::LoadEntries { query } => match UiServices::shared().await {
            Ok(services) => match services.entries().list_entries(&query).await {
                Ok(entries) => vec![UiIntent::EntriesPage(EntriesPageIntent::SetEntries(entries))],
                Err(err) => entries_status_error(format!("读取文章失败：{err}")),
            },
            Err(err) => entries_status_error(format!("初始化应用失败：{err}")),
        },
        EntriesCommand::ToggleRead { entry_id, entry_title, currently_read } => {
            match UiServices::shared().await {
                Ok(services) => {
                    match services.entries().set_read(entry_id, !currently_read).await {
                        Ok(()) => entries_intents(vec![
                            EntriesPageIntent::SetStatus {
                                message: format!(
                                    "已将《{}》{}。",
                                    entry_title,
                                    if currently_read {
                                        "标记为未读"
                                    } else {
                                        "标记为已读"
                                    }
                                ),
                                tone: "info".to_string(),
                            },
                            EntriesPageIntent::BumpReload,
                        ]),
                        Err(err) => entries_status_error(format!("更新已读状态失败：{err}")),
                    }
                }
                Err(err) => entries_status_error(format!("初始化应用失败：{err}")),
            }
        }
        EntriesCommand::ToggleStarred { entry_id, entry_title, currently_starred } => {
            match UiServices::shared().await {
                Ok(services) => {
                    match services.entries().set_starred(entry_id, !currently_starred).await {
                        Ok(()) => entries_intents(vec![
                            EntriesPageIntent::SetStatus {
                                message: format!(
                                    "已{}《{}》。",
                                    if currently_starred { "取消收藏" } else { "收藏" },
                                    entry_title
                                ),
                                tone: "info".to_string(),
                            },
                            EntriesPageIntent::BumpReload,
                        ]),
                        Err(err) => entries_status_error(format!("更新收藏状态失败：{err}")),
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

fn entries_intents(intents: Vec<EntriesPageIntent>) -> Vec<UiIntent> {
    intents.into_iter().map(UiIntent::EntriesPage).collect()
}

fn entries_status_error(message: impl Into<String>) -> Vec<UiIntent> {
    entries_intents(vec![EntriesPageIntent::SetStatus {
        message: message.into(),
        tone: "error".to_string(),
    }])
}

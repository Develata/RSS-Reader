use crate::{
    bootstrap::AppServices,
    pages::entries_page::intent::EntriesPageIntent,
    ui::{commands::EntriesCommand, snapshot::UiIntent},
};

pub(super) async fn execute(command: EntriesCommand) -> Vec<UiIntent> {
    match command {
        EntriesCommand::Bootstrap { feed_id, load_preferences, load_feeds } => {
            match AppServices::shared().await {
                Ok(services) => {
                    let mut intents = Vec::new();

                    if let Some(feed_id) = feed_id {
                        let _ = services.remember_last_opened_feed_id(feed_id).await;
                    }

                    if load_preferences {
                        match services.load_settings().await {
                            Ok(settings) => intents.push(UiIntent::EntriesPage(
                                EntriesPageIntent::ApplyLoadedSettings(settings),
                            )),
                            Err(err) => {
                                return entries_status_error(format!("读取设置失败：{err}"));
                            }
                        }
                    }

                    if load_feeds {
                        match services.list_feeds().await {
                            Ok(feeds) => intents
                                .push(UiIntent::EntriesPage(EntriesPageIntent::SetFeeds(feeds))),
                            Err(err) => {
                                return entries_status_error(format!("读取订阅失败：{err}"));
                            }
                        }
                    }

                    intents
                }
                Err(err) => entries_status_error(format!("初始化应用失败：{err}")),
            }
        }
        EntriesCommand::LoadEntries { query } => match AppServices::shared().await {
            Ok(services) => match services.list_entries(&query).await {
                Ok(entries) => vec![UiIntent::EntriesPage(EntriesPageIntent::SetEntries(entries))],
                Err(err) => entries_status_error(format!("读取文章失败：{err}")),
            },
            Err(err) => entries_status_error(format!("初始化应用失败：{err}")),
        },
        EntriesCommand::ToggleRead { entry_id, entry_title, currently_read } => {
            match AppServices::shared().await {
                Ok(services) => match services.set_read(entry_id, !currently_read).await {
                    Ok(()) => entries_intents(vec![
                        EntriesPageIntent::SetStatus {
                            message: format!(
                                "已将《{}》{}。",
                                entry_title,
                                if currently_read { "标记为未读" } else { "标记为已读" }
                            ),
                            tone: "info".to_string(),
                        },
                        EntriesPageIntent::BumpReload,
                    ]),
                    Err(err) => entries_status_error(format!("更新已读状态失败：{err}")),
                },
                Err(err) => entries_status_error(format!("初始化应用失败：{err}")),
            }
        }
        EntriesCommand::ToggleStarred { entry_id, entry_title, currently_starred } => {
            match AppServices::shared().await {
                Ok(services) => match services.set_starred(entry_id, !currently_starred).await {
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
                },
                Err(err) => entries_status_error(format!("初始化应用失败：{err}")),
            }
        }
        EntriesCommand::SaveBrowsingPreferences {
            grouping_mode,
            show_archived,
            read_filter,
            starred_filter,
            selected_feed_urls,
        } => match AppServices::shared().await {
            Ok(services) => match services.load_settings().await {
                Ok(mut settings) => {
                    let changed = settings.entry_grouping_mode != grouping_mode
                        || settings.show_archived_entries != show_archived
                        || settings.entry_read_filter != read_filter
                        || settings.entry_starred_filter != starred_filter
                        || settings.entry_filtered_feed_urls != selected_feed_urls;

                    if !changed {
                        return Vec::new();
                    }

                    settings.entry_grouping_mode = grouping_mode;
                    settings.show_archived_entries = show_archived;
                    settings.entry_read_filter = read_filter;
                    settings.entry_starred_filter = starred_filter;
                    settings.entry_filtered_feed_urls = selected_feed_urls;

                    match services.save_settings(&settings).await {
                        Ok(()) => Vec::new(),
                        Err(err) => entries_status_error(format!("保存文章页偏好失败：{err}")),
                    }
                }
                Err(err) => entries_status_error(format!("读取文章页偏好失败：{err}")),
            },
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

use std::sync::Arc;

use crate::bootstrap::AppServices;

use super::{
    effect::EntriesPageEffect,
    intent::EntriesPageIntent,
    queries::{
        load_entries_page_entries, load_entries_page_feeds, load_entries_page_preferences,
        remember_last_opened_feed,
    },
};

pub(crate) struct EntriesPageRuntimeOutcome {
    pub(crate) intents: Vec<EntriesPageIntent>,
}

impl EntriesPageRuntimeOutcome {
    pub(crate) fn empty() -> Self {
        Self { intents: Vec::new() }
    }
}

pub(crate) async fn execute_entries_page_effect(
    effect: EntriesPageEffect,
) -> EntriesPageRuntimeOutcome {
    match effect {
        EntriesPageEffect::Bootstrap { feed_id, load_preferences, load_feeds } => {
            let mut intents = Vec::new();

            if let Some(feed_id) = feed_id {
                remember_last_opened_feed(feed_id).await;
            }

            if load_preferences {
                match load_entries_page_preferences().await {
                    Ok(settings) => intents.push(EntriesPageIntent::ApplyLoadedSettings(settings)),
                    Err(err) => return status_error(err),
                }
            }

            if load_feeds {
                match load_entries_page_feeds().await {
                    Ok(feeds) => intents.push(EntriesPageIntent::SetFeeds(feeds)),
                    Err(err) => return status_error(err),
                }
            }

            EntriesPageRuntimeOutcome { intents }
        }
        EntriesPageEffect::LoadEntries(query) => match load_entries_page_entries(query).await {
            Ok(entries) => {
                EntriesPageRuntimeOutcome { intents: vec![EntriesPageIntent::SetEntries(entries)] }
            }
            Err(err) => status_error(err),
        },
        EntriesPageEffect::ToggleRead { entry_id, entry_title, currently_read } => {
            match shared_services().await {
                Ok(services) => match services.set_read(entry_id, !currently_read).await {
                    Ok(()) => EntriesPageRuntimeOutcome {
                        intents: vec![
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
                        ],
                    },
                    Err(err) => status_error(format!("更新已读状态失败：{err}")),
                },
                Err(err) => status_error(err),
            }
        }
        EntriesPageEffect::ToggleStarred { entry_id, entry_title, currently_starred } => {
            match shared_services().await {
                Ok(services) => match services.set_starred(entry_id, !currently_starred).await {
                    Ok(()) => EntriesPageRuntimeOutcome {
                        intents: vec![
                            EntriesPageIntent::SetStatus {
                                message: format!(
                                    "已{}《{}》。",
                                    if currently_starred { "取消收藏" } else { "收藏" },
                                    entry_title
                                ),
                                tone: "info".to_string(),
                            },
                            EntriesPageIntent::BumpReload,
                        ],
                    },
                    Err(err) => status_error(format!("更新收藏状态失败：{err}")),
                },
                Err(err) => status_error(err),
            }
        }
        EntriesPageEffect::SaveBrowsingPreferences {
            grouping_mode,
            show_archived,
            read_filter,
            starred_filter,
            selected_feed_urls,
        } => match shared_services().await {
            Ok(services) => match services.load_settings().await {
                Ok(mut settings) => {
                    let changed = settings.entry_grouping_mode != grouping_mode
                        || settings.show_archived_entries != show_archived
                        || settings.entry_read_filter != read_filter
                        || settings.entry_starred_filter != starred_filter
                        || settings.entry_filtered_feed_urls != selected_feed_urls;

                    if !changed {
                        return EntriesPageRuntimeOutcome::empty();
                    }

                    settings.entry_grouping_mode = grouping_mode;
                    settings.show_archived_entries = show_archived;
                    settings.entry_read_filter = read_filter;
                    settings.entry_starred_filter = starred_filter;
                    settings.entry_filtered_feed_urls = selected_feed_urls;

                    match services.save_settings(&settings).await {
                        Ok(()) => EntriesPageRuntimeOutcome::empty(),
                        Err(err) => status_error(format!("保存文章页偏好失败：{err}")),
                    }
                }
                Err(err) => status_error(format!("读取文章页偏好失败：{err}")),
            },
            Err(err) => status_error(err),
        },
    }
}

fn status_error(message: impl Into<String>) -> EntriesPageRuntimeOutcome {
    EntriesPageRuntimeOutcome {
        intents: vec![EntriesPageIntent::SetStatus {
            message: message.into(),
            tone: "error".to_string(),
        }],
    }
}

async fn shared_services() -> Result<Arc<AppServices>, String> {
    AppServices::shared().await.map_err(|err| format!("初始化应用失败：{err}"))
}

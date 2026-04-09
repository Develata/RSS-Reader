use std::sync::Arc;

use crate::bootstrap::{AppServices, ReaderNavigation};

use super::{
    effect::ReaderPageEffect,
    intent::ReaderPageIntent,
    state::ReaderPageLoadedContent,
    support::{ReaderBody, format_reader_datetime_utc, select_reader_body},
};

pub(crate) struct ReaderPageRuntimeOutcome {
    pub(crate) intents: Vec<ReaderPageIntent>,
}

pub(crate) async fn execute_reader_page_effect(
    effect: ReaderPageEffect,
) -> ReaderPageRuntimeOutcome {
    match effect {
        ReaderPageEffect::LoadEntry(entry_id) => {
            let mut intents = vec![ReaderPageIntent::BeginLoading];
            match shared_services().await {
                Ok(services) => match services.get_entry(entry_id).await {
                    Ok(Some(entry)) => {
                        let (body_html, body_text) = match select_reader_body(
                            entry.content_html,
                            entry.content_text,
                            entry.summary,
                        ) {
                            ReaderBody::Html(html) => (Some(html), String::new()),
                            ReaderBody::Text(text) => (None, text),
                        };

                        let content = ReaderPageLoadedContent {
                            title: entry.title,
                            body_text,
                            body_html,
                            source: entry
                                .url
                                .map(|url| url.to_string())
                                .unwrap_or_else(|| "无原文链接".to_string()),
                            published_at: format_reader_datetime_utc(entry.published_at)
                                .unwrap_or_else(|| "未知发布时间".to_string()),
                            is_read: entry.is_read,
                            is_starred: entry.is_starred,
                            navigation_state: services
                                .reader_navigation(entry_id)
                                .await
                                .unwrap_or_else(|_| ReaderNavigation::default()),
                        };
                        intents.push(ReaderPageIntent::ApplyLoadedContent(content));
                    }
                    Ok(None) => {
                        intents.push(ReaderPageIntent::SetError(Some("文章不存在".to_string())))
                    }
                    Err(err) => intents.push(ReaderPageIntent::SetError(Some(err.to_string()))),
                },
                Err(err) => intents.push(ReaderPageIntent::SetError(Some(err))),
            }
            ReaderPageRuntimeOutcome { intents }
        }
        ReaderPageEffect::ToggleRead { entry_id, currently_read, via_shortcut } => {
            match shared_services().await {
                Ok(services) => match services.set_read(entry_id, !currently_read).await {
                    Ok(()) => ReaderPageRuntimeOutcome {
                        intents: vec![
                            ReaderPageIntent::SetStatus {
                                message: if via_shortcut {
                                    if currently_read {
                                        "已通过快捷键标记为未读。".to_string()
                                    } else {
                                        "已通过快捷键标记为已读。".to_string()
                                    }
                                } else if currently_read {
                                    "已将当前文章标记为未读。".to_string()
                                } else {
                                    "已将当前文章标记为已读。".to_string()
                                },
                                tone: "info".to_string(),
                            },
                            ReaderPageIntent::BumpReload,
                        ],
                    },
                    Err(err) => status_error(format!("更新已读状态失败：{err}")),
                },
                Err(err) => status_error(err),
            }
        }
        ReaderPageEffect::ToggleStarred { entry_id, currently_starred, via_shortcut } => {
            match shared_services().await {
                Ok(services) => match services.set_starred(entry_id, !currently_starred).await {
                    Ok(()) => ReaderPageRuntimeOutcome {
                        intents: vec![
                            ReaderPageIntent::SetStatus {
                                message: if via_shortcut {
                                    if currently_starred {
                                        "已通过快捷键取消收藏。".to_string()
                                    } else {
                                        "已通过快捷键收藏文章。".to_string()
                                    }
                                } else if currently_starred {
                                    "已取消收藏当前文章。".to_string()
                                } else {
                                    "已收藏当前文章。".to_string()
                                },
                                tone: "info".to_string(),
                            },
                            ReaderPageIntent::BumpReload,
                        ],
                    },
                    Err(err) => status_error(format!("更新收藏状态失败：{err}")),
                },
                Err(err) => status_error(err),
            }
        }
    }
}

fn status_error(message: impl Into<String>) -> ReaderPageRuntimeOutcome {
    ReaderPageRuntimeOutcome {
        intents: vec![ReaderPageIntent::SetStatus {
            message: message.into(),
            tone: "error".to_string(),
        }],
    }
}

async fn shared_services() -> Result<Arc<AppServices>, String> {
    AppServices::shared().await.map_err(|err| format!("初始化应用失败：{err}"))
}

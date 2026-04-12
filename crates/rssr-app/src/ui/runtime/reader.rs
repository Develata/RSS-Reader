use crate::{
    pages::reader_page::{
        intent::ReaderPageIntent,
        state::ReaderPageLoadedContent,
        support::{ReaderBody, format_reader_datetime_utc, select_reader_body},
    },
    ui::{commands::ReaderCommand, runtime::services::UiServices, snapshot::UiIntent},
};
use rssr_application::{ToggleReadInput, ToggleStarredInput};

pub(super) async fn execute(command: ReaderCommand) -> Vec<UiIntent> {
    match command {
        ReaderCommand::LoadEntry { entry_id } => match UiServices::shared().await {
            Ok(services) => {
                let mut intents = vec![UiIntent::ReaderPage(ReaderPageIntent::BeginLoading)];
                match services.reader().load_entry(entry_id).await {
                    Ok(snapshot) => {
                        let Some(entry) = snapshot.entry else {
                            intents.push(UiIntent::ReaderPage(ReaderPageIntent::SetError(Some(
                                "文章不存在".to_string(),
                            ))));
                            return intents;
                        };

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
                            navigation_state: snapshot.navigation,
                        };
                        intents.push(UiIntent::ReaderPage(ReaderPageIntent::ApplyLoadedContent(
                            content,
                        )));
                    }
                    Err(err) => intents.push(UiIntent::ReaderPage(ReaderPageIntent::SetError(
                        Some(err.to_string()),
                    ))),
                }
                intents
            }
            Err(err) => reader_status_error(format!("初始化应用失败：{err}")),
        },
        ReaderCommand::ToggleRead { entry_id, currently_read, via_shortcut } => {
            match UiServices::shared().await {
                Ok(services) => match services
                    .reader()
                    .toggle_read(ToggleReadInput { entry_id, currently_read })
                    .await
                {
                    Ok(outcome) => reader_intents(vec![
                        ReaderPageIntent::SetStatus {
                            message: if via_shortcut {
                                if outcome.is_read {
                                    "已通过快捷键标记为已读。".to_string()
                                } else {
                                    "已通过快捷键标记为未读。".to_string()
                                }
                            } else if outcome.is_read {
                                "已将当前文章标记为已读。".to_string()
                            } else {
                                "已将当前文章标记为未读。".to_string()
                            },
                            tone: "info".to_string(),
                        },
                        ReaderPageIntent::BumpReload,
                    ]),
                    Err(err) => reader_status_error(format!("{err}")),
                },
                Err(err) => reader_status_error(format!("初始化应用失败：{err}")),
            }
        }
        ReaderCommand::ToggleStarred { entry_id, currently_starred, via_shortcut } => {
            match UiServices::shared().await {
                Ok(services) => {
                    match services
                        .reader()
                        .toggle_starred(ToggleStarredInput { entry_id, currently_starred })
                        .await
                    {
                        Ok(outcome) => reader_intents(vec![
                            ReaderPageIntent::SetStatus {
                                message: if via_shortcut {
                                    if outcome.is_starred {
                                        "已通过快捷键收藏文章。".to_string()
                                    } else {
                                        "已通过快捷键取消收藏。".to_string()
                                    }
                                } else if outcome.is_starred {
                                    "已收藏当前文章。".to_string()
                                } else {
                                    "已取消收藏当前文章。".to_string()
                                },
                                tone: "info".to_string(),
                            },
                            ReaderPageIntent::BumpReload,
                        ]),
                        Err(err) => reader_status_error(format!("{err}")),
                    }
                }
                Err(err) => reader_status_error(format!("初始化应用失败：{err}")),
            }
        }
    }
}

fn reader_intents(intents: Vec<ReaderPageIntent>) -> Vec<UiIntent> {
    intents.into_iter().map(UiIntent::ReaderPage).collect()
}

fn reader_status_error(message: impl Into<String>) -> Vec<UiIntent> {
    reader_intents(vec![ReaderPageIntent::SetStatus {
        message: message.into(),
        tone: "error".to_string(),
    }])
}

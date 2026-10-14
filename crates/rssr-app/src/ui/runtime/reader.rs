use super::entry_flags::{Flag, flag_message, flag_tone};
use crate::{
    datetime::format_datetime,
    pages::reader_page::{
        intent::ReaderPageIntent,
        state::ReaderPageLoadedContent,
        support::{ReaderBody, select_reader_body},
    },
    ui::{commands::ReaderCommand, runtime::backend::backend, snapshot::UiIntent},
};
use rssr_application::EntryFlagOutcome;

pub(super) async fn execute(command: ReaderCommand) -> Vec<UiIntent> {
    match command {
        ReaderCommand::LoadEntry { entry_id } => match backend().await {
            Ok(services) => {
                // BeginLoading 由 session 在发起加载前同步派发，这里只产出结果。
                let mut intents = Vec::new();
                match services.load_reader_entry(entry_id).await {
                    Ok(snapshot) => {
                        let Some(entry) = snapshot.entry else {
                            intents.push(UiIntent::ReaderPage(ReaderPageIntent::SetError {
                                entry_id,
                                error: Some("文章不存在".to_string()),
                            }));
                            return intents;
                        };

                        let (body_html, body_text) = match select_reader_body(
                            entry.content_html,
                            entry.content_text,
                            entry.summary,
                            entry.url.as_ref(),
                        ) {
                            ReaderBody::Html(html) => (Some(html), String::new()),
                            ReaderBody::Text(text) => (None, text),
                        };

                        let content = ReaderPageLoadedContent {
                            title: entry.title,
                            body_text,
                            body_html,
                            source: snapshot.feed_title.unwrap_or_else(|| "未知来源".to_string()),
                            author: entry.author.filter(|author| !author.trim().is_empty()),
                            original_url: entry
                                .url
                                .filter(|url| matches!(url.scheme(), "http" | "https"))
                                .map(|url| url.to_string()),
                            published_at: format_datetime(entry.published_at)
                                .unwrap_or_else(|| "未知发布时间".to_string()),
                            is_read: entry.is_read,
                            is_starred: entry.is_starred,
                            navigation_state: snapshot.navigation,
                        };
                        intents.push(UiIntent::ReaderPage(ReaderPageIntent::ApplyLoadedContent {
                            entry_id,
                            content,
                        }));
                    }
                    Err(err) => intents.push(UiIntent::ReaderPage(ReaderPageIntent::SetError {
                        entry_id,
                        error: Some(err.to_string()),
                    })),
                }
                intents
            }
            Err(err) => reader_status_error(format!("初始化应用失败：{err}")),
        },
        ReaderCommand::LocalizeEntryAssets { entry_id } => match backend().await {
            Ok(services) => match services.localize_entry_assets(entry_id).await {
                // 缓存写回供下次打开使用。即使仍在同一篇，也不替换正在阅读的 HTML：
                // 这会销毁原生选区、图片焦点及当前滚动位置。
                Ok(_) => Vec::new(),
                Err(error) => {
                    tracing::debug!(entry_id, error = %error, "当前文章正文图片按需本地化未能完成");
                    Vec::new()
                }
            },
            Err(error) => {
                tracing::debug!(entry_id, error = %error, "初始化应用失败，跳过当前文章按需图片本地化");
                Vec::new()
            }
        },
        ReaderCommand::SetRead { entry_id, read, via_shortcut } => {
            let result = async { backend().await?.set_entry_read(entry_id, read).await }.await;
            reader_flag_intents(Flag::Read, entry_id, via_shortcut, result)
        }
        ReaderCommand::SetStarred { entry_id, starred, via_shortcut } => {
            let result =
                async { backend().await?.set_entry_starred(entry_id, starred).await }.await;
            reader_flag_intents(Flag::Starred, entry_id, via_shortcut, result)
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

/// The status and, when the entry still exists, the flag patch for the reader. A change made
/// with a shortcut says so.
fn reader_flag_intents(
    flag: Flag,
    entry_id: i64,
    via_shortcut: bool,
    result: anyhow::Result<EntryFlagOutcome>,
) -> Vec<UiIntent> {
    let outcome = match result {
        Ok(outcome) => outcome,
        Err(err) => return reader_status_error(format!("{err}")),
    };
    let message = match (via_shortcut, flag, outcome) {
        (true, Flag::Read, EntryFlagOutcome::Set { value, changed: true }) => {
            format!("已通过快捷键标记为{}。", if value { "已读" } else { "未读" })
        }
        (true, Flag::Starred, EntryFlagOutcome::Set { value, changed: true }) => if value {
            "已通过快捷键收藏文章。"
        } else {
            "已通过快捷键取消收藏。"
        }
        .to_string(),
        _ => flag_message(flag, "当前文章", outcome),
    };
    let tone = flag_tone(outcome).to_string();
    let mut intents = vec![ReaderPageIntent::SetStatus { message, tone }];
    if let EntryFlagOutcome::Set { value, .. } = outcome {
        intents.push(ReaderPageIntent::PatchEntryFlags {
            entry_id,
            is_read: (flag == Flag::Read).then_some(value),
            is_starred: (flag == Flag::Starred).then_some(value),
        });
    }
    reader_intents(intents)
}

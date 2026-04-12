use crate::{
    bootstrap::AddSubscriptionOutcome,
    pages::feeds_page::intent::{FeedsPageIntent, FeedsPageSnapshot},
    ui::{commands::FeedsCommand, runtime::services::UiServices, snapshot::UiIntent},
};
#[cfg(target_arch = "wasm32")]
use dioxus::prelude::document;
use rssr_application::{ConfigImportOutcome, OpmlImportOutcome};

pub(super) async fn execute(command: FeedsCommand) -> Vec<UiIntent> {
    match command {
        FeedsCommand::LoadSnapshot => match UiServices::shared().await {
            Ok(services) => match services.feeds().load_snapshot().await {
                Ok(outcome) => {
                    feeds_intents(vec![FeedsPageIntent::SnapshotLoaded(Ok(FeedsPageSnapshot {
                        feeds: outcome.feeds,
                        feed_count: outcome.feed_count,
                        entry_count: outcome.entry_count,
                    }))])
                }
                Err(err) => {
                    feeds_intents(vec![FeedsPageIntent::SnapshotLoaded(Err(err.to_string()))])
                }
            },
            Err(err) => feeds_status_error(format!("初始化应用失败：{err}")),
        },
        FeedsCommand::AddFeed { raw_url } => match UiServices::shared().await {
            Ok(services) => match services.feeds().add_subscription(&raw_url).await {
                Ok(AddSubscriptionOutcome::SavedAndRefreshed) => feeds_intents(vec![
                    FeedsPageIntent::FeedUrlChanged(String::new()),
                    FeedsPageIntent::SetStatus {
                        message: "订阅已保存并完成首次刷新。".to_string(),
                        tone: "info".to_string(),
                    },
                    FeedsPageIntent::BumpReload,
                ]),
                Ok(AddSubscriptionOutcome::SavedRefreshFailed { message }) => feeds_intents(vec![
                    FeedsPageIntent::FeedUrlChanged(String::new()),
                    FeedsPageIntent::SetStatus {
                        message: format!("订阅已保存，但首次刷新失败：{message}"),
                        tone: "error".to_string(),
                    },
                    FeedsPageIntent::BumpReload,
                ]),
                Err(err) => feeds_status_error(format!("保存订阅失败：{err}")),
            },
            Err(err) => feeds_status_error(format!("初始化应用失败：{err}")),
        },
        FeedsCommand::RefreshAll => match UiServices::shared().await {
            Ok(services) => match services.feeds().refresh_all().await {
                Ok(outcome) => feeds_intents(vec![
                    FeedsPageIntent::SetStatus {
                        message: outcome.failure_message.as_ref().map_or_else(
                            || "刷新完成。".to_string(),
                            |failure| format!("刷新完成，但部分订阅失败：{failure}"),
                        ),
                        tone: if outcome.failure_message.is_some() {
                            "error".to_string()
                        } else {
                            "info".to_string()
                        },
                    },
                    FeedsPageIntent::BumpReload,
                ]),
                Err(err) => feeds_status_error(format!("刷新失败：{err}")),
            },
            Err(err) => feeds_status_error(format!("初始化应用失败：{err}")),
        },
        FeedsCommand::RefreshFeed { feed_id, feed_title } => match UiServices::shared().await {
            Ok(services) => match services.feeds().refresh_feed(feed_id).await {
                Ok(outcome) => feeds_intents(vec![
                    FeedsPageIntent::SetStatus {
                        message: outcome.failure_message.as_ref().map_or_else(
                            || format!("已刷新订阅：{feed_title}"),
                            |failure| format!("刷新订阅失败：{failure}"),
                        ),
                        tone: if outcome.failure_message.is_some() {
                            "error".to_string()
                        } else {
                            "info".to_string()
                        },
                    },
                    FeedsPageIntent::BumpReload,
                ]),
                Err(err) => feeds_status_error(format!("刷新订阅失败：{err}")),
            },
            Err(err) => feeds_status_error(format!("初始化应用失败：{err}")),
        },
        FeedsCommand::RemoveFeed { feed_id, feed_title, confirmed } => {
            if !confirmed {
                return feeds_intents(vec![
                    FeedsPageIntent::PendingDeleteFeedSet(Some(feed_id)),
                    FeedsPageIntent::SetStatus {
                        message: format!("再次点击即可删除订阅：{feed_title}"),
                        tone: "info".to_string(),
                    },
                ]);
            }

            match UiServices::shared().await {
                Ok(services) => match services.feeds().remove_feed(feed_id).await {
                    Ok(()) => feeds_intents(vec![
                        FeedsPageIntent::PendingDeleteFeedSet(None),
                        FeedsPageIntent::SetStatus {
                            message: format!("已删除订阅：{feed_title}"),
                            tone: "info".to_string(),
                        },
                        FeedsPageIntent::BumpReload,
                    ]),
                    Err(err) => feeds_intents(vec![
                        FeedsPageIntent::PendingDeleteFeedSet(None),
                        FeedsPageIntent::SetStatus {
                            message: format!("删除订阅失败：{err}"),
                            tone: "error".to_string(),
                        },
                    ]),
                },
                Err(err) => feeds_intents(vec![
                    FeedsPageIntent::PendingDeleteFeedSet(None),
                    FeedsPageIntent::SetStatus {
                        message: format!("初始化应用失败：{err}"),
                        tone: "error".to_string(),
                    },
                ]),
            }
        }
        FeedsCommand::ExportConfig => match UiServices::shared().await {
            Ok(services) => match services.feeds().export_config_json().await {
                Ok(raw) => feeds_intents(vec![
                    FeedsPageIntent::ConfigTextExported(raw),
                    FeedsPageIntent::SetStatus {
                        message: "已导出配置包 JSON。".to_string(),
                        tone: "info".to_string(),
                    },
                ]),
                Err(err) => feeds_status_error(format!("导出配置包失败：{err}")),
            },
            Err(err) => feeds_status_error(format!("初始化应用失败：{err}")),
        },
        FeedsCommand::ImportConfig { raw, confirmed } => {
            if !confirmed {
                return feeds_intents(vec![
                    FeedsPageIntent::PendingConfigImportSet(true),
                    FeedsPageIntent::SetStatus {
                        message: "导入配置会按配置包覆盖当前订阅集合，并清理缺失订阅的本地文章；再次点击才会执行。"
                            .to_string(),
                        tone: "info".to_string(),
                    },
                ]);
            }

            match UiServices::shared().await {
                Ok(services) => match services.feeds().import_config_json(&raw).await {
                    Ok(outcome) => feeds_intents(vec![
                        FeedsPageIntent::PendingConfigImportSet(false),
                        FeedsPageIntent::SetStatus {
                            message: format!("配置包已导入：{}。", config_import_summary(&outcome)),
                            tone: "info".to_string(),
                        },
                        FeedsPageIntent::BumpReload,
                    ]),
                    Err(err) => feeds_intents(vec![
                        FeedsPageIntent::PendingConfigImportSet(false),
                        FeedsPageIntent::SetStatus {
                            message: format!("导入配置包失败：{err}"),
                            tone: "error".to_string(),
                        },
                    ]),
                },
                Err(err) => feeds_intents(vec![
                    FeedsPageIntent::PendingConfigImportSet(false),
                    FeedsPageIntent::SetStatus {
                        message: format!("初始化应用失败：{err}"),
                        tone: "error".to_string(),
                    },
                ]),
            }
        }
        FeedsCommand::ExportOpml => match UiServices::shared().await {
            Ok(services) => match services.feeds().export_opml().await {
                Ok(raw) => feeds_intents(vec![
                    FeedsPageIntent::OpmlTextExported(raw),
                    FeedsPageIntent::SetStatus {
                        message: "已导出 OPML。".to_string(),
                        tone: "info".to_string(),
                    },
                ]),
                Err(err) => feeds_status_error(format!("导出 OPML 失败：{err}")),
            },
            Err(err) => feeds_status_error(format!("初始化应用失败：{err}")),
        },
        FeedsCommand::ImportOpml { raw } => match UiServices::shared().await {
            Ok(services) => match services.feeds().import_opml(&raw).await {
                Ok(outcome) => feeds_intents(vec![
                    FeedsPageIntent::SetStatus {
                        message: format!("OPML 已导入：{}。", opml_import_summary(&outcome)),
                        tone: "info".to_string(),
                    },
                    FeedsPageIntent::BumpReload,
                ]),
                Err(err) => feeds_status_error(format!("导入 OPML 失败：{err}")),
            },
            Err(err) => feeds_status_error(format!("初始化应用失败：{err}")),
        },
        FeedsCommand::ReadFeedUrlFromClipboard => match read_feed_url_from_clipboard().await {
            Ok(Some(text)) => feeds_intents(vec![FeedsPageIntent::FeedUrlChanged(text)]),
            Ok(None) => Vec::new(),
            Err(err) => feeds_status_error(format!("读取系统剪贴板失败：{err}")),
        },
    }
}

fn feeds_intents(intents: Vec<FeedsPageIntent>) -> Vec<UiIntent> {
    intents.into_iter().map(UiIntent::FeedsPage).collect()
}

fn feeds_status_error(message: impl Into<String>) -> Vec<UiIntent> {
    feeds_intents(vec![FeedsPageIntent::SetStatus {
        message: message.into(),
        tone: "error".to_string(),
    }])
}

fn config_import_summary(outcome: &ConfigImportOutcome) -> String {
    let settings = if outcome.settings_updated { "设置已更新" } else { "设置未变化" };
    format!(
        "导入 {} 个订阅，清理 {} 个缺失订阅，{settings}",
        outcome.imported_feed_count, outcome.removed_feed_count
    )
}

fn opml_import_summary(outcome: &OpmlImportOutcome) -> String {
    format!("{} 个订阅", outcome.imported_feed_count)
}

#[cfg(target_arch = "wasm32")]
async fn read_feed_url_from_clipboard() -> Result<Option<String>, String> {
    document::eval(
        r#"
        if (typeof navigator === "undefined" || !navigator.clipboard || !navigator.clipboard.readText) {
            return null;
        }
        return navigator.clipboard.readText();
        "#,
    )
    .join::<Option<String>>()
    .await
    .map_err(|err| err.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
async fn read_feed_url_from_clipboard() -> Result<Option<String>, String> {
    Err("当前平台不支持从系统剪贴板读取订阅地址".to_string())
}

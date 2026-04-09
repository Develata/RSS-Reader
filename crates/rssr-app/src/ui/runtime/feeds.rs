use crate::{
    bootstrap::AppServices,
    pages::feeds_page::intent::{FeedsPageIntent, FeedsPageSnapshot},
    ui::{commands::FeedsCommand, snapshot::UiIntent},
};
use anyhow::Context;
#[cfg(target_arch = "wasm32")]
use dioxus::prelude::document;
use rssr_domain::EntryQuery;

pub(super) async fn execute(command: FeedsCommand) -> Vec<UiIntent> {
    match command {
        FeedsCommand::LoadSnapshot => match AppServices::shared().await {
            Ok(services) => {
                let result: anyhow::Result<FeedsPageSnapshot> = async {
                    let feeds = services.list_feeds().await.context("读取订阅失败")?;
                    let entry_count = services
                        .list_entries(&EntryQuery::default())
                        .await
                        .context("读取文章统计失败")?
                        .len();
                    Ok(FeedsPageSnapshot { feed_count: feeds.len(), entry_count, feeds })
                }
                .await;
                feeds_intents(vec![FeedsPageIntent::SnapshotLoaded(
                    result.map_err(|err| err.to_string()),
                )])
            }
            Err(err) => feeds_status_error(format!("初始化应用失败：{err}")),
        },
        FeedsCommand::AddFeed { raw_url } => match AppServices::shared().await {
            Ok(services) => match services.add_subscription(&raw_url).await {
                Ok(()) => feeds_intents(vec![
                    FeedsPageIntent::FeedUrlChanged(String::new()),
                    FeedsPageIntent::SetStatus {
                        message: "订阅已保存并完成首次刷新。".to_string(),
                        tone: "info".to_string(),
                    },
                    FeedsPageIntent::BumpReload,
                ]),
                Err(err) if err.to_string().contains("首次刷新订阅失败") => {
                    feeds_intents(vec![
                        FeedsPageIntent::FeedUrlChanged(String::new()),
                        FeedsPageIntent::SetStatus {
                            message: format!("订阅已保存，但首次刷新失败：{err}"),
                            tone: "error".to_string(),
                        },
                        FeedsPageIntent::BumpReload,
                    ])
                }
                Err(err) => feeds_status_error(format!("保存订阅失败：{err}")),
            },
            Err(err) => feeds_status_error(format!("初始化应用失败：{err}")),
        },
        FeedsCommand::RefreshAll => match AppServices::shared().await {
            Ok(services) => match services.refresh_all().await {
                Ok(_) => feeds_intents(vec![
                    FeedsPageIntent::SetStatus {
                        message: "刷新完成。".to_string(),
                        tone: "info".to_string(),
                    },
                    FeedsPageIntent::BumpReload,
                ]),
                Err(err) => feeds_status_error(format!("刷新失败：{err}")),
            },
            Err(err) => feeds_status_error(format!("初始化应用失败：{err}")),
        },
        FeedsCommand::RefreshFeed { feed_id, feed_title } => match AppServices::shared().await {
            Ok(services) => match services.refresh_feed(feed_id).await {
                Ok(_) => feeds_intents(vec![
                    FeedsPageIntent::SetStatus {
                        message: format!("已刷新订阅：{feed_title}"),
                        tone: "info".to_string(),
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

            match AppServices::shared().await {
                Ok(services) => match services.remove_feed(feed_id).await {
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
        FeedsCommand::ExportConfig => match AppServices::shared().await {
            Ok(services) => match services.export_config_json().await {
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

            match AppServices::shared().await {
                Ok(services) => match services.import_config_json(&raw).await {
                    Ok(()) => feeds_intents(vec![
                        FeedsPageIntent::PendingConfigImportSet(false),
                        FeedsPageIntent::SetStatus {
                            message: "配置包已导入。".to_string(),
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
        FeedsCommand::ExportOpml => match AppServices::shared().await {
            Ok(services) => match services.export_opml().await {
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
        FeedsCommand::ImportOpml { raw } => match AppServices::shared().await {
            Ok(services) => match services.import_opml(&raw).await {
                Ok(()) => feeds_intents(vec![
                    FeedsPageIntent::SetStatus {
                        message: "OPML 已导入。".to_string(),
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

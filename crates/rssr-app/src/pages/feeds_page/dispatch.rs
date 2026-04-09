use std::sync::Arc;

use crate::bootstrap::AppServices;

use super::{commands::FeedsPageCommand, intent::FeedsPageIntent};

pub(crate) async fn execute_command(command: FeedsPageCommand) -> Vec<FeedsPageIntent> {
    match command {
        FeedsPageCommand::AddFeed { raw_url } => add_feed(raw_url).await,
        FeedsPageCommand::RefreshAll => {
            run_service_command(
                |services| async move {
                    services.refresh_all().await.map(|_| "刷新完成。".to_string())
                },
                "刷新失败",
            )
            .await
        }
        FeedsPageCommand::RefreshFeed { feed_id, feed_title } => {
            run_service_command(
                move |services| async move {
                    services
                        .refresh_feed(feed_id)
                        .await
                        .map(|_| format!("已刷新订阅：{feed_title}"))
                },
                "刷新订阅失败",
            )
            .await
        }
        FeedsPageCommand::RemoveFeed { feed_id, feed_title, confirmed } => {
            remove_feed(feed_id, feed_title, confirmed).await
        }
        FeedsPageCommand::ExportConfig => export_config().await,
        FeedsPageCommand::ImportConfig { raw, confirmed } => import_config(raw, confirmed).await,
        FeedsPageCommand::ExportOpml => export_opml().await,
        FeedsPageCommand::ImportOpml { raw } => import_opml(raw).await,
    }
}

async fn add_feed(raw_url: String) -> Vec<FeedsPageIntent> {
    match shared_services().await {
        Ok(services) => match services.add_subscription(&raw_url).await {
            Ok(()) => vec![
                FeedsPageIntent::FeedUrlChanged(String::new()),
                status_info("订阅已保存并完成首次刷新。"),
                FeedsPageIntent::BumpReload,
            ],
            Err(err) if err.to_string().contains("首次刷新订阅失败") => vec![
                FeedsPageIntent::FeedUrlChanged(String::new()),
                status_error(format!("订阅已保存，但首次刷新失败：{err}")),
                FeedsPageIntent::BumpReload,
            ],
            Err(err) => vec![status_error(format!("保存订阅失败：{err}"))],
        },
        Err(err) => vec![status_error(err)],
    }
}

async fn remove_feed(feed_id: i64, feed_title: String, confirmed: bool) -> Vec<FeedsPageIntent> {
    if !confirmed {
        return vec![
            FeedsPageIntent::PendingDeleteFeedSet(Some(feed_id)),
            status_info(format!("再次点击即可删除订阅：{feed_title}")),
        ];
    }

    match shared_services().await {
        Ok(services) => match services.remove_feed(feed_id).await {
            Ok(()) => vec![
                FeedsPageIntent::PendingDeleteFeedSet(None),
                status_info(format!("已删除订阅：{feed_title}")),
                FeedsPageIntent::BumpReload,
            ],
            Err(err) => vec![
                FeedsPageIntent::PendingDeleteFeedSet(None),
                status_error(format!("删除订阅失败：{err}")),
            ],
        },
        Err(err) => vec![FeedsPageIntent::PendingDeleteFeedSet(None), status_error(err)],
    }
}

async fn export_config() -> Vec<FeedsPageIntent> {
    match shared_services().await {
        Ok(services) => match services.export_config_json().await {
            Ok(raw) => {
                vec![FeedsPageIntent::ConfigTextExported(raw), status_info("已导出配置包 JSON。")]
            }
            Err(err) => vec![status_error(format!("导出配置包失败：{err}"))],
        },
        Err(err) => vec![status_error(err)],
    }
}

async fn import_config(raw: String, confirmed: bool) -> Vec<FeedsPageIntent> {
    if !confirmed {
        return vec![
            FeedsPageIntent::PendingConfigImportSet(true),
            status_info(
                "导入配置会按配置包覆盖当前订阅集合，并清理缺失订阅的本地文章；再次点击才会执行。",
            ),
        ];
    }

    match shared_services().await {
        Ok(services) => match services.import_config_json(&raw).await {
            Ok(()) => vec![
                FeedsPageIntent::PendingConfigImportSet(false),
                status_info("配置包已导入。"),
                FeedsPageIntent::BumpReload,
            ],
            Err(err) => vec![
                FeedsPageIntent::PendingConfigImportSet(false),
                status_error(format!("导入配置包失败：{err}")),
            ],
        },
        Err(err) => vec![FeedsPageIntent::PendingConfigImportSet(false), status_error(err)],
    }
}

async fn export_opml() -> Vec<FeedsPageIntent> {
    match shared_services().await {
        Ok(services) => match services.export_opml().await {
            Ok(raw) => vec![FeedsPageIntent::OpmlTextExported(raw), status_info("已导出 OPML。")],
            Err(err) => vec![status_error(format!("导出 OPML 失败：{err}"))],
        },
        Err(err) => vec![status_error(err)],
    }
}

async fn import_opml(raw: String) -> Vec<FeedsPageIntent> {
    match shared_services().await {
        Ok(services) => match services.import_opml(&raw).await {
            Ok(()) => vec![status_info("OPML 已导入。"), FeedsPageIntent::BumpReload],
            Err(err) => vec![status_error(format!("导入 OPML 失败：{err}"))],
        },
        Err(err) => vec![status_error(err)],
    }
}

async fn run_service_command<F, Fut>(run: F, error_prefix: &str) -> Vec<FeedsPageIntent>
where
    F: FnOnce(Arc<AppServices>) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<String>>,
{
    match shared_services().await {
        Ok(services) => match run(services).await {
            Ok(message) => vec![status_info(message), FeedsPageIntent::BumpReload],
            Err(err) => vec![status_error(format!("{error_prefix}：{err}"))],
        },
        Err(err) => vec![status_error(err)],
    }
}

async fn shared_services() -> Result<Arc<AppServices>, String> {
    AppServices::shared().await.map_err(|err| format!("初始化应用失败：{err}"))
}

fn status_info(message: impl Into<String>) -> FeedsPageIntent {
    FeedsPageIntent::SetStatus { message: message.into(), tone: "info".to_string() }
}

fn status_error(message: impl Into<String>) -> FeedsPageIntent {
    FeedsPageIntent::SetStatus { message: message.into(), tone: "error".to_string() }
}

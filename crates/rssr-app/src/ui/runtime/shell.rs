use crate::{
    router::AppRoute,
    ui::{
        commands::ShellCommand,
        runtime::services::UiServices,
        snapshot::{AuthenticatedShellSnapshot, StartupRouteSnapshot, UiIntent},
    },
};
use rssr_application::StartupTarget;

pub(super) async fn execute(command: ShellCommand) -> Vec<UiIntent> {
    match command {
        ShellCommand::ManualRefresh => match UiServices::shared().await {
            Ok(services) => match services.feeds().refresh_all().await {
                Ok(outcome) => vec![UiIntent::SetStatus {
                    message: refresh_message(&outcome),
                    tone: if outcome.failure_message.is_some() { "error" } else { "info" }
                        .to_string(),
                }],
                Err(err) => status_error(format!("刷新失败：{err}")),
            },
            Err(err) => status_error(format!("初始化应用失败：{err}")),
        },
        ShellCommand::LoadAuthenticatedShell => match UiServices::shared().await {
            Ok(services) => {
                let shell = services.shell();
                let settings = match shell.load_authenticated_shell().await {
                    Ok(settings) => settings,
                    Err(err) => return status_error(format!("读取设置失败：{err}")),
                };
                shell.ensure_auto_refresh_started();
                vec![UiIntent::AuthenticatedShellLoaded(AuthenticatedShellSnapshot { settings })]
            }
            Err(err) => status_error(format!("初始化应用失败：{err}")),
        },
        ShellCommand::ResolveStartupRoute => match UiServices::shared().await {
            Ok(services) => {
                let shell = services.shell();
                let target = match shell.resolve_startup_target().await {
                    Ok(target) => target,
                    Err(err) => return resolve_with_fallback(format!("解析启动页面失败：{err}")),
                };

                let route = match target {
                    StartupTarget::AllEntries => AppRoute::EntriesPage {},
                    StartupTarget::FeedEntries { feed_id } => AppRoute::FeedEntriesPage { feed_id },
                };

                vec![UiIntent::StartupRouteResolved(StartupRouteSnapshot { route })]
            }
            Err(err) => resolve_with_fallback(format!("初始化应用失败：{err}")),
        },
    }
}

fn status_error(message: impl Into<String>) -> Vec<UiIntent> {
    vec![UiIntent::SetStatus { message: message.into(), tone: "error".to_string() }]
}

fn resolve_with_fallback(message: impl Into<String>) -> Vec<UiIntent> {
    vec![
        UiIntent::SetStatus { message: message.into(), tone: "error".to_string() },
        UiIntent::StartupRouteResolved(StartupRouteSnapshot { route: AppRoute::EntriesPage {} }),
    ]
}

fn refresh_message(outcome: &crate::bootstrap::RefreshAllExecutionOutcome) -> String {
    let success = super::new_entries_message(outcome.inserted_count);
    match &outcome.failure_message {
        Some(failure) if outcome.total_count > 0 && outcome.failed_count == outcome.total_count => {
            format!("刷新失败：{failure}")
        }
        Some(failure) => format!("{success}，部分订阅失败：{failure}"),
        None => format!("{success}。"),
    }
}

use crate::{
    router::AppRoute,
    ui::{
        commands::ShellCommand,
        runtime::backend::backend,
        snapshot::{AuthenticatedShellSnapshot, StartupRouteSnapshot, UiIntent},
    },
};
use rssr_application::StartupTarget;

pub(super) async fn execute(command: ShellCommand) -> Vec<UiIntent> {
    match command {
        ShellCommand::ManualRefresh => match backend().await {
            Ok(services) => match services.refresh_all().await {
                Ok(outcome) => vec![UiIntent::SetStatus {
                    message: refresh_message(&outcome),
                    tone: if outcome.failed_count == 0 { "info" } else { "error" }.to_string(),
                }],
                Err(err) => status_error(format!("刷新失败：{err}")),
            },
            Err(err) => status_error(format!("初始化应用失败：{err}")),
        },
        ShellCommand::LoadAuthenticatedShell => match backend().await {
            Ok(services) => {
                let shell = &services;
                let settings = match shell.load_settings().await {
                    Ok(settings) => settings,
                    Err(err) => return status_error(format!("读取设置失败：{err}")),
                };
                shell.ensure_auto_refresh_started();
                vec![UiIntent::AuthenticatedShellLoaded(AuthenticatedShellSnapshot { settings })]
            }
            Err(err) => status_error(format!("初始化应用失败：{err}")),
        },
        ShellCommand::ResolveStartupRoute => match backend().await {
            Ok(services) => {
                let shell = &services;
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

fn refresh_message(outcome: &rssr_application::RefreshAllExecutionOutcome) -> String {
    let success = super::new_entries_message(outcome.inserted_count);
    if outcome.failed_count == 0 {
        return format!("{success}。");
    }
    // The result lists a bounded number of failures; `failed_count` is the full count.
    let unlisted = outcome.failed_count.saturating_sub(outcome.failures.len() as u64);
    let failure = match super::failure_lines(&outcome.failures) {
        Some(lines) if unlisted > 0 => format!("{lines}（另有 {unlisted} 个未列出）"),
        Some(lines) => lines,
        None => format!("{} 个订阅失败（明细过多，未列出）", outcome.failed_count),
    };
    if outcome.total_count > 0 && outcome.failed_count == outcome.total_count {
        format!("刷新失败：{failure}")
    } else {
        format!("{success}，部分订阅失败：{failure}")
    }
}

#[cfg(test)]
mod tests {
    use rssr_application::{ErrorKind, RefreshAllExecutionOutcome, RefreshFailureItem};

    use super::refresh_message;

    fn outcome(
        total: u64,
        failed: u64,
        listed: usize,
        truncated: bool,
    ) -> RefreshAllExecutionOutcome {
        RefreshAllExecutionOutcome {
            inserted_count: 2,
            total_count: total,
            failed_count: failed,
            failures: (0..listed)
                .map(|index| RefreshFailureItem {
                    feed_id: index as i64,
                    url: format!("https://example.com/{index}.xml"),
                    name: Some(format!("订阅{index}")),
                    kind: ErrorKind::Network,
                    message: "HTTP 500".into(),
                })
                .collect(),
            failures_truncated: truncated,
        }
    }

    /// The status line counts every failure, listed or not (the result lists a bounded number).
    #[test]
    fn unlisted_failures_are_still_reported() {
        assert_eq!(refresh_message(&outcome(5, 0, 0, false)), "新增 2 篇文章。");
        assert_eq!(
            refresh_message(&outcome(5, 1, 1, false)),
            "新增 2 篇文章，部分订阅失败：订阅0: HTTP 500"
        );
        assert_eq!(
            refresh_message(&outcome(500, 300, 1, true)),
            "新增 2 篇文章，部分订阅失败：订阅0: HTTP 500（另有 299 个未列出）"
        );
        assert_eq!(
            refresh_message(&outcome(300, 300, 0, true)),
            "刷新失败：300 个订阅失败（明细过多，未列出）"
        );
    }
}

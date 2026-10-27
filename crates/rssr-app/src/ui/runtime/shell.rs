use crate::{
    router::AppRoute,
    ui::{
        commands::ShellCommand,
        failure::UiFailure,
        runtime::backend::backend,
        snapshot::{AuthenticatedShellSnapshot, StartupRouteSnapshot, UiIntent},
    },
};
use rssr_application::StartupTarget;

pub(super) async fn execute(command: ShellCommand) -> Vec<UiIntent> {
    match command {
        ShellCommand::ManualRefresh => match async { backend().await?.refresh_all().await }.await {
            Ok(outcome) => vec![UiIntent::SetStatus {
                message: refresh_message(&outcome),
                tone: if outcome.failed_count == 0 { "info" } else { "error" }.to_string(),
            }],
            Err(error) => vec![UiIntent::Failed(UiFailure::new("刷新", error))],
        },
        ShellCommand::LoadAuthenticatedShell => {
            let mut started = false;
            let loaded = async {
                let backend = backend().await?;
                // Before the read: a failed settings read must not leave the periodic refresh
                // unstarted (review C4B-A-03).
                backend.ensure_auto_refresh_started();
                started = true;
                backend.load_settings().await
            }
            .await;
            let mut intents = Vec::new();
            if started {
                intents.push(UiIntent::AutoRefreshStarted);
            }
            intents.push(match loaded {
                Ok(settings) => {
                    UiIntent::AuthenticatedShellLoaded(AuthenticatedShellSnapshot { settings })
                }
                Err(error) => UiIntent::Failed(UiFailure::new("读取设置", error)),
            });
            intents
        }
        ShellCommand::LoadSettings => {
            match async { backend().await?.load_settings().await }.await {
                Ok(settings) => {
                    vec![UiIntent::AuthenticatedShellLoaded(AuthenticatedShellSnapshot {
                        settings,
                    })]
                }
                Err(error) => vec![UiIntent::Failed(UiFailure::new("读取设置", error))],
            }
        }
        ShellCommand::ResolveStartupRoute => {
            match async { backend().await?.resolve_startup_target().await }.await {
                Ok(target) => {
                    let route = match target {
                        StartupTarget::AllEntries => AppRoute::EntriesPage {},
                        StartupTarget::FeedEntries { feed_id } => {
                            AppRoute::FeedEntriesPage { feed_id }
                        }
                    };
                    vec![UiIntent::StartupRouteResolved(StartupRouteSnapshot { route })]
                }
                Err(error) => vec![
                    UiIntent::Failed(UiFailure::new("解析启动页面", error)),
                    UiIntent::StartupRouteResolved(StartupRouteSnapshot {
                        route: AppRoute::EntriesPage {},
                    }),
                ],
            }
        }
    }
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

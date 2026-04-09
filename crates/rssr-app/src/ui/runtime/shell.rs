use crate::{
    bootstrap::AppServices,
    router::AppRoute,
    ui::{
        commands::ShellCommand,
        snapshot::{AuthenticatedShellSnapshot, StartupRouteSnapshot, UiIntent},
    },
};
use rssr_domain::StartupView;

pub(super) async fn execute(command: ShellCommand) -> Vec<UiIntent> {
    match command {
        ShellCommand::LoadAuthenticatedShell => match AppServices::shared().await {
            Ok(services) => {
                let settings = match services.load_settings().await {
                    Ok(settings) => settings,
                    Err(err) => return status_error(format!("读取设置失败：{err}")),
                };
                services.ensure_auto_refresh_started();
                vec![UiIntent::AuthenticatedShellLoaded(AuthenticatedShellSnapshot { settings })]
            }
            Err(err) => status_error(format!("初始化应用失败：{err}")),
        },
        ShellCommand::ResolveStartupRoute => match AppServices::shared().await {
            Ok(services) => {
                let settings = match services.load_settings().await {
                    Ok(settings) => settings,
                    Err(err) => return resolve_with_fallback(format!("读取设置失败：{err}")),
                };

                let route = match settings.startup_view {
                    StartupView::All => AppRoute::EntriesPage {},
                    StartupView::LastFeed => {
                        let last_feed_id = services.load_last_opened_feed_id().await.ok().flatten();
                        let feed_exists = match last_feed_id {
                            Some(feed_id) => services
                                .list_feeds()
                                .await
                                .map(|feeds| feeds.iter().any(|feed| feed.id == feed_id))
                                .unwrap_or(false),
                            None => false,
                        };

                        if let Some(feed_id) = last_feed_id.filter(|_| feed_exists) {
                            AppRoute::FeedEntriesPage { feed_id }
                        } else {
                            AppRoute::EntriesPage {}
                        }
                    }
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

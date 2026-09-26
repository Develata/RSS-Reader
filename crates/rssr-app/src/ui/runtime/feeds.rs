use crate::{
    bootstrap::AddSubscriptionOutcome,
    pages::feeds_page::intent::{FeedsPageIntent, FeedsPageSnapshot},
    ui::{commands::FeedsCommand, runtime::services::UiServices, snapshot::UiIntent},
};
use rssr_application::OpmlImportOutcome;

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
        FeedsCommand::AddFeed { raw_url, fallback_site_url } => {
            let result = match UiServices::shared().await {
                Ok(services) => services
                    .feeds()
                    .add_subscription(&raw_url, fallback_site_url)
                    .await
                    .map_err(|err| format!("{err:#}")),
                Err(err) => Err(format!("初始化应用失败：{err}")),
            };
            add_feed_result(result)
        }
        FeedsCommand::RefreshFeed { feed_id, feed_title } => match UiServices::shared().await {
            Ok(services) => match services.feeds().refresh_feed(feed_id).await {
                Ok(outcome) => feeds_intents(vec![
                    FeedsPageIntent::SetStatus {
                        message: outcome.failure_message.as_ref().map_or_else(
                            || {
                                format!(
                                    "{}：{feed_title}",
                                    super::new_entries_message(outcome.inserted_count)
                                )
                            },
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
        FeedsCommand::RemoveFeed { feed_id, feed_title } => match UiServices::shared().await {
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
        },
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
        FeedsCommand::ImportConfig { raw } => match UiServices::shared().await {
            Ok(services) => match services.feeds().import_config_json(&raw).await {
                Ok(outcome) => feeds_intents(vec![
                    FeedsPageIntent::PendingConfigImportSet(false),
                    FeedsPageIntent::SetStatus {
                        message: format!("配置包已导入：{}。", outcome.summary_line()),
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
        },
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

fn opml_import_summary(outcome: &OpmlImportOutcome) -> String {
    format!("{} 个订阅", outcome.imported_feed_count)
}

fn add_feed_result(result: Result<AddSubscriptionOutcome, String>) -> Vec<UiIntent> {
    let result = match result {
        Ok(AddSubscriptionOutcome::NeedsSelection { page_url, candidates }) => {
            return feeds_intents(vec![
                FeedsPageIntent::FeedCandidates { page_url, candidates },
                FeedsPageIntent::AddFeedFinished { saved: false },
            ]);
        }
        result => result,
    };
    let (saved, message, tone) = match result {
        Ok(AddSubscriptionOutcome::NeedsSelection { .. }) => unreachable!(),
        Ok(AddSubscriptionOutcome::SavedAndRefreshed) => {
            (true, "订阅已保存并完成首次刷新。".to_string(), "info")
        }
        Ok(AddSubscriptionOutcome::SavedRefreshFailed { message }) => {
            (true, format!("订阅已保存，但首次刷新失败：{message}"), "error")
        }
        Err(message) => (false, message, "error"),
    };
    let mut intents = vec![
        FeedsPageIntent::AddFeedFinished { saved },
        FeedsPageIntent::SetStatus { message, tone: tone.to_string() },
    ];
    if saved {
        intents.push(FeedsPageIntent::BumpReload);
    }
    feeds_intents(intents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_add_outcome_releases_gate_and_only_saved_subscriptions_reload() {
        for (result, expected_saved, expected_tone) in [
            (Ok(AddSubscriptionOutcome::SavedAndRefreshed), true, "info"),
            (
                Ok(AddSubscriptionOutcome::SavedRefreshFailed { message: "offline".into() }),
                true,
                "error",
            ),
            (Err("invalid feed URL".into()), false, "error"),
        ] {
            let intents = add_feed_result(result)
                .into_iter()
                .filter_map(UiIntent::into_feeds_page_intent)
                .collect::<Vec<_>>();
            assert!(
                matches!(intents.first(), Some(FeedsPageIntent::AddFeedFinished { saved }) if *saved == expected_saved)
            );
            assert!(
                matches!(&intents[1], FeedsPageIntent::SetStatus { tone, .. } if tone == expected_tone)
            );
            assert_eq!(
                intents.iter().any(|intent| matches!(intent, FeedsPageIntent::BumpReload)),
                expected_saved
            );
        }
    }
}

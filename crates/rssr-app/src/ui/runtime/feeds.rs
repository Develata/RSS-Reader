use crate::{
    bootstrap::AddSubscriptionOutcome,
    pages::feeds_page::intent::{FeedsPageIntent, FeedsPageSnapshot},
    ui::{commands::FeedsCommand, runtime::backend::backend, snapshot::UiIntent},
};
use rssr_api::ErrorCode;
use rssr_application::{ConfigImportOutcome, OpmlImportOutcome};

pub(super) async fn execute(command: FeedsCommand) -> Vec<UiIntent> {
    match command {
        FeedsCommand::LoadSnapshot => match backend().await {
            Ok(services) => match services.load_feeds_snapshot().await {
                Ok(outcome) => {
                    feeds_intents(vec![FeedsPageIntent::SnapshotLoaded(Ok(FeedsPageSnapshot {
                        feeds: outcome.feeds,
                        feed_count: outcome.feed_count,
                        entry_count: outcome.entry_count,
                        fingerprint: outcome.fingerprint,
                    }))])
                }
                Err(err) => {
                    feeds_intents(vec![FeedsPageIntent::SnapshotLoaded(Err(err.to_string()))])
                }
            },
            Err(err) => feeds_status_error(format!("初始化应用失败：{err}")),
        },
        FeedsCommand::AddFeed { raw_url, fallback_site_url } => {
            let result = match backend().await {
                Ok(services) => services
                    .add_subscription(&raw_url, fallback_site_url)
                    .await
                    .map_err(|err| format!("{err:#}")),
                Err(err) => Err(format!("初始化应用失败：{err}")),
            };
            add_feed_result(result)
        }
        FeedsCommand::RefreshFeed { feed_id, feed_title } => match backend().await {
            Ok(services) => match services.refresh_feed(feed_id).await {
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
        FeedsCommand::RemoveFeed { feed_id, feed_title } => match backend().await {
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
        },
        FeedsCommand::ExportConfig => match backend().await {
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
        FeedsCommand::ImportConfig { raw, expected } => {
            let result = match backend().await {
                Ok(services) => match services.import_config_json(&raw, &expected).await {
                    Ok(outcome) => ConfigImportResult::Imported(outcome),
                    Err(err) if err.code == ErrorCode::Conflict => ConfigImportResult::Conflict,
                    Err(err) => ConfigImportResult::Failed(format!("导入配置包失败：{err}")),
                },
                Err(err) => ConfigImportResult::Failed(format!("初始化应用失败：{err}")),
            };
            import_config_result(result)
        }
        FeedsCommand::ExportOpml => match backend().await {
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
        FeedsCommand::ImportOpml { raw } => match backend().await {
            Ok(services) => match services.import_opml(&raw).await {
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

/// `settings` 只在配置包更新了设置时存在。读回失败不影响导入本身已经提交的结果。
/// What a configuration import led to.
enum ConfigImportResult {
    Imported(ConfigImportOutcome),
    /// The configuration changed after the page read it; nothing was replaced.
    Conflict,
    Failed(String),
}

fn import_config_result(result: ConfigImportResult) -> Vec<UiIntent> {
    let mut intents = vec![FeedsPageIntent::PendingConfigImportSet(false)];
    let (message, tone) = match result {
        ConfigImportResult::Imported(outcome) => {
            let summary = outcome.summary_line();
            intents.push(FeedsPageIntent::BumpReload);
            // 配置包可以替换外观设置：替换提交的设置随结果返回，交给全局主题立即生效。
            if outcome.settings_updated {
                intents.push(FeedsPageIntent::SettingsReplaced(outcome.settings));
            }
            (format!("配置包已导入：{summary}。"), "info")
        }
        ConfigImportResult::Conflict => {
            intents.push(FeedsPageIntent::BumpReload);
            (
                "订阅或设置在本页载入后已被修改，配置包未导入；已重新载入，请确认后再导入。"
                    .to_string(),
                "error",
            )
        }
        ConfigImportResult::Failed(message) => (message, "error"),
    };
    intents.push(FeedsPageIntent::SetStatus { message, tone: tone.to_string() });
    feeds_intents(intents)
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

    /// The committed settings from the outcome go to the shell when they changed; a conflict
    /// reloads the page and says so; other failures leave the page as it is.
    #[test]
    fn config_import_hands_replaced_settings_to_the_shell() {
        let replaced = rssr_domain::UserSettings {
            custom_css: "body { color: red; }".into(),
            ..rssr_domain::UserSettings::default()
        };
        let outcome = |settings_updated| ConfigImportOutcome {
            imported_feed_count: 1,
            removed_feed_count: 0,
            settings_updated,
            settings: replaced.clone(),
        };
        for (result, expected_settings, expected_tone, reloads) in [
            (ConfigImportResult::Imported(outcome(true)), Some(&replaced), "info", true),
            (ConfigImportResult::Imported(outcome(false)), None, "info", true),
            (ConfigImportResult::Conflict, None, "error", true),
            (ConfigImportResult::Failed("导入配置包失败：bad json".into()), None, "error", false),
        ] {
            let intents = import_config_result(result)
                .into_iter()
                .filter_map(UiIntent::into_feeds_page_intent)
                .collect::<Vec<_>>();
            assert!(matches!(intents[0], FeedsPageIntent::PendingConfigImportSet(false)));
            let settings = intents.iter().find_map(|intent| match intent {
                FeedsPageIntent::SettingsReplaced(settings) => Some(settings),
                _ => None,
            });
            assert_eq!(settings, expected_settings);
            assert!(matches!(
                intents.last(),
                Some(FeedsPageIntent::SetStatus { tone, .. }) if tone == expected_tone
            ));
            assert_eq!(
                intents.iter().any(|intent| matches!(intent, FeedsPageIntent::BumpReload)),
                reloads
            );
        }
    }
}

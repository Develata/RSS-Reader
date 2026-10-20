use crate::{
    bootstrap::AddSubscriptionOutcome,
    pages::feeds_page::intent::{FeedsPageIntent, FeedsPageSnapshot},
    ui::{
        commands::FeedsCommand, failure::UiFailure, runtime::backend::backend, snapshot::UiIntent,
    },
};
use rssr_application::{ConfigImportOutcome, OpmlImportOutcome};

pub(super) async fn execute(command: FeedsCommand) -> Vec<UiIntent> {
    match command {
        FeedsCommand::LoadSnapshot => {
            match async { backend().await?.load_feeds_snapshot().await }.await {
                Ok(outcome) => {
                    feeds_intents(vec![FeedsPageIntent::SnapshotLoaded(Ok(FeedsPageSnapshot {
                        feeds: outcome.feeds,
                        feed_count: outcome.feed_count,
                        entry_count: outcome.entry_count,
                        fingerprint: outcome.fingerprint,
                    }))])
                }
                Err(error) => feeds_intents(vec![FeedsPageIntent::SnapshotLoaded(Err(
                    UiFailure::new("读取订阅", error),
                ))]),
            }
        }
        FeedsCommand::AddFeed { raw_url, fallback_site_url, token } => {
            let result =
                async { backend().await?.add_subscription(&raw_url, fallback_site_url).await }
                    .await
                    .map_err(|error| UiFailure::new("添加订阅", error));
            add_feed_result(token, result)
        }
        FeedsCommand::RefreshFeed { feed_id, feed_title } => {
            match async { backend().await?.refresh_feed(feed_id).await }.await {
                Ok(outcome) => feeds_intents(vec![FeedsPageIntent::SetStatus {
                    message: outcome.failure.as_ref().map_or_else(
                        || {
                            format!(
                                "{}：{feed_title}",
                                super::new_entries_message(outcome.inserted_count)
                            )
                        },
                        |failure| format!("刷新订阅失败：{}", super::failure_line(failure)),
                    ),
                    tone: if outcome.failure.is_some() {
                        "error".to_string()
                    } else {
                        "info".to_string()
                    },
                }]),
                Err(error) => feeds_failed("刷新订阅", error),
            }
        }
        FeedsCommand::RemoveFeed { feed_id, feed_title } => {
            match async { backend().await?.remove_feed(feed_id).await }.await {
                Ok(()) => feeds_intents(vec![
                    FeedsPageIntent::PendingDeleteFeedSet(None),
                    FeedsPageIntent::SetStatus {
                        message: format!("已删除订阅：{feed_title}"),
                        tone: "info".to_string(),
                    },
                    FeedsPageIntent::BumpReload,
                ]),
                Err(error) => feeds_intents(vec![
                    FeedsPageIntent::PendingDeleteFeedSet(None),
                    FeedsPageIntent::Failed(UiFailure::new("删除订阅", error)),
                ]),
            }
        }
        FeedsCommand::ExportConfig => {
            match async { backend().await?.export_config_json().await }.await {
                Ok(raw) => feeds_intents(vec![
                    FeedsPageIntent::ConfigTextExported(raw),
                    FeedsPageIntent::SetStatus {
                        message: "已导出配置包 JSON。".to_string(),
                        tone: "info".to_string(),
                    },
                ]),
                Err(error) => feeds_failed("导出配置包", error),
            }
        }
        FeedsCommand::ImportConfig { raw, expected } => {
            let result =
                match async { backend().await?.import_config_json(&raw, &expected).await }.await {
                    Ok(outcome) => ConfigImportResult::Imported(outcome),
                    Err(error) => {
                        let failure = UiFailure::new("导入配置包", error);
                        if failure.is_conflict() {
                            ConfigImportResult::Conflict
                        } else {
                            ConfigImportResult::Failed(failure)
                        }
                    }
                };
            import_config_result(result)
        }
        FeedsCommand::ExportOpml => match async { backend().await?.export_opml().await }.await {
            Ok(raw) => feeds_intents(vec![
                FeedsPageIntent::OpmlTextExported(raw),
                FeedsPageIntent::SetStatus {
                    message: "已导出 OPML。".to_string(),
                    tone: "info".to_string(),
                },
            ]),
            Err(error) => feeds_failed("导出 OPML", error),
        },
        FeedsCommand::ImportOpml { raw } => {
            match async { backend().await?.import_opml(&raw).await }.await {
                Ok(outcome) => feeds_intents(vec![
                    FeedsPageIntent::SetStatus {
                        message: format!("OPML 已导入：{}。", opml_import_summary(&outcome)),
                        tone: "info".to_string(),
                    },
                    FeedsPageIntent::BumpReload,
                ]),
                Err(error) => feeds_failed("导入 OPML", error),
            }
        }
    }
}

fn feeds_intents(intents: Vec<FeedsPageIntent>) -> Vec<UiIntent> {
    intents.into_iter().map(UiIntent::FeedsPage).collect()
}

fn feeds_failed(action: &'static str, error: rssr_api::BackendError) -> Vec<UiIntent> {
    vec![UiIntent::Failed(UiFailure::new(action, error))]
}

/// `settings` 只在配置包更新了设置时存在。读回失败不影响导入本身已经提交的结果。
/// What a configuration import led to.
enum ConfigImportResult {
    Imported(ConfigImportOutcome),
    /// The configuration changed after the page read it; nothing was replaced.
    Conflict,
    Failed(UiFailure),
}

fn import_config_result(result: ConfigImportResult) -> Vec<UiIntent> {
    let mut intents = vec![FeedsPageIntent::PendingConfigImportSet(false)];
    let (message, tone) = match result {
        ConfigImportResult::Imported(outcome) => {
            let summary = super::config_import_summary(&outcome);
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
        ConfigImportResult::Failed(failure) => {
            intents.push(FeedsPageIntent::Failed(failure));
            return feeds_intents(intents);
        }
    };
    intents.push(FeedsPageIntent::SetStatus { message, tone: tone.to_string() });
    feeds_intents(intents)
}

fn opml_import_summary(outcome: &OpmlImportOutcome) -> String {
    format!("{} 个订阅", outcome.imported_feed_count)
}

/// What adding led to. A job: the page (or the shell, when the page was left) re-reads what it
/// shows when it ends, saved or not (C4b item 3).
fn add_feed_result(token: u64, result: Result<AddSubscriptionOutcome, UiFailure>) -> Vec<UiIntent> {
    let result = match result {
        Ok(AddSubscriptionOutcome::NeedsSelection { page_url, candidates }) => {
            return feeds_intents(vec![
                FeedsPageIntent::FeedCandidates { token, page_url, candidates },
                FeedsPageIntent::AddFeedFinished { token, saved: false },
            ]);
        }
        result => result,
    };
    let (saved, message, tone) = match result {
        Ok(AddSubscriptionOutcome::NeedsSelection { .. }) => unreachable!(),
        // The draft stays: an unknown outcome keeps it too, the page reads again what it shows.
        Err(failure) => {
            return feeds_intents(vec![
                FeedsPageIntent::AddFeedFinished { token, saved: false },
                FeedsPageIntent::Failed(failure),
            ]);
        }
        Ok(AddSubscriptionOutcome::SavedAndRefreshed) => {
            (true, "订阅已保存并完成首次刷新。".to_string(), "info")
        }
        Ok(AddSubscriptionOutcome::SavedRefreshFailed { failure }) => (
            true,
            format!("订阅已保存，但首次刷新失败：{}", super::failure_line(&failure)),
            "error",
        ),
    };
    feeds_intents(vec![
        FeedsPageIntent::AddFeedFinished { token, saved },
        FeedsPageIntent::SetStatus { message, tone: tone.to_string() },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every outcome releases the gate of the add it answers (its token); none reloads the page
    /// itself: the job's end re-reads every page through the shell (C4b item 3).
    #[test]
    fn every_add_outcome_releases_its_gate_and_leaves_the_reload_to_the_shell() {
        for (result, expected_saved, expected_tone) in [
            (Ok(AddSubscriptionOutcome::SavedAndRefreshed), true, "info"),
            (
                Ok(AddSubscriptionOutcome::SavedRefreshFailed {
                    failure: rssr_application::RefreshFailureItem {
                        feed_id: 7,
                        url: "https://example.com/feed.xml".into(),
                        name: None,
                        kind: rssr_application::ErrorKind::Network,
                        message: "offline".into(),
                    },
                }),
                true,
                "error",
            ),
            (Err(crate::ui::failure::plain("invalid feed URL")), false, "error"),
        ] {
            let intents = add_feed_result(9, result)
                .into_iter()
                .filter_map(UiIntent::into_feeds_page_intent)
                .collect::<Vec<_>>();
            assert!(matches!(
                intents.first(),
                Some(FeedsPageIntent::AddFeedFinished { token: 9, saved }) if *saved == expected_saved
            ));
            let tone = match &intents[1] {
                FeedsPageIntent::SetStatus { tone, .. } => tone.clone(),
                FeedsPageIntent::Failed(failure) => failure.tone().to_owned(),
                other => panic!("{other:?}"),
            };
            assert_eq!(tone, expected_tone);
            assert!(!intents.iter().any(|intent| matches!(intent, FeedsPageIntent::BumpReload)));
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
            (
                ConfigImportResult::Failed(crate::ui::failure::plain("bad json")),
                None,
                "error",
                false,
            ),
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
            let tone = match intents.last() {
                Some(FeedsPageIntent::SetStatus { tone, .. }) => tone.clone(),
                Some(FeedsPageIntent::Failed(failure)) => failure.tone().to_owned(),
                other => panic!("{other:?}"),
            };
            assert_eq!(tone, expected_tone);
            assert_eq!(
                intents.iter().any(|intent| matches!(intent, FeedsPageIntent::BumpReload)),
                reloads
            );
        }
    }
}

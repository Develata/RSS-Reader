pub(crate) mod backend;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod characterization;
mod entries;
mod entry_flags;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod failure_classes;
#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) mod fake_backend;
mod feeds;
mod reader;
mod settings;
mod shell;

use crate::ui::{commands::UiCommand, snapshot::UiIntent};

pub(crate) async fn execute_ui_command(command: UiCommand) -> Vec<UiIntent> {
    match command {
        UiCommand::Shell(command) => shell::execute(command).await,
        UiCommand::Entries(command) => entries::execute(command).await,
        UiCommand::Reader(command) => reader::execute(command).await,
        UiCommand::Feeds(command) => feeds::execute(command).await,
        UiCommand::Settings(command) => settings::execute(command).await,
    }
}

pub(super) fn new_entries_message(inserted_count: u64) -> String {
    if inserted_count == 0 {
        "没有新文章".into()
    } else {
        format!("新增 {inserted_count} 篇文章")
    }
}

/// One failed subscription as the status line shows it: its name, or its address when it has
/// none (B2-D7).
pub(super) fn failure_line(failure: &rssr_application::RefreshFailureItem) -> String {
    format!("{}: {}", failure.name.as_deref().unwrap_or(&failure.url), failure.message)
}

/// Every failed subscription on one status line; `None` when nothing failed.
pub(super) fn failure_lines(failures: &[rssr_application::RefreshFailureItem]) -> Option<String> {
    (!failures.is_empty())
        .then(|| failures.iter().map(failure_line).collect::<Vec<_>>().join(" | "))
}

/// What a configuration import changed, for the status line.
pub(super) fn config_import_summary(outcome: &rssr_application::ConfigImportOutcome) -> String {
    let settings = if outcome.settings_updated { "设置已更新" } else { "设置未变化" };
    format!(
        "导入 {} 个订阅，清理 {} 个缺失订阅，{settings}",
        outcome.imported_feed_count, outcome.removed_feed_count
    )
}

#[cfg(test)]
mod failure_line_tests {
    use rssr_application::{ErrorKind, RefreshFailureItem};

    #[test]
    fn a_failure_names_the_subscription_or_its_address() {
        let failure = |name: Option<&str>| RefreshFailureItem {
            feed_id: 1,
            url: "https://example.com/feed.xml".into(),
            name: name.map(str::to_string),
            kind: ErrorKind::Network,
            message: "HTTP 404".into(),
        };
        assert_eq!(super::failure_line(&failure(Some("技术资讯"))), "技术资讯: HTTP 404");
        assert_eq!(super::failure_line(&failure(None)), "https://example.com/feed.xml: HTTP 404");
        assert_eq!(
            super::failure_lines(&[failure(Some("甲")), failure(None)]).as_deref(),
            Some("甲: HTTP 404 | https://example.com/feed.xml: HTTP 404")
        );
    }
}

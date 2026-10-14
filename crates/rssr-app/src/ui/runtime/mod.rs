mod backend;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod characterization;
mod entries;
mod entry_flags;
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

/// One failed subscription as the status line shows it.
pub(super) fn failure_line(failure: &rssr_application::RefreshFailureItem) -> String {
    format!("{}: {}", failure.url, failure.message)
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

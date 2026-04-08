use std::sync::Arc;

use crate::bootstrap::AppServices;

use super::entries_page_commands::{EntriesPageCommand, EntriesPageCommandOutcome, error, info};

pub(crate) async fn execute_command(command: EntriesPageCommand) -> EntriesPageCommandOutcome {
    match command {
        EntriesPageCommand::ToggleRead { entry_id, entry_title, currently_read } => {
            match shared_services().await {
                Ok(services) => match services.set_read(entry_id, !currently_read).await {
                    Ok(()) => info(
                        format!(
                            "已将《{}》{}。",
                            entry_title,
                            if currently_read { "标记为未读" } else { "标记为已读" }
                        ),
                        true,
                    ),
                    Err(err) => error(format!("更新已读状态失败：{err}"), false),
                },
                Err(err) => error(err, false),
            }
        }
        EntriesPageCommand::ToggleStarred { entry_id, entry_title, currently_starred } => {
            match shared_services().await {
                Ok(services) => match services.set_starred(entry_id, !currently_starred).await {
                    Ok(()) => info(
                        format!(
                            "已{}《{}》。",
                            if currently_starred { "取消收藏" } else { "收藏" },
                            entry_title
                        ),
                        true,
                    ),
                    Err(err) => error(format!("更新收藏状态失败：{err}"), false),
                },
                Err(err) => error(err, false),
            }
        }
    }
}

async fn shared_services() -> Result<Arc<AppServices>, String> {
    AppServices::shared().await.map_err(|err| format!("初始化应用失败：{err}"))
}

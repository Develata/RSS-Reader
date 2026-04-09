mod entries;
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

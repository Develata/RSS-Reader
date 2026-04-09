mod entries;
mod feeds;
mod reader;
mod settings;
mod shell;

pub(crate) use self::{
    entries::EntriesCommand, feeds::FeedsCommand, reader::ReaderCommand, settings::SettingsCommand,
    shell::ShellCommand,
};

#[derive(Debug, Clone)]
pub(crate) enum UiCommand {
    Shell(ShellCommand),
    Entries(EntriesCommand),
    Reader(ReaderCommand),
    Feeds(FeedsCommand),
    Settings(SettingsCommand),
}

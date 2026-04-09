use super::commands::FeedsPageCommand;

#[derive(Debug, Clone)]
pub(crate) enum FeedsPageEffect {
    LoadSnapshot,
    ExecuteCommand(FeedsPageCommand),
    ReadFeedUrlFromClipboard,
}

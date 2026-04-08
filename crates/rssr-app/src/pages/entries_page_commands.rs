#[derive(Debug, Clone)]
pub(crate) enum EntriesPageCommand {
    ToggleRead { entry_id: i64, entry_title: String, currently_read: bool },
    ToggleStarred { entry_id: i64, entry_title: String, currently_starred: bool },
}

pub(crate) struct EntriesPageCommandOutcome {
    pub(crate) status_message: String,
    pub(crate) status_tone: &'static str,
    pub(crate) reload: bool,
}

pub(crate) fn info(message: impl Into<String>, reload: bool) -> EntriesPageCommandOutcome {
    EntriesPageCommandOutcome { status_message: message.into(), status_tone: "info", reload }
}

pub(crate) fn error(message: impl Into<String>, reload: bool) -> EntriesPageCommandOutcome {
    EntriesPageCommandOutcome { status_message: message.into(), status_tone: "error", reload }
}

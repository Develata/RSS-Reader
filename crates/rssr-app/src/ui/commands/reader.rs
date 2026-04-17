#[derive(Debug, Clone)]
pub(crate) enum ReaderCommand {
    LoadEntry { entry_id: i64 },
    LocalizeEntryAssets { entry_id: i64 },
    ToggleRead { entry_id: i64, currently_read: bool, via_shortcut: bool },
    ToggleStarred { entry_id: i64, currently_starred: bool, via_shortcut: bool },
}

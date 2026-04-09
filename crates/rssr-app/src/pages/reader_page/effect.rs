#[derive(Debug, Clone)]
pub(crate) enum ReaderPageEffect {
    LoadEntry(i64),
    ToggleRead { entry_id: i64, currently_read: bool, via_shortcut: bool },
    ToggleStarred { entry_id: i64, currently_starred: bool, via_shortcut: bool },
}

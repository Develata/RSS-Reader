#[derive(Debug, Clone)]
pub(crate) enum ReaderCommand {
    LoadEntry {
        entry_id: i64,
    },
    /// The page's read-only refresh (C4b item 2): the entry's flags and neighbours again.
    RereadEntry {
        entry_id: i64,
    },
    LocalizeEntryAssets {
        entry_id: i64,
    },
    SetRead {
        entry_id: i64,
        read: bool,
        via_shortcut: bool,
    },
    SetStarred {
        entry_id: i64,
        starred: bool,
        via_shortcut: bool,
    },
}

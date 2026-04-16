mod entries;
mod models;
mod storage;

pub use entries::{entry_flags, to_domain_entry, upsert_entries};
pub use models::{
    APP_STATE_STORAGE_KEY, BrowserState, ENTRY_CONTENT_STORAGE_KEY, ENTRY_FLAGS_STORAGE_KEY,
    LoadedState, PersistedAppStateSlice, PersistedEntryContent, PersistedEntryContentSlice,
    PersistedEntryFlag, PersistedEntryFlagsSlice, PersistedEntryIndex, PersistedFeed,
    PersistedState, STORAGE_KEY,
};
pub use storage::{
    load_state, save_app_state_slice, save_entry_content_patch, save_entry_flag_patch,
    save_state_snapshot,
};

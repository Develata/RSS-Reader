use rssr_infra::application_adapters::browser::state::{
    APP_STATE_STORAGE_KEY, BrowserState, BrowserStore, COMMIT_STORAGE_KEY,
    ENTRY_CONTENT_STORAGE_KEY, ENTRY_FLAGS_STORAGE_KEY, STORAGE_KEY,
};

pub const KEYS: [&str; 4] =
    [STORAGE_KEY, APP_STATE_STORAGE_KEY, ENTRY_FLAGS_STORAGE_KEY, ENTRY_CONTENT_STORAGE_KEY];

pub fn storage() -> web_sys::Storage {
    web_sys::window().unwrap().local_storage().unwrap().unwrap()
}

pub fn clear_browser_state_storage() {
    let storage = storage();
    for key in KEYS {
        storage.remove_item(key).unwrap();
        storage.remove_item(&format!("{key}-next")).unwrap();
    }
    storage.remove_item(COMMIT_STORAGE_KEY).unwrap();
}

pub async fn seed_state(state: BrowserState) -> BrowserStore {
    clear_browser_state_storage();
    for (key, raw) in KEYS.into_iter().zip([
        serde_json::to_string(&state.core).unwrap(),
        serde_json::to_string(&state.app_state).unwrap(),
        serde_json::to_string(&state.entry_flags).unwrap(),
        serde_json::to_string(&state.entry_content).unwrap(),
    ]) {
        storage().set_item(key, &raw).unwrap();
    }
    BrowserStore::open().await.unwrap()
}

pub async fn persisted_state() -> BrowserState {
    BrowserStore::open().await.unwrap().snapshot().await.unwrap()
}

pub fn committed_slice(key: &str) -> Option<String> {
    let commit: serde_json::Value =
        serde_json::from_str(&storage().get_item(COMMIT_STORAGE_KEY).unwrap().unwrap()).unwrap();
    let index = KEYS.iter().position(|candidate| *candidate == key).unwrap();
    let revision = commit["revisions"][index].as_u64().unwrap();
    let active = if revision.is_multiple_of(2) { key.to_string() } else { format!("{key}-next") };
    storage().get_item(&active).unwrap()
}

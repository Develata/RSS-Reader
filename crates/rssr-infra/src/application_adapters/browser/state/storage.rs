use anyhow::Context;
use js_sys::Date;
use web_sys::{Storage, window};

use super::{
    APP_STATE_STORAGE_KEY, BrowserState, ENTRY_CONTENT_STORAGE_KEY, ENTRY_FLAGS_STORAGE_KEY,
    LoadedState, PersistedAppStateSlice, PersistedEntryContentSlice, PersistedEntryFlagsSlice,
    STORAGE_KEY,
};

pub fn load_state() -> LoadedState {
    let Some(storage) = browser_storage() else {
        return LoadedState { state: BrowserState::default(), warning: None };
    };

    let raw = match storage.get_item(STORAGE_KEY) {
        Ok(Some(raw)) => raw,
        Ok(None) => {
            return LoadedState { state: BrowserState::default(), warning: None };
        }
        Err(_) => {
            return LoadedState {
                state: BrowserState::default(),
                warning: Some("读取浏览器本地存储失败，已使用空状态启动。".to_string()),
            };
        }
    };

    match serde_json::from_str(&raw).context("解析浏览器本地状态失败") {
        Ok(core) => LoadedState {
            state: BrowserState {
                core,
                app_state: load_app_state_slice(&storage).unwrap_or_default(),
                entry_flags: load_entry_flags_slice(&storage).unwrap_or_default(),
                entry_content: load_entry_content_slice(&storage).unwrap_or_default(),
            },
            warning: None,
        },
        Err(error) => {
            backup_corrupt_blob(&storage, STORAGE_KEY, &raw);
            let _ = storage.remove_item(STORAGE_KEY);
            LoadedState {
                state: BrowserState::default(),
                warning: Some(format!(
                    "浏览器本地状态已损坏，已保留损坏副本并使用空状态启动：{error}"
                )),
            }
        }
    }
}

/// 按引用接收：此前是按值，于是每个调用方都要 `state.clone()` 深拷贝整个浏览器状态
/// （全部订阅 + 全部条目 + 全部标记 + **全部正文**），而这里其实只读不写。
/// 刷新提交和保存设置都会走到这条路径。
pub fn save_state_snapshot(state: &BrowserState) -> anyhow::Result<()> {
    save_serialized_state(serde_json::to_string(&state.core)?)?;
    save_app_state_slice_internal(&state.app_state)?;
    save_entry_flags_slice_internal(&state.entry_flags)?;
    save_entry_content_slice_internal(&state.entry_content)?;
    Ok(())
}

pub fn save_app_state_slice(slice: &PersistedAppStateSlice) -> anyhow::Result<()> {
    save_app_state_slice_internal(slice)
}

/// 直接写入调用方已持有的权威标记集合。
///
/// 此前这里会先把 localStorage 里的全部标记读出来 + JSON 解析一遍，再线性查找合并——
/// 在**同一个标签页内**，调用方刚改完的内存 `state.entry_flags` 就是权威值（所有 browser
/// adapter 共享同一个 `Arc<Mutex<BrowserState>>`），那次读取与解析纯属多余。
///
/// 取舍：跨标签页时两个标签各有自己的内存副本，整片覆盖会丢掉另一个标签的标记改动，
/// 而先读后合并能按条目保住它。这不是新引入的问题——`save_state_snapshot` 本来就整片覆盖
/// 同一批 key——但覆盖频率从「刷新/存设置时」变成了「每次切已读/收藏」。
/// 真要支持多标签页，应该走 `storage` 事件做跨标签同步，而不是靠这里的读改写。
/// 标记数量随「历史上读过/收藏过的文章数」单调增长，不受归档窗口限制，因此这次省掉的
/// 是一份会一直变大的开销。
pub fn save_entry_flags_slice(slice: &PersistedEntryFlagsSlice) -> anyhow::Result<()> {
    save_entry_flags_slice_internal(slice)
}

fn save_serialized_state(raw: String) -> anyhow::Result<()> {
    let Some(storage) = browser_storage() else {
        return Ok(());
    };
    save_storage_key(&storage, STORAGE_KEY, raw)?;
    Ok(())
}

fn browser_storage() -> Option<Storage> {
    window().and_then(|window| window.local_storage().ok()).flatten()
}

fn save_storage_key(storage: &Storage, key: &str, raw: String) -> anyhow::Result<()> {
    storage.set_item(key, &raw).map_err(|_| anyhow::anyhow!("写入浏览器本地存储失败"))
}

fn save_app_state_slice_internal(slice: &PersistedAppStateSlice) -> anyhow::Result<()> {
    let Some(storage) = browser_storage() else {
        return Ok(());
    };

    let raw = serde_json::to_string(slice)?;
    save_storage_key(&storage, APP_STATE_STORAGE_KEY, raw)
}

fn save_entry_flags_slice_internal(slice: &PersistedEntryFlagsSlice) -> anyhow::Result<()> {
    let Some(storage) = browser_storage() else {
        return Ok(());
    };

    save_storage_key(&storage, ENTRY_FLAGS_STORAGE_KEY, serde_json::to_string(&slice)?)
}

fn save_entry_content_slice_internal(slice: &PersistedEntryContentSlice) -> anyhow::Result<()> {
    let Some(storage) = browser_storage() else {
        return Ok(());
    };

    save_storage_key(&storage, ENTRY_CONTENT_STORAGE_KEY, serde_json::to_string(&slice)?)
}

fn load_app_state_slice(storage: &Storage) -> Option<PersistedAppStateSlice> {
    let raw = storage.get_item(APP_STATE_STORAGE_KEY).ok().flatten()?;
    match serde_json::from_str(&raw) {
        Ok(slice) => Some(slice),
        Err(_) => {
            backup_corrupt_blob(storage, APP_STATE_STORAGE_KEY, &raw);
            let _ = storage.remove_item(APP_STATE_STORAGE_KEY);
            None
        }
    }
}

fn load_entry_flags_slice(storage: &Storage) -> Option<PersistedEntryFlagsSlice> {
    let raw = storage.get_item(ENTRY_FLAGS_STORAGE_KEY).ok().flatten()?;
    match serde_json::from_str(&raw) {
        Ok(slice) => Some(slice),
        Err(_) => {
            backup_corrupt_blob(storage, ENTRY_FLAGS_STORAGE_KEY, &raw);
            let _ = storage.remove_item(ENTRY_FLAGS_STORAGE_KEY);
            None
        }
    }
}

fn load_entry_content_slice(storage: &Storage) -> Option<PersistedEntryContentSlice> {
    let raw = storage.get_item(ENTRY_CONTENT_STORAGE_KEY).ok().flatten()?;
    match serde_json::from_str(&raw) {
        Ok(slice) => Some(slice),
        Err(_) => {
            backup_corrupt_blob(storage, ENTRY_CONTENT_STORAGE_KEY, &raw);
            let _ = storage.remove_item(ENTRY_CONTENT_STORAGE_KEY);
            None
        }
    }
}

fn backup_corrupt_blob(storage: &Storage, key: &str, raw: &str) {
    let backup_key = format!("{key}-corrupt-{}", Date::now() as i64);
    let _ = storage.set_item(&backup_key, raw);
}

//! Copy-on-write slices with one atomic publication key. Called only under the Web Lock.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use web_sys::Storage;

use super::{
    APP_STATE_STORAGE_KEY, BrowserState, ENTRY_CONTENT_STORAGE_KEY, ENTRY_FLAGS_STORAGE_KEY,
    STORAGE_KEY,
};

pub const COMMIT_STORAGE_KEY: &str = "rssr-web-commit-v1";
/// 提交头里的数据格式版本（键名和锁名不随它改）。2：订阅增加 `custom_title`（审计 B6）。
/// 版本 1 的数据片按新模型原样可读（新字段有默认值），所以升级不重写数据片：第一次在锁内
/// 成功读出全部数据片后，经唯一发布点把提交头换成 2。只认版本 1 的旧标签页此后在任何读写
/// 之前就被版本检查挡住，不会把它不认识的字段写丢。
const COMMIT_VERSION: u32 = 2;
const KEYS: [&str; 4] =
    [STORAGE_KEY, APP_STATE_STORAGE_KEY, ENTRY_FLAGS_STORAGE_KEY, ENTRY_CONTENT_STORAGE_KEY];

#[derive(Clone, Copy, Default)]
pub(crate) struct Changes(u8);
impl Changes {
    pub(crate) const NONE: Self = Self(0);
    pub(crate) const CORE: Self = Self(1);
    pub(crate) const APP_STATE: Self = Self(2);
    pub(crate) const FLAGS: Self = Self(4);
    pub(crate) const CONTENT: Self = Self(8);
}
impl std::ops::BitOr for Changes {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Commit {
    version: u32,
    epoch: String,
    revisions: [u64; 4],
}

#[derive(Default)]
pub(super) struct Cache {
    state: BrowserState,
    commit: Option<Commit>,
}

fn storage() -> Result<Storage> {
    web_sys::window()
        .context("浏览器窗口不可用")?
        .local_storage()
        .map_err(|e| anyhow::anyhow!("无法访问浏览器本地存储: {e:?}"))?
        .context("浏览器本地存储不可用，未保存任何更改")
}

fn get(storage: &Storage, key: &str) -> Result<Option<String>> {
    storage.get_item(key).map_err(|e| anyhow::anyhow!("读取浏览器本地存储失败 ({key}): {e:?}"))
}
fn put(storage: &Storage, key: &str, raw: &str) -> Result<()> {
    storage.set_item(key, raw).map_err(|error| {
        if is_quota_exceeded(&error) {
            // 发布时新旧两份数据片要同时放得下（见 `publish`），所以接近上限时每次写正文都会失败，
            // 删除订阅也一样要先写剩余数据的新副本。存储被禁用时浏览器也抛这个错误（WHATWG）。
            anyhow::anyhow!(
                "浏览器本地存储配额不足或存储受限，本次更改未保存（{key}，约 {} KiB）。",
                raw.len() / 1024
            )
        } else {
            anyhow::anyhow!("写入浏览器本地存储失败 ({key}): {error:?}")
        }
    })
}

/// WHATWG Storage：超出配额时 `setItem` 抛 `QuotaExceededError`（旧版 Firefox 用
/// `NS_ERROR_DOM_QUOTA_REACHED`）。审计 D1：此前只显示 `JsValue` 的调试串。
fn is_quota_exceeded(error: &wasm_bindgen::JsValue) -> bool {
    js_sys::Reflect::get(error, &wasm_bindgen::JsValue::from_str("name"))
        .ok()
        .and_then(|name| name.as_string())
        .is_some_and(|name| {
            matches!(name.as_str(), "QuotaExceededError" | "NS_ERROR_DOM_QUOTA_REACHED")
        })
}
fn slot(index: usize, revision: u64) -> String {
    if revision.is_multiple_of(2) {
        KEYS[index].to_string()
    } else {
        format!("{}-next", KEYS[index])
    }
}

fn decode<T: DeserializeOwned>(raw: &str, key: &str) -> Result<T> {
    serde_json::from_str(raw).with_context(|| format!("本地数据损坏 ({key})；原数据已保留，未覆盖"))
}
fn load_slice<T: DeserializeOwned>(storage: &Storage, commit: &Commit, index: usize) -> Result<T> {
    let key = slot(index, commit.revisions[index]);
    decode(&get(storage, &key)?.with_context(|| format!("已提交的数据片段缺失 ({key})"))?, &key)
}

fn serialized(state: &BrowserState, index: usize) -> Result<String> {
    Ok(match index {
        0 => serde_json::to_string(&state.core)?,
        1 => serde_json::to_string(&state.app_state)?,
        2 => serde_json::to_string(&state.entry_flags)?,
        3 => serde_json::to_string(&state.entry_content)?,
        _ => unreachable!(),
    })
}

fn current(storage: &Storage) -> Result<Commit> {
    if let Some(raw) = get(storage, COMMIT_STORAGE_KEY)? {
        let commit: Commit = decode(&raw, COMMIT_STORAGE_KEY)?;
        ensure!(
            (1..=COMMIT_VERSION).contains(&commit.version),
            "无法读取此版本的浏览器数据；未修改原数据"
        );
        return Ok(commit);
    }
    // First open adopts legacy keys in place. Validate all existing slices first; a corrupt
    // or inaccessible slice must not be replaced with a default or a partial migration.
    for key in KEYS {
        ensure!(
            get(storage, &format!("{key}-next"))?.is_none(),
            "本地提交记录缺失且存在新版数据片段；原数据已保留，未覆盖"
        );
    }
    let mut raw = Vec::with_capacity(4);
    for key in KEYS {
        raw.push(get(storage, key)?);
    }
    if let Some(value) = &raw[0] {
        let _: super::PersistedState = decode(value, KEYS[0])?;
    }
    if let Some(value) = &raw[1] {
        let _: super::PersistedAppStateSlice = decode(value, KEYS[1])?;
    }
    if let Some(value) = &raw[2] {
        let _: super::PersistedEntryFlagsSlice = decode(value, KEYS[2])?;
    }
    if let Some(value) = &raw[3] {
        let _: super::PersistedEntryContentSlice = decode(value, KEYS[3])?;
    }
    let empty = BrowserState::default();
    for (index, value) in raw.iter().enumerate() {
        if value.is_none() {
            put(storage, KEYS[index], &serialized(&empty, index)?)?;
        }
    }
    let commit = Commit {
        version: COMMIT_VERSION,
        epoch: uuid::Uuid::new_v4().to_string(),
        revisions: [0; 4],
    };
    put(storage, COMMIT_STORAGE_KEY, &serde_json::to_string(&commit)?)?;
    Ok(commit)
}

fn synchronize(storage: &Storage, cache: &mut Cache) -> Result<Commit> {
    let commit = current(storage)?;
    let changed = |index| {
        cache.commit.as_ref().is_none_or(|old| {
            old.epoch != commit.epoch || old.revisions[index] != commit.revisions[index]
        })
    };
    // Only parse slices whose committed revision changed; a flag operation does not parse body.
    if changed(0) {
        cache.state.core = load_slice(storage, &commit, 0)?;
    }
    if changed(1) {
        cache.state.app_state = load_slice(storage, &commit, 1)?;
    }
    if changed(2) {
        cache.state.entry_flags = load_slice(storage, &commit, 2)?;
    }
    if changed(3) {
        cache.state.entry_content = load_slice(storage, &commit, 3)?;
    }
    cache.commit = Some(commit.clone());
    Ok(commit)
}

fn cleanup(storage: &Storage, commit: &Commit) {
    // Fixed two slots per slice: interrupted staging cannot grow an unbounded journal.
    // Cleanup is best effort; once publication succeeds a cleanup error cannot undo success.
    for (index, revision) in commit.revisions.iter().enumerate() {
        if let Err(error) = storage.remove_item(&slot(index, revision ^ 1)) {
            tracing::warn!(?error, "浏览器暂存片段清理失败，将在下次写入重试");
        }
    }
}

fn publish(
    storage: &Storage,
    cache: &mut Cache,
    changes: Changes,
    mut commit: Commit,
) -> Result<()> {
    // 没有改动也要发布一次旧版本的提交头：只读打开同样完成升级。
    if changes.0 == 0 && commit.version == COMMIT_VERSION {
        return Ok(());
    }
    cleanup(storage, &commit);
    for index in 0..4 {
        if changes.0 & (1 << index) != 0 {
            commit.revisions[index] =
                commit.revisions[index].checked_add(1).context("存储版本号已耗尽")?;
            put(storage, &slot(index, commit.revisions[index]), &serialized(&cache.state, index)?)?;
        }
    }
    commit.version = COMMIT_VERSION;
    // setItem is the only publication point: until it succeeds, all readers use old slices.
    put(storage, COMMIT_STORAGE_KEY, &serde_json::to_string(&commit)?)?;
    cache.commit = Some(commit.clone());
    cleanup(storage, &commit);
    Ok(())
}

/// Called under the same Web Lock as transaction; existing/corrupt data is never replaced.
pub(super) fn initialize_if_pristine(cache: &mut Cache, initial: BrowserState) -> Result<()> {
    let storage = storage()?;
    let mut pristine = get(&storage, COMMIT_STORAGE_KEY)?.is_none();
    if pristine {
        for key in KEYS {
            if get(&storage, key)?.is_some() || get(&storage, &format!("{key}-next"))?.is_some() {
                pristine = false;
                break;
            }
        }
    }
    transaction(cache, move |state| {
        if pristine {
            *state = initial;
            Ok(((), Changes::CORE | Changes::APP_STATE | Changes::FLAGS | Changes::CONTENT))
        } else {
            Ok(((), Changes::NONE))
        }
    })
}

pub(super) fn transaction<T>(
    cache: &mut Cache,
    operation: impl FnOnce(&mut BrowserState) -> Result<(T, Changes)>,
) -> Result<T> {
    let storage = storage()?;
    let commit = synchronize(&storage, cache)?;
    let result = operation(&mut cache.state).and_then(|(value, changes)| {
        publish(&storage, cache, changes, commit)?;
        Ok(value)
    });
    if result.is_err() {
        // Reload the still-committed data only on failure, rather than cloning all bodies
        // before every write. If recovery also fails, the invalid cache cannot be queried.
        cache.commit = None;
        if let Err(error) = synchronize(&storage, cache) {
            tracing::warn!(%error, "浏览器内存快照恢复失败，下次访问将重试读取已提交数据");
        }
    }
    result
}

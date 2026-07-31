//! 顶栏搜索词、导航收起状态与文章区控件收起状态的持久化，三端行为一致。
//!
//! 这几项此前只有 Web 端存进 `localStorage`，桌面与 Android 每次启动都回到「未筛选、导航展开」。
//! 同一个界面元素在不同平台记不记得住，属于平台差异漏进了产品语义，因此统一为三端都记住。
//!
//! **必须是同步读写。** `use_app_shell_state` 在首帧就要拿到值。若改走 `AppStateSnapshot`
//! 那条异步链路，首帧会先渲染成空搜索框加展开的导航，下一帧才跳成持久化的值——
//! Web 端现有的体验会退化出一次闪烁。为此这里不进 domain / application，
//! 就是一层宿主能力适配。
//!
//! 落盘格式（原生端）：数据库同目录下的 `shell-prefs.json`。字段全部走 `Default` 兜底，
//! 文件缺失、损坏、字段增减一律回落到默认值，不做版本协商也不做迁移——这是一份随时可以
//! 丢弃重建的界面偏好，为它引入迁移责任不划算。注意 `entry_controls_hidden` 的默认值是
//! `true`（控件默认收起），因此 `ShellPrefs` 手写 `Default` 实现而不是派生——派生的
//! `bool::default()` 会把老文件里缺失的该字段读成「展开」，等于倒退默认体验。
//! Web 端沿用原有的 `localStorage` 键，老用户已经存下的值不会因为这次改动丢失。

#[cfg(not(target_arch = "wasm32"))]
mod platform {
    use std::{
        path::PathBuf,
        sync::{Mutex, OnceLock},
    };

    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(default)]
    struct ShellPrefs {
        entry_search: String,
        nav_hidden: bool,
        entry_controls_hidden: bool,
    }

    /// 控件默认收起：老版本写出的偏好文件没有 `entry_controls_hidden` 字段，
    /// 反序列化走 `Default` 兜底时必须落回 `true`，否则老用户升级后控件
    /// 会莫名变成展开。
    impl Default for ShellPrefs {
        fn default() -> Self {
            Self { entry_search: String::new(), nav_hidden: false, entry_controls_hidden: true }
        }
    }

    /// 进程内缓存。有它之后写入不必先读盘做 read-modify-write，
    /// 也就不会出现两个字段互相覆盖对方的情况。
    fn cached() -> &'static Mutex<ShellPrefs> {
        static CACHE: OnceLock<Mutex<ShellPrefs>> = OnceLock::new();
        CACHE.get_or_init(|| Mutex::new(read_from_disk()))
    }

    /// 解析一次落盘路径并确保目录存在，之后整个进程复用。
    ///
    /// 建目录刻意放在这里而不是写入路径上：搜索框每敲一个键都会走一次写入，
    /// 把 `create_dir_all` 留在那里等于每个按键多一次目录 syscall，而这个目录
    /// 本来就由数据库在启动时建好。目录不可写时也只在这里判一次，不必每次重试。
    fn prefs_path() -> Option<&'static PathBuf> {
        static PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
        PATH.get_or_init(|| {
            let dir = match rssr_infra::db::sqlite_native::local_data_dir() {
                Ok(dir) => dir,
                Err(error) => {
                    tracing::debug!(%error, "无法定位本地数据目录，界面偏好本次不持久化");
                    return None;
                }
            };
            if let Err(error) = std::fs::create_dir_all(&dir) {
                tracing::debug!(%error, "创建本地数据目录失败，界面偏好本次不持久化");
                return None;
            }
            Some(dir.join("shell-prefs.json"))
        })
        .as_ref()
    }

    /// 中毒说明此前有线程持锁时 panic 了。这里存的是两个随时可再生的界面字段，
    /// 与其此后整个进程静默丢掉用户的每一次输入，不如接着用最后那份值。
    fn locked() -> std::sync::MutexGuard<'static, ShellPrefs> {
        cached().lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn read_from_disk() -> ShellPrefs {
        let Some(path) = prefs_path() else {
            return ShellPrefs::default();
        };
        let Ok(raw) = std::fs::read_to_string(path) else {
            // 首次启动时文件本就不存在，这条路径是常态，不值得记日志。
            return ShellPrefs::default();
        };
        decode(&raw)
    }

    /// 解码与落盘分开：路径解析走 `OnceLock` + `current_exe()`，进程内只能定一次，
    /// 测不了；而「文件坏了会怎样」恰恰是这里唯一值得测的部分。
    fn decode(raw: &str) -> ShellPrefs {
        serde_json::from_str(raw).unwrap_or_else(|error| {
            tracing::debug!(%error, "界面偏好文件无法解析，按默认值处理");
            ShellPrefs::default()
        })
    }

    /// 改动缓存并落盘。值没变就直接返回：搜索框每次按键都会调到这里，
    /// 但重复设置同一个值（例如页面重渲染回填）不该产生一次写盘。
    ///
    /// 落盘时刻意持锁：这样「缓存里的值」与「文件里的值」不会因为两次并发写入交错而错位。
    /// 实测单次写入 0.5~1.1 ms（本机，含被实时扫描的目录），按键速率下不构成可感知的卡顿，
    /// 因此不引入写入合并——合并的代价是进程被杀时丢掉最后几个字符，
    /// 而「记住搜索词」这件事恰恰不该记成一个前缀。
    ///
    /// 路径在取锁之前解析：`prefs_path()` 自己也走一个 `OnceLock`，留在锁里只是给将来
    /// 埋一个「路径解析回头读偏好就死锁」的坑，挪出来零成本。
    fn update(mutate: impl FnOnce(&mut ShellPrefs)) {
        let path = prefs_path();

        let mut prefs = locked();
        let before = prefs.clone();
        mutate(&mut prefs);
        if *prefs == before {
            return;
        }

        let Some(path) = path else {
            return;
        };
        let Ok(encoded) = serde_json::to_string(&*prefs) else {
            return;
        };
        if let Err(error) = std::fs::write(path, encoded) {
            tracing::debug!(%error, "写入界面偏好失败，本次改动只留在内存里");
        }
    }

    pub(super) fn initial_entry_search() -> String {
        locked().entry_search.clone()
    }

    pub(super) fn remember_entry_search(value: &str) {
        update(|prefs| prefs.entry_search = value.to_string());
    }

    pub(super) fn initial_nav_hidden() -> bool {
        locked().nav_hidden
    }

    pub(super) fn remember_nav_hidden(hidden: bool) {
        update(|prefs| prefs.nav_hidden = hidden);
    }

    pub(super) fn initial_entry_controls_hidden() -> bool {
        locked().entry_controls_hidden
    }

    pub(super) fn remember_entry_controls_hidden(hidden: bool) {
        update(|prefs| prefs.entry_controls_hidden = hidden);
    }

    #[cfg(test)]
    mod tests {
        use super::{ShellPrefs, decode};

        #[test]
        fn round_trips_through_the_on_disk_form() {
            let prefs = ShellPrefs {
                entry_search: "rust".to_string(),
                nav_hidden: true,
                entry_controls_hidden: false,
            };
            let encoded = serde_json::to_string(&prefs).expect("encode prefs");

            assert_eq!(decode(&encoded), prefs);
        }

        /// 界面偏好没有版本协商，因此每一种坏输入都必须安静地退回默认值。
        /// 这里任何一条走成 panic，用户看到的就是启动即崩溃——而崩溃的原因
        /// 只是一个可以随手删掉的偏好文件。
        #[test]
        fn every_kind_of_damaged_file_falls_back_to_defaults() {
            let cases = [
                "",                         // 空文件（写到一半掉电）
                "{",                        // 截断的 JSON
                "null",                     // 合法 JSON，但不是对象
                "[]",                       // 类型对不上
                r#"{"entry_search": 7}"#,   // 字段类型对不上
                r#"{"nav_hidden": "yes"}"#, // 同上
            ];

            for case in cases {
                assert_eq!(decode(case), ShellPrefs::default(), "应当回落到默认值：{case}");
            }
        }

        /// 缺字段补默认、多字段忽略：这两条一起保证偏好文件可以在版本之间来回读写，
        /// 老版本读到新版本写的文件不会整份作废。
        #[test]
        fn missing_fields_default_and_unknown_fields_are_ignored() {
            assert_eq!(
                decode(r#"{"entry_search": "rust"}"#),
                ShellPrefs { entry_search: "rust".to_string(), ..ShellPrefs::default() }
            );
            assert_eq!(
                decode(r#"{"nav_hidden": true, "future_field": {"a": 1}}"#),
                ShellPrefs {
                    entry_search: String::new(),
                    nav_hidden: true,
                    ..ShellPrefs::default()
                }
            );
        }

        /// 老版本写出的文件没有 `entry_controls_hidden`：兜底必须落在「收起」，
        /// 不能落在 `bool` 派生默认的「展开」。
        #[test]
        fn missing_entry_controls_hidden_defaults_to_collapsed() {
            assert!(decode(r#"{"entry_search": "rust"}"#).entry_controls_hidden);
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod platform {
    /// 键名沿用改动前的取值，老用户存在浏览器里的搜索词与导航状态继续生效。
    const ENTRY_SEARCH_KEY: &str = "rssr-entry-search";
    const NAV_HIDDEN_KEY: &str = "rssr-nav-hidden";
    const ENTRY_CONTROLS_HIDDEN_KEY: &str = "rssr-entry-controls-hidden";

    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok().flatten()
    }

    pub(super) fn initial_entry_search() -> String {
        storage()
            .and_then(|storage| storage.get_item(ENTRY_SEARCH_KEY).ok().flatten())
            .unwrap_or_default()
    }

    pub(super) fn remember_entry_search(value: &str) {
        if let Some(storage) = storage() {
            let _ = storage.set_item(ENTRY_SEARCH_KEY, value);
        }
    }

    pub(super) fn initial_nav_hidden() -> bool {
        storage()
            .and_then(|storage| storage.get_item(NAV_HIDDEN_KEY).ok().flatten())
            .is_some_and(|value| value == "1")
    }

    pub(super) fn remember_nav_hidden(hidden: bool) {
        if let Some(storage) = storage() {
            let _ = storage.set_item(NAV_HIDDEN_KEY, if hidden { "1" } else { "0" });
        }
    }

    pub(super) fn initial_entry_controls_hidden() -> bool {
        storage()
            .and_then(|storage| storage.get_item(ENTRY_CONTROLS_HIDDEN_KEY).ok().flatten())
            .is_none_or(|value| value == "1")
    }

    pub(super) fn remember_entry_controls_hidden(hidden: bool) {
        if let Some(storage) = storage() {
            let _ = storage.set_item(ENTRY_CONTROLS_HIDDEN_KEY, if hidden { "1" } else { "0" });
        }
    }
}

pub(crate) fn initial_entry_search() -> String {
    platform::initial_entry_search()
}

pub(crate) fn remember_entry_search(value: &str) {
    platform::remember_entry_search(value);
}

pub(crate) fn initial_nav_hidden() -> bool {
    platform::initial_nav_hidden()
}

pub(crate) fn remember_nav_hidden(hidden: bool) {
    platform::remember_nav_hidden(hidden);
}

pub(crate) fn initial_entry_controls_hidden() -> bool {
    platform::initial_entry_controls_hidden()
}

pub(crate) fn remember_entry_controls_hidden(hidden: bool) {
    platform::remember_entry_controls_hidden(hidden);
}

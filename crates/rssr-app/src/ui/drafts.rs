//! The settings page's draft across a reload or a sign-in (phase C, C4b item 4). Before the page
//! is reloaded (its build differs from the server's) or leaves for the sign-in page, the edits
//! not stored yet (the ones a save in flight sent, then the ones made since) are kept in this
//! tab's `sessionStorage`, with a version. The settings page that opens next takes them once:
//! they are its pending edits over the settings it reads, never saved by themselves. They stay
//! stored until the page has handed them to the shell, so a sign-in or reload before that keeps
//! them (review C4B-B-09). When the browser cannot keep them, the page does not leave.
//!
//! Only the edits, not the whole settings: a draft restored over settings changed elsewhere
//! changes only what the user edited.

use rssr_application::SettingsPatch;
use serde::{Deserialize, Serialize};

/// The key in `sessionStorage`.
const KEY: &str = "rssr.settings-draft";
/// The draft's format; a draft of another version is not restored.
const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct Stored {
    version: u32,
    edits: SettingsPatch,
}

/// The browser could not keep the draft: the page must not leave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NotKept;

/// Keeps `draft` for the page that comes next; nothing to keep is kept trivially.
pub(crate) fn keep(draft: Option<&SettingsPatch>) -> Result<(), NotKept> {
    let Some(edits) = draft.filter(|edits| !edits.is_empty()) else { return Ok(()) };
    let stored = serde_json::to_string(&Stored { version: VERSION, edits: edits.clone() })
        .map_err(|_| NotKept)?;
    platform::write(KEY, &stored).map_err(|_| NotKept)
}

/// The kept draft, if any, left stored until [`forget`]. A draft that cannot be read, or of
/// another version, is dropped.
pub(crate) fn peek() -> Option<SettingsPatch> {
    let raw = platform::read(KEY)?;
    let edits = serde_json::from_str::<Stored>(&raw)
        .ok()
        .filter(|stored| stored.version == VERSION && !stored.edits.is_empty())
        .map(|stored| stored.edits);
    if edits.is_none() {
        forget();
    }
    edits
}

/// Removes the kept draft: the page has it (and the shell keeps it from now on), so it is
/// restored once.
pub(crate) fn forget() {
    platform::remove(KEY);
}

/// Leaves (`leave`: reload, go to the sign-in page) after keeping `draft`; stays, returning false,
/// when it could not be kept.
pub(crate) fn leave_keeping(draft: Option<&SettingsPatch>, leave: impl FnOnce()) -> bool {
    if keep(draft).is_err() {
        return false;
    }
    leave();
    true
}

#[cfg(target_arch = "wasm32")]
mod platform {
    fn storage() -> Result<web_sys::Storage, ()> {
        web_sys::window().ok_or(())?.session_storage().map_err(|_| ())?.ok_or(())
    }

    pub(super) fn write(key: &str, value: &str) -> Result<(), ()> {
        storage()?.set_item(key, value).map_err(|_| ())
    }

    pub(super) fn read(key: &str) -> Option<String> {
        storage().ok()?.get_item(key).ok().flatten()
    }

    pub(super) fn remove(key: &str) {
        if let Ok(storage) = storage() {
            let _ = storage.remove_item(key);
        }
    }
}

/// Elsewhere the page is never reloaded and has no sign-in page to go to: nothing to keep.
#[cfg(all(not(target_arch = "wasm32"), not(test)))]
mod platform {
    pub(super) fn write(_key: &str, _value: &str) -> Result<(), ()> {
        Ok(())
    }

    pub(super) fn read(_key: &str) -> Option<String> {
        None
    }

    pub(super) fn remove(_key: &str) {}
}

/// The tests' tab: a storage per test thread that can be made to refuse writes.
#[cfg(all(not(target_arch = "wasm32"), test))]
pub(crate) mod platform {
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static STORAGE: RefCell<HashMap<String, String>> = RefCell::default();
        static REFUSING: RefCell<bool> = const { RefCell::new(false) };
    }

    pub(super) fn write(key: &str, value: &str) -> Result<(), ()> {
        if REFUSING.with(|refusing| *refusing.borrow()) {
            return Err(());
        }
        STORAGE.with(|storage| storage.borrow_mut().insert(key.into(), value.into()));
        Ok(())
    }

    pub(super) fn read(key: &str) -> Option<String> {
        STORAGE.with(|storage| storage.borrow().get(key).cloned())
    }

    pub(super) fn remove(key: &str) {
        STORAGE.with(|storage| storage.borrow_mut().remove(key));
    }

    /// Makes writes fail (a full or blocked storage), or work again.
    pub(crate) fn refuse_writes(refusing: bool) {
        REFUSING.with(|refuse| *refuse.borrow_mut() = refusing);
    }

    /// Stores `value` under `key` as an earlier page would have.
    pub(crate) fn put(key: &str, value: &str) {
        STORAGE.with(|storage| storage.borrow_mut().insert(key.into(), value.into()));
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use rssr_application::SettingsPatch;
    use rssr_domain::ThemeMode;

    use super::{KEY, NotKept, forget, keep, leave_keeping, peek, platform};

    /// What the next settings page restores: the draft, then gone.
    fn take() -> Option<SettingsPatch> {
        let draft = peek();
        forget();
        draft
    }

    fn dark() -> SettingsPatch {
        SettingsPatch {
            theme: Some(ThemeMode::Dark),
            custom_css: Some("body { color: red; }".into()),
            ..Default::default()
        }
    }

    /// A kept draft is taken once, with its version; nothing to keep keeps nothing.
    #[test]
    fn a_kept_draft_is_restored_once() {
        assert_eq!(keep(None), Ok(()));
        assert_eq!(keep(Some(&SettingsPatch::default())), Ok(()));
        assert_eq!(platform::read(KEY), None);

        keep(Some(&dark())).expect("kept");
        assert!(platform::read(KEY).is_some_and(|raw| raw.contains("\"version\":1")));
        assert_eq!(peek(), Some(dark()), "read");
        assert_eq!(peek(), Some(dark()), "still there until forgotten");
        assert_eq!(take(), Some(dark()));
        assert_eq!(take(), None, "once");
    }

    /// A draft of another version, or one that cannot be read, is dropped, not restored.
    #[test]
    fn a_draft_of_another_version_is_dropped() {
        platform::put(KEY, r#"{"version":2,"edits":{"theme":"dark"}}"#);
        assert_eq!(peek(), None);
        assert_eq!(platform::read(KEY), None, "dropped");
        platform::put(KEY, "{");
        assert_eq!(peek(), None);
    }

    /// When the browser cannot keep the draft the page does not leave; when it can, it leaves.
    #[test]
    fn the_page_leaves_only_with_its_draft_kept() {
        platform::refuse_writes(true);
        let mut left = false;
        assert!(!leave_keeping(Some(&dark()), || left = true));
        assert!(!left, "stays with its draft");
        assert_eq!(keep(Some(&dark())), Err(NotKept));
        assert!(leave_keeping(None, || left = true), "nothing to keep");

        platform::refuse_writes(false);
        let mut left = false;
        assert!(leave_keeping(Some(&dark()), || left = true));
        assert!(left);
        assert_eq!(take(), Some(dark()));
    }
}

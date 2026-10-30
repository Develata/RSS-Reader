//! The settings page's draft across a reload or a sign-in (phase C, C4b item 4). Before the page
//! is reloaded (its build differs from the server's) or leaves for the sign-in page, the edits
//! not stored yet (the ones a save in flight sent, then the ones made since) are kept in this
//! tab's `sessionStorage`, with a version. The settings page that opens next takes them once:
//! they are its pending edits over the settings it reads, never saved by themselves. They stay
//! stored until the page has handed them to the shell, so a sign-in or reload before that keeps
//! them (review C4B-B-09). When the browser cannot keep them, the page does not leave.
//!
//! Once a page has started to leave, the navigation may land at any time, or never (the user
//! stopped it; nothing tells the page which). So from then on the stored draft follows the
//! settings page's edits ([`follow`]): what lands is the latest edits, and once they are saved or
//! the settings page is closed nothing stale is left to restore (review C4C2-01, C4C2-02).
//!
//! Only the edits, not the whole settings: a draft restored over settings changed elsewhere
//! changes only what the user edited.

use std::cell::Cell;

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
        platform::remove(KEY);
    }
    edits
}

/// The page has the kept draft (and the shell keeps it from now on): it goes, so it is restored
/// once. Once this page is leaving the stored draft is the page's edits as they follow
/// ([`follow`]), so it stays: the navigation may still land (review C4B-B-R2-01).
pub(crate) fn forget() {
    if !LEAVING.with(Cell::get) {
        platform::remove(KEY);
    }
}

thread_local! {
    /// This page started to leave (reload, sign-in) with its draft kept. It stays set: a stopped
    /// navigation tells the page nothing (no event, no time limit that proves it).
    static LEAVING: Cell<bool> = const { Cell::new(false) };
}

/// The settings page's edits not stored yet changed (`None`: saved, none, or the page closed).
/// Once this page is leaving, the stored draft follows them: the navigation may land later with
/// the latest edits, and edits saved or dropped are not restored afterwards. Before that, nothing
/// to do.
///
/// When the latest edits cannot be kept, the page does not leave, as when the first ones could
/// not (C4b item 4; review C4C2-R2-01): the departure is stopped (`window.stop()`, which also
/// cancels this page's requests in flight; they end as a lost answer does, failed or unknown),
/// the stored draft, now stale, goes, and the caller is told.
pub(crate) fn follow(draft: Option<&SettingsPatch>) -> Result<(), NotKept> {
    if !LEAVING.with(Cell::get) {
        return Ok(());
    }
    if draft.is_none_or(SettingsPatch::is_empty) {
        platform::remove(KEY);
        return Ok(());
    }
    keep(draft).inspect_err(|_| {
        platform::stop();
        LEAVING.with(|leaving| leaving.set(false));
        platform::remove(KEY);
    })
}

/// Leaves (`leave`: reload, go to the sign-in page) after keeping `draft`; stays, returning false,
/// when it could not be kept. From then on the stored draft follows the settings page's edits.
pub(crate) fn leave_keeping(draft: Option<&SettingsPatch>, leave: impl FnOnce()) -> bool {
    if keep(draft).is_err() {
        return false;
    }
    LEAVING.with(|leaving| leaving.set(true));
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

    /// Stops the navigation under way (and this document's loading).
    pub(super) fn stop() {
        if let Some(window) = web_sys::window() {
            let _ = window.stop();
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

    pub(super) fn stop() {}
}

/// The tests' tab: a storage per test thread that can be made to refuse writes.
#[cfg(all(not(target_arch = "wasm32"), test))]
pub(crate) mod platform {
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static STORAGE: RefCell<HashMap<String, String>> = RefCell::default();
        static REFUSING: RefCell<bool> = const { RefCell::new(false) };
        static STOPPED: RefCell<bool> = const { RefCell::new(false) };
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

    pub(super) fn stop() {
        STOPPED.with(|stopped| *stopped.borrow_mut() = true);
    }

    /// Whether the departure was stopped.
    pub(crate) fn stopped() -> bool {
        STOPPED.with(|stopped| *stopped.borrow())
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

    use super::{KEY, LEAVING, NotKept, follow, forget, keep, leave_keeping, peek, platform};

    /// What the next settings page restores: the draft, then gone.
    fn take() -> Option<SettingsPatch> {
        let draft = peek();
        forget();
        draft
    }

    /// A settings page opening as the app's does: it reads the kept draft, hands it to the shell
    /// (whose changes the stored draft follows), then has it forgotten.
    fn open_settings() -> Option<SettingsPatch> {
        let draft = peek();
        follow(draft.as_ref()).expect("kept");
        forget();
        draft
    }

    /// The settings page closed (or its edits saved): the shell has no edits any more.
    fn close_settings() {
        follow(None).expect("nothing to keep");
    }

    /// The page that comes after a navigation landed: this tab's storage, a fresh app.
    fn next_document() {
        LEAVING.with(|leaving| leaving.set(false));
    }

    fn stored() -> Option<SettingsPatch> {
        platform::read(KEY)
            .and_then(|raw| serde_json::from_str::<super::Stored>(&raw).ok())
            .map(|stored| stored.edits)
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

    /// A draft waiting for the next settings page while the app leaves from another page, the
    /// navigation then stopped: the settings page opened later restores it once, however soon or
    /// late, and opening it again restores nothing (review C4C2-01, and the C4c acceptance's
    /// finding before it).
    #[test]
    fn a_draft_restored_after_a_stopped_departure_is_restored_once() {
        keep(Some(&dark())).expect("kept by an earlier page");
        next_document();
        assert!(leave_keeping(None, || {}), "leaves from another page");
        assert_eq!(stored(), Some(dark()), "still waiting for a settings page");
        assert_eq!(open_settings(), Some(dark()), "restored");
        assert_eq!(stored(), Some(dark()), "stored: the navigation may still land");
        close_settings();
        assert_eq!(open_settings(), None, "not restored a second time");
        next_document();
        assert_eq!(open_settings(), None, "nor by a later page");
    }

    /// While the page is leaving, the stored draft follows the settings page's edits: a
    /// navigation that lands late (no time limit proves it stopped) brings the latest edits
    /// (review C4C2-02).
    #[test]
    fn a_departure_that_lands_late_restores_the_latest_edits() {
        assert!(leave_keeping(Some(&dark()), || {}));
        let more =
            SettingsPatch { startup_view: Some(rssr_domain::StartupView::LastFeed), ..dark() };
        follow(Some(&more)).expect("kept");
        assert_eq!(stored(), Some(more.clone()), "follows the edits");
        next_document();
        assert_eq!(open_settings(), Some(more), "the latest edits, once");
        assert_eq!(stored(), None);
    }

    /// Edits saved (or dropped with the settings page) while the page is leaving are not
    /// restored later: nothing stale is left stored (review C4B-B-R3-04 without a time limit).
    #[test]
    fn edits_saved_during_a_departure_are_not_restored_later() {
        assert!(leave_keeping(Some(&dark()), || {}));
        follow(None).expect("nothing to keep");
        assert_eq!(stored(), None, "saved: gone");
        next_document();
        assert_eq!(open_settings(), None);
    }

    /// Later edits that cannot be kept while the page leaves: the page does not leave, as when the
    /// first ones could not (review C4C2-R2-01). The departure is stopped, the stale draft goes and
    /// nothing follows any more.
    #[test]
    fn later_edits_that_cannot_be_kept_stop_the_departure() {
        assert!(leave_keeping(Some(&dark()), || {}));
        let more =
            SettingsPatch { startup_view: Some(rssr_domain::StartupView::LastFeed), ..dark() };
        platform::refuse_writes(true);
        assert_eq!(follow(Some(&more)), Err(NotKept));
        assert!(platform::stopped(), "the navigation stopped");
        assert_eq!(stored(), None, "the stale draft went");
        platform::refuse_writes(false);
        assert_eq!(follow(Some(&more)), Ok(()));
        assert_eq!(stored(), None, "the page stays: nothing follows");
    }

    /// Before the page leaves, the settings page's edits are not stored, and a restored draft goes
    /// as the page takes it.
    #[test]
    fn edits_are_stored_only_once_the_page_leaves() {
        follow(Some(&dark())).expect("not leaving");
        assert_eq!(stored(), None, "not leaving");
        keep(Some(&dark())).expect("kept");
        assert_eq!(open_settings(), Some(dark()));
        assert_eq!(stored(), None, "restored once");
    }
}

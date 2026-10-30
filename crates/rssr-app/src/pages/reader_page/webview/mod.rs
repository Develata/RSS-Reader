//! Reader's current WebView adapter. Selectors, mount timing, synchronous input
//! handling, modal focus/scroll and bridge cleanup belong here. Page rules only
//! receive image values, raw keys and close requests; this is not a general host API.
use std::{cell::Cell, rc::Rc};

use dioxus::prelude::*;

use super::image_viewer_state::ReaderImage;

// Each bridge gets the same synchronous predicate and a projection of the Rust
// bindings. Keep them local to Eval: no global registration or readiness race.
pub(crate) fn shortcut_script(body: &str) -> String {
    let bindings = crate::hooks::reader_shortcut_policy::BINDINGS
        .map(|(key, action)| (key.to_ascii_lowercase(), action.captures_position()))
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
    format!(
        "const readerShortcutBindings = {};\n{}\n{body}",
        serde_json::json!(bindings),
        include_str!("shortcut_input.js"),
    )
}

// Keep cancellation in the owning Dioxus scope, including a scope dropped while
// its receive task is waiting. The task itself keeps its existing scope lifetime.
fn use_listener_cleanup() -> Rc<Cell<Option<document::Eval>>> {
    let bridge = use_hook(|| Rc::new(Cell::new(None::<document::Eval>)));
    let cleanup = Rc::clone(&bridge);
    use_drop(move || {
        if let Some(eval) = cleanup.get() {
            let _ = eval.send(());
        }
    });
    bridge
}

pub(super) fn use_image_activations(on_image: Callback<ReaderImage>) {
    let bridge = use_listener_cleanup();
    use_future(move || {
        let bridge = Rc::clone(&bridge);
        async move {
            let mut eval = document::eval(include_str!("image_events.js"));
            bridge.set(Some(eval));
            while let Ok(image) = eval.recv::<ReaderImage>().await {
                on_image.call(image);
            }
        }
    });
}

/// Called after the dialog is mounted. The view need not know about Eval,
/// native dialog cancellation, scroll locking or the bridge's stop message.
pub(super) fn use_image_dialog(on_close: Callback<()>) -> Callback<()> {
    let bridge = use_listener_cleanup();
    use_callback(move |()| {
        let mut eval = document::eval(include_str!("image_dialog.js"));
        bridge.set(Some(eval));
        spawn(async move {
            if eval.recv::<()>().await.is_ok() {
                on_close.call(());
            }
        });
    })
}

pub(crate) fn use_shortcut_keys(on_key: Callback<String>) {
    let bridge = use_listener_cleanup();
    use_future(move || {
        let bridge = Rc::clone(&bridge);
        async move {
            let mut eval = document::eval(&shortcut_script(include_str!("reader_shortcuts.js")));
            bridge.set(Some(eval));
            while let Ok(key) = eval.recv::<String>().await {
                on_key.call(key);
            }
        }
    });
}

use dioxus::prelude::*;

pub(crate) fn initial_entry_search() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window()
            && let Ok(Some(storage)) = window.local_storage()
            && let Ok(Some(value)) = storage.get_item("rssr-entry-search")
        {
            return value;
        }
    }

    String::new()
}

pub(crate) fn remember_entry_search(_value: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window()
            && let Ok(Some(storage)) = window.local_storage()
        {
            let _ = storage.set_item("rssr-entry-search", _value);
        }
    }
}

pub(crate) fn initial_nav_hidden() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window()
            && let Ok(Some(storage)) = window.local_storage()
            && let Ok(Some(value)) = storage.get_item("rssr-nav-hidden")
        {
            return value == "1";
        }
    }

    false
}

pub(crate) fn remember_nav_hidden(_hidden: bool) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window()
            && let Ok(Some(storage)) = window.local_storage()
        {
            let _ = storage.set_item("rssr-nav-hidden", if _hidden { "1" } else { "0" });
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn complete_web_auth_transition(on_authenticated: EventHandler<()>) {
    if let Some(window) = web_sys::window()
        && window.location().reload().is_ok()
    {
        return;
    }

    on_authenticated.call(());
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn complete_web_auth_transition(on_authenticated: EventHandler<()>) {
    on_authenticated.call(());
}

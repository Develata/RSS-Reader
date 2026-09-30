use dioxus::prelude::*;

#[component]
pub fn StatusBanner(
    message: String,
    tone: Option<String>,
    #[props(default)] announce: bool,
    #[props(default)] visually_hidden: bool,
) -> Element {
    let empty = message.trim().is_empty();
    if empty && !announce {
        return rsx! {};
    }

    let tone = tone.unwrap_or_else(|| "info".to_string());

    rsx! {
        // Dynamic callers keep this component mounted. An empty live region stays
        // accessible without taking layout space; later text updates can be announced.
        p {
            class: if empty || visually_hidden { "sr-only" } else { "status-banner" },
            "data-layout": "status-banner",
            "data-state": "{tone}",
            role: announce.then_some("status"),
            aria_live: announce.then_some("polite"),
            aria_atomic: announce.then_some("true"),
            "{message}"
        }
    }
}

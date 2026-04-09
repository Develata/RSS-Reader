use dioxus::prelude::*;

#[component]
pub fn StatusBanner(message: String, tone: Option<String>) -> Element {
    if message.trim().is_empty() {
        return rsx! {};
    }

    let tone = tone.unwrap_or_else(|| "info".to_string());

    rsx! {
        p { class: "status-banner", "data-state": "{tone}", "{message}" }
    }
}

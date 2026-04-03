use dioxus::prelude::{Signal, WritableExt};

pub fn set_status_info(
    mut status: Signal<String>,
    mut status_tone: Signal<String>,
    message: impl Into<String>,
) {
    status.set(message.into());
    status_tone.set("info".to_string());
}

pub fn set_status_error(
    mut status: Signal<String>,
    mut status_tone: Signal<String>,
    message: impl Into<String>,
) {
    status.set(message.into());
    status_tone.set("error".to_string());
}

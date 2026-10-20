use dioxus::prelude::{Signal, WritableExt};

pub fn set_status_info(
    mut status: Signal<String>,
    mut status_tone: Signal<String>,
    message: impl Into<String>,
) {
    status.set(message.into());
    status_tone.set("info".to_string());
}

/// Sets the status and its tone (`info`, `error`, `success`, or `unknown` for an outcome that may
/// have happened).
pub fn set_status(
    mut status: Signal<String>,
    mut status_tone: Signal<String>,
    message: impl Into<String>,
    tone: impl Into<String>,
) {
    status.set(message.into());
    status_tone.set(tone.into());
}

pub fn set_status_error(
    mut status: Signal<String>,
    mut status_tone: Signal<String>,
    message: impl Into<String>,
) {
    status.set(message.into());
    status_tone.set("error".to_string());
}

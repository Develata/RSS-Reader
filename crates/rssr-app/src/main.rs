mod app;
mod bootstrap;
mod components;
mod hooks;
mod pages;
mod router;
mod theme;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

use dioxus::prelude::LaunchBuilder;
    use tracing_subscriber::EnvFilter;

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,dioxus_desktop::edits=off")),
        )
        .init();
    let window = dioxus::desktop::WindowBuilder::new()
        .with_title("RSS Reader")
        .with_inner_size(dioxus::desktop::LogicalSize::new(1280.0, 900.0))
        .with_visible(true)
        .with_focused(true)
        .with_decorations(true)
        .with_resizable(true)
        .with_minimizable(true)
        .with_maximizable(true)
        .with_closable(true);

    let config = dioxus::desktop::Config::new().with_window(window).with_menu(None);

    LaunchBuilder::new().with_cfg(config).launch(app::App);
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use tracing_subscriber::EnvFilter;

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,dioxus_desktop::edits=off")),
        )
        .init();
    dioxus::launch(app::App);
}

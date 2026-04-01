#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod bootstrap;
mod components;
mod hooks;
mod pages;
mod router;
mod theme;
mod web_auth;

use tracing_subscriber::EnvFilter;

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,dioxus_desktop::edits=off")),
        )
        .init();
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn main() {
    use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
    use dioxus::prelude::LaunchBuilder;

    init_tracing();

    let window = WindowBuilder::new()
        .with_title("RSS-Reader")
        .with_inner_size(LogicalSize::new(1280.0, 900.0))
        .with_visible(true)
        .with_focused(true)
        .with_decorations(true)
        .with_resizable(true)
        .with_minimizable(true)
        .with_maximizable(true)
        .with_closable(true);

    let config = Config::new().with_window(window).with_menu(None);
    LaunchBuilder::new().with_cfg(config).launch(app::App);
}

#[cfg(target_os = "android")]
fn main() {
    init_tracing();
    dioxus::launch(app::App);
}

#[cfg(target_arch = "wasm32")]
fn main() {
    init_tracing();
    dioxus::launch(app::App);
}

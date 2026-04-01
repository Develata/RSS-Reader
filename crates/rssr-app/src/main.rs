#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod bootstrap;
mod components;
mod hooks;
mod pages;
mod router;
mod theme;
mod web_auth;

#[cfg(not(target_arch = "wasm32"))]
use tracing_subscriber::EnvFilter;

#[cfg(target_arch = "wasm32")]
fn init_tracing() {
    let mut builder = tracing_wasm::WASMLayerConfigBuilder::new();
    builder.set_max_level(tracing::Level::INFO);
    tracing_wasm::set_as_global_default_with_config(builder.build());
}

#[cfg(not(target_arch = "wasm32"))]
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
    use dioxus::desktop::{Config, LogicalSize, WindowBuilder, tao::window::Icon};
    use dioxus::prelude::LaunchBuilder;
    use image::ImageFormat;

    init_tracing();

    let window_icon = load_window_icon();
    let window = WindowBuilder::new()
        .with_title("RSS-Reader")
        .with_window_icon(window_icon)
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

    fn load_window_icon() -> Option<Icon> {
        let image = image::load_from_memory_with_format(
            include_bytes!("../../../icons/icon.png"),
            ImageFormat::Png,
        )
        .ok()?
        .into_rgba8();
        let (width, height) = image.dimensions();
        Icon::from_rgba(image.into_raw(), width, height).ok()
    }
}

#[cfg(target_os = "android")]
fn main() {
    init_tracing();
    dioxus::launch(app::App);
}

#[cfg(target_arch = "wasm32")]
fn main() {
    console_error_panic_hook::set_once();
    init_tracing();
    dioxus::launch(app::App);
}

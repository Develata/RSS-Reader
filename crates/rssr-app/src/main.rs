mod app;
mod bootstrap;
mod components;
mod hooks;
mod pages;
mod router;
mod theme;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();
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

    let config = dioxus::desktop::Config::new().with_window(window);

    dioxus::LaunchBuilder::desktop()
        .with_cfg(config)
        .launch(app::App);
}

#[cfg(target_arch = "wasm32")]
fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();
    dioxus::LaunchBuilder::web().launch(app::App);
}

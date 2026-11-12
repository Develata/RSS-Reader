#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]
// Rust 1.99 Clippy false-positive from async_trait macro expansion:
// https://github.com/rust-lang/rust-clippy/issues/17529
#![allow(clippy::double_must_use)]

mod app;
mod bootstrap;
mod components;
mod datetime;
mod demo;
mod hooks;
mod pages;
mod router;
mod status;
mod theme;
mod ui;

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
    use dioxus::desktop::{
        Config, LogicalSize, WindowBuilder, tao::event::Event, tao::window::Icon,
    };
    use dioxus::prelude::LaunchBuilder;
    use image::ImageFormat;

    init_tracing();

    let window_icon = load_window_icon();
    let window = WindowBuilder::new()
        .with_title(app::APP_NAME)
        .with_window_icon(window_icon)
        .with_inner_size(LogicalSize::new(1280.0, 900.0))
        .with_visible(true)
        .with_focused(true)
        .with_decorations(true)
        .with_resizable(true)
        .with_minimizable(true)
        .with_maximizable(true)
        .with_closable(true);

    let config = Config::new()
        .with_window(window)
        // Must follow `with_window`: Dioxus only applies a menu to decorated windows.
        .with_menu(desktop_menu())
        // Release builds disable WebView context menus by default. Keep the
        // native Copy/Select All actions available while reading. Dioxus
        // 0.7.9 also enables WebView devtools with this setting.
        .with_disable_context_menu(false)
        .with_custom_head(bootstrap::native_document::head())
        // The event loop's last event: the process exits right after it, so the library is closed
        // here (its log checkpointed into the main file) rather than cut off mid-close.
        .with_custom_event_handler(|event, _| {
            if let Event::LoopDestroyed = event {
                bootstrap::close_local_library();
            }
        });
    LaunchBuilder::new().with_cfg(config).launch(app::App);

    /// Windows and Linux keep no menu bar: WebView2 and WebKitGTK handle the
    /// Ctrl editing keys inside the page themselves.
    #[cfg(not(target_os = "macos"))]
    fn desktop_menu() -> Option<dioxus::desktop::muda::Menu> {
        None
    }

    /// macOS delivers Cmd+C / V / X / Z / A, and Cmd+Q / W / M / H, only as key
    /// equivalents of the app's main menu; WKWebView has no fallback of its own.
    /// Without these menus those shortcuts do nothing. Every item is a
    /// predefined AppKit action, so the app adds no shortcut handling of its own.
    #[cfg(target_os = "macos")]
    fn desktop_menu() -> Option<dioxus::desktop::muda::Menu> {
        use dioxus::desktop::muda::{AboutMetadata, Menu, PredefinedMenuItem, Submenu};

        let build = || -> dioxus::desktop::muda::Result<Menu> {
            let name = app::APP_NAME;
            // AppKit turns the first submenu into the application menu, titled
            // with the process name whatever title is given here.
            let app_menu = Submenu::with_items(
                name,
                true,
                &[
                    &PredefinedMenuItem::about(
                        Some(&format!("关于 {name}")),
                        Some(AboutMetadata {
                            name: Some(name.to_owned()),
                            version: Some(env!("CARGO_PKG_VERSION").to_owned()),
                            ..Default::default()
                        }),
                    ),
                    &PredefinedMenuItem::separator(),
                    &PredefinedMenuItem::services(Some("服务")),
                    &PredefinedMenuItem::separator(),
                    &PredefinedMenuItem::hide(Some(&format!("隐藏 {name}"))),
                    &PredefinedMenuItem::hide_others(Some("隐藏其他")),
                    &PredefinedMenuItem::show_all(Some("全部显示")),
                    &PredefinedMenuItem::separator(),
                    &PredefinedMenuItem::quit(Some(&format!("退出 {name}"))),
                ],
            )?;
            let edit_menu = Submenu::with_items(
                "编辑",
                true,
                &[
                    &PredefinedMenuItem::undo(Some("撤销")),
                    &PredefinedMenuItem::redo(Some("重做")),
                    &PredefinedMenuItem::separator(),
                    &PredefinedMenuItem::cut(Some("剪切")),
                    &PredefinedMenuItem::copy(Some("拷贝")),
                    &PredefinedMenuItem::paste(Some("粘贴")),
                    &PredefinedMenuItem::select_all(Some("全选")),
                ],
            )?;
            let window_menu = Submenu::with_items(
                "窗口",
                true,
                &[
                    &PredefinedMenuItem::minimize(Some("最小化")),
                    &PredefinedMenuItem::maximize(Some("缩放")),
                    &PredefinedMenuItem::fullscreen(Some("进入全屏幕")),
                    &PredefinedMenuItem::separator(),
                    &PredefinedMenuItem::close_window(Some("关闭窗口")),
                ],
            )?;
            window_menu.set_as_windows_menu_for_nsapp();
            Menu::with_items(&[&app_menu, &edit_menu, &window_menu])
        };
        build().inspect_err(|error| tracing::warn!(%error, "macOS menu bar unavailable")).ok()
    }

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
    use dioxus::mobile::{Config, WindowCloseBehaviour};
    use dioxus::prelude::LaunchBuilder;

    init_tracing();
    let config = Config::new()
        .with_close_behaviour(WindowCloseBehaviour::WindowHides)
        .with_disable_context_menu(false)
        .with_custom_head(bootstrap::native_document::head());
    LaunchBuilder::new().with_cfg(config).launch(app::App);
}

#[cfg(target_arch = "wasm32")]
fn main() {
    console_error_panic_hook::set_once();
    init_tracing();
    bootstrap::install_history_capture();
    dioxus::launch(app::App);
}

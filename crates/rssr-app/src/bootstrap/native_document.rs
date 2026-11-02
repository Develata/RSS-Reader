//! Native WebView document setup; no page or settings state lives here.

// Dioxus 0.7.9 / sledgehammer_bindgen_macro 0.6.5 decodes a whole mutation
// string table, then slices it using UTF-16 lengths supplied by Rust. A BOM at
// the start is data: stripping it shifts every subsequent field in the batch.
// Patch only that interpreter's decoder, not the browser's TextDecoder or the
// imported CSS. Dioxus constructs the interpreter in a module script and starts
// receiving edits on load, so DOMContentLoaded runs between those two steps.
// Recheck this private `c` field and startup order when updating Dioxus; the
// native transport acceptance probe exercises the real emitted string table.
const PRESERVE_TRANSPORT_BOM: &str = r#"
document.addEventListener("DOMContentLoaded", () => {
    const interpreter = window.interpreter;
    if (interpreter?.c instanceof TextDecoder && !interpreter.c.ignoreBOM) {
        interpreter.c = new TextDecoder("utf-8", { ignoreBOM: true });
    }
}, { once: true });
"#;

/// WebKitGTK restarts its smooth-scroll animation on every wheel event, so a fast wheel
/// (hundreds of events a second) moves the page only a few pixels per frame. Let wheel steps
/// scroll directly; WebView2 and WKWebView keep their own scrolling.
#[cfg(target_os = "linux")]
pub(crate) fn use_direct_wheel_scrolling() {
    use dioxus::desktop::wry::WebViewExtUnix;
    use webkit2gtk::{SettingsExt, WebViewExt};

    dioxus::prelude::use_hook(|| {
        if let Some(settings) = dioxus::desktop::window().webview.webview().settings() {
            settings.set_enable_smooth_scrolling(false);
        }
    });
}

pub(crate) fn head() -> String {
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    let icon = STANDARD.encode(include_bytes!("../../../../assets/branding/rssr-mark.svg"));
    // Native Title updates the window, not document.title. Install branding
    // before App mounts so WebView never probes a missing /favicon.ico.
    format!(
        r#"<script>document.title = {title:?};{PRESERVE_TRANSPORT_BOM}</script><link rel="icon" type="image/svg+xml" href="data:image/svg+xml;base64,{icon}">"#,
        title = crate::app::APP_NAME,
    )
}

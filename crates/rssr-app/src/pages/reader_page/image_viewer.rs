use super::{
    image_viewer_state::{ImageViewerState, ReaderImage},
    webview,
};
use dioxus::prelude::*;

pub(super) fn use_reader_image_viewer(entry_id: i64) -> Signal<ImageViewerState> {
    let mut state = use_signal(ImageViewerState::default);
    crate::ui::use_reactive_side_effect(entry_id, move |_| {
        if matches!(*state.peek(), ImageViewerState::Open(_)) {
            state.write().close();
        }
    });
    let on_image = use_callback(move |image| state.write().open(image));
    webview::use_image_activations(on_image);
    state
}

#[component]
pub(super) fn ReaderImageViewer(image: ReaderImage, on_close: EventHandler<()>) -> Element {
    let on_mounted = webview::use_image_dialog(on_close);
    rsx! {
        dialog {
            "data-layout": "reader-image-viewer", aria_label: "图片查看器",
            onmounted: move |_| on_mounted.call(()),
            button {
                class: "icon-link-button", "data-action": "close-reader-image", r#type: "button",
                aria_label: "关闭图片", title: "关闭图片", onclick: move |_| on_close.call(()),
                "×"
            }
            div { "data-layout": "reader-image-viewport",
                img { src: image.src, alt: image.alt }
            }
        }
    }
}

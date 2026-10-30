use std::{cell::Cell, rc::Rc};

use super::image_viewer_state::{ImageViewerState, ReaderImage};
use dioxus::prelude::*;

pub(super) fn use_reader_image_viewer(entry_id: i64) -> Signal<ImageViewerState> {
    let mut state = use_signal(ImageViewerState::default);
    crate::ui::use_reactive_side_effect(entry_id, move |_| {
        if matches!(*state.peek(), ImageViewerState::Open(_)) {
            state.write().close();
        }
    });
    let bridge = use_hook(|| Rc::new(Cell::new(None::<document::Eval>)));
    let cleanup = Rc::clone(&bridge);
    use_drop(move || {
        if let Some(eval) = cleanup.get() {
            let _ = eval.send(());
        }
    });
    use_future(move || {
        let bridge = Rc::clone(&bridge);
        async move {
            let mut eval = document::eval(include_str!("image_events.js"));
            bridge.set(Some(eval));
            while let Ok(image) = eval.recv::<ReaderImage>().await {
                state.write().open(image);
            }
        }
    });
    state
}

#[component]
pub(super) fn ReaderImageViewer(image: ReaderImage, on_close: EventHandler<()>) -> Element {
    let bridge = use_hook(|| Rc::new(Cell::new(None::<document::Eval>)));
    let cleanup = Rc::clone(&bridge);
    use_drop(move || {
        if let Some(eval) = cleanup.get() {
            let _ = eval.send(());
        }
    });
    rsx! {
        dialog {
            "data-layout": "reader-image-viewer", aria_label: "图片查看器",
            onmounted: move |_| {
                let mut eval = document::eval(include_str!("image_dialog.js"));
                bridge.set(Some(eval));
                spawn(async move {
                    // Native dialog cancel/backdrop events request the Rust close transition.
                    if eval.recv::<()>().await.is_ok() { on_close.call(()); }
                });
            },
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

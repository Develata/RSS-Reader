use std::{cell::Cell, rc::Rc};

use dioxus::prelude::*;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(super) struct ReaderImage {
    src: String,
    alt: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) enum ImageViewerState {
    #[default]
    Closed,
    Open(ReaderImage),
}

impl ImageViewerState {
    fn open(&mut self, image: ReaderImage) {
        if !image.src.trim().is_empty() {
            *self = Self::Open(image);
        }
    }

    pub(super) fn close(&mut self) -> bool {
        let was_open = matches!(self, Self::Open(_));
        *self = Self::Closed;
        was_open
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewer_reuses_rendered_source_and_close_is_idempotent() {
        let mut state = ImageViewerState::Closed;
        assert!(!state.close());
        state.open(ReaderImage { src: " ".into(), alt: String::new() });
        assert_eq!(state, ImageViewerState::Closed);
        for src in [
            "data:image/png;base64,abcd",
            "asset://local/image.png",
            "https://example.com/图 片.png",
        ] {
            let image = ReaderImage { src: src.into(), alt: "中文 image".into() };
            state.open(image.clone());
            assert_eq!(state, ImageViewerState::Open(image));
            assert!(state.close());
            assert!(!state.close());
        }
    }
}

//! Image selection and close consumption, independent of the modal host.
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(super) struct ReaderImage {
    pub(super) src: String,
    pub(super) alt: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) enum ImageViewerState {
    #[default]
    Closed,
    Open(ReaderImage),
}

impl ImageViewerState {
    pub(super) fn open(&mut self, image: ReaderImage) {
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

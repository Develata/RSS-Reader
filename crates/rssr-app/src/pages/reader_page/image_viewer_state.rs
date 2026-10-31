//! Image selection and close consumption, independent of the modal host.
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(super) struct ReaderImage {
    pub(super) src: String,
    pub(super) alt: String,
}

/// The DOM bridge carries the existing Reader load generation as text, without
/// passing a u64 through JavaScript's lossy number representation.
#[derive(Debug, Deserialize)]
pub(super) struct ReaderImageActivation {
    generation: String,
    #[serde(flatten)]
    image: ReaderImage,
}

impl ReaderImageActivation {
    pub(super) fn into_current_image(self, generation: u64) -> Option<ReaderImage> {
        (self.generation.parse::<u64>() == Ok(generation)).then_some(self.image)
    }
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
    fn image_activation_belongs_to_the_original_visit_even_after_returning_to_the_entry() {
        let activation = || ReaderImageActivation {
            generation: "1".into(),
            image: ReaderImage { src: "data:image/png;base64,abcd".into(), alt: "A".into() },
        };
        assert!(activation().into_current_image(1).is_some());
        assert!(activation().into_current_image(2).is_none(), "another entry");
        assert!(activation().into_current_image(3).is_none(), "a new visit to A");
    }

    #[test]
    fn image_generation_rejects_invalid_tokens_and_preserves_u64_precision() {
        for token in ["", "not-a-generation", "1.0", "-1", "18446744073709551616"] {
            let activation = ReaderImageActivation {
                generation: token.into(),
                image: ReaderImage { src: "image.png".into(), alt: String::new() },
            };
            assert!(activation.into_current_image(1).is_none());
        }
        let activation = ReaderImageActivation {
            generation: u64::MAX.to_string(),
            image: ReaderImage { src: "image.png".into(), alt: String::new() },
        };
        assert!(activation.into_current_image(u64::MAX).is_some());
    }

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

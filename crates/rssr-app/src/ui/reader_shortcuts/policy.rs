//! Reader key decisions, independent of the renderer, router and page session.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReaderShortcut {
    Previous,
    Next,
    ToggleRead,
    ToggleStarred,
}

// Logical key names, shared by Rust dispatch and the current host projection.
// Single-character bindings accept ASCII capitals; named keys remain exact.
pub(super) const BINDINGS: [(&str, ReaderShortcut); 6] = [
    ("ArrowLeft", ReaderShortcut::Previous),
    ("k", ReaderShortcut::Previous),
    ("ArrowRight", ReaderShortcut::Next),
    ("j", ReaderShortcut::Next),
    ("m", ReaderShortcut::ToggleRead),
    ("f", ReaderShortcut::ToggleStarred),
];

impl ReaderShortcut {
    pub(super) const fn captures_position(self) -> bool {
        matches!(self, Self::Previous | Self::Next)
    }
}

/// Leave system combinations and IME input untouched. Decode the key lazily so
/// excluded events need not clone their renderer-owned key data. Key repeat and
/// native default actions retain their existing behavior.
pub(crate) fn resolve<'a>(
    modified: bool,
    composing: bool,
    key: impl FnOnce() -> &'a str,
) -> Option<ReaderShortcut> {
    if modified || composing {
        return None;
    }
    let key = key();
    BINDINGS.iter().find_map(|&(bound, action)| {
        (key == bound || (bound.len() == 1 && key.eq_ignore_ascii_case(bound))).then_some(action)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_keys_keep_the_existing_actions_and_case_handling() {
        for (key, expected) in [
            ("ArrowLeft", Some(ReaderShortcut::Previous)),
            ("k", Some(ReaderShortcut::Previous)),
            ("K", Some(ReaderShortcut::Previous)),
            ("ArrowRight", Some(ReaderShortcut::Next)),
            ("j", Some(ReaderShortcut::Next)),
            ("J", Some(ReaderShortcut::Next)),
            ("m", Some(ReaderShortcut::ToggleRead)),
            ("M", Some(ReaderShortcut::ToggleRead)),
            ("f", Some(ReaderShortcut::ToggleStarred)),
            ("F", Some(ReaderShortcut::ToggleStarred)),
            ("ｍ", None),
            ("K", None),
            ("r", None),
            ("", None),
            ("jk", None),
            ("arrowleft", None),
            ("ARROWRIGHT", None),
        ] {
            assert_eq!(resolve(false, false, || key), expected);
        }
    }

    #[test]
    fn only_navigation_captures_position_before_dispatch() {
        for key in ["ArrowLeft", "ArrowRight", "j", "J", "k", "K"] {
            assert!(resolve(false, false, || key).unwrap().captures_position());
        }
        for key in ["m", "M", "f", "F"] {
            assert!(!resolve(false, false, || key).unwrap().captures_position());
        }
    }

    #[test]
    fn modifiers_and_composition_bypass_even_key_decoding() {
        for (modified, composing) in [(true, false), (false, true), (true, true)] {
            assert_eq!(resolve(modified, composing, || panic!("must not decode")), None);
        }
    }
}

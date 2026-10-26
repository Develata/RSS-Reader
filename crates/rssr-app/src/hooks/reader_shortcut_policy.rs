//! Reader key decisions, independent of the renderer, router and page session.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ShortcutKey {
    Previous,
    Next,
    Character(char),
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReaderShortcut {
    Previous,
    Next,
    ToggleRead,
    ToggleStarred,
}

/// Leave system combinations and IME input untouched. Decode the key lazily so
/// excluded events need not clone their renderer-owned key data. Key repeat and
/// native default actions retain their existing behavior.
pub(super) fn resolve(
    modified: bool,
    composing: bool,
    key: impl FnOnce() -> ShortcutKey,
) -> Option<ReaderShortcut> {
    if modified || composing {
        return None;
    }
    match key() {
        ShortcutKey::Previous => Some(ReaderShortcut::Previous),
        ShortcutKey::Next => Some(ReaderShortcut::Next),
        ShortcutKey::Character('m' | 'M') => Some(ReaderShortcut::ToggleRead),
        ShortcutKey::Character('f' | 'F') => Some(ReaderShortcut::ToggleStarred),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_keys_keep_the_existing_actions_and_case_handling() {
        for (key, expected) in [
            (ShortcutKey::Previous, Some(ReaderShortcut::Previous)),
            (ShortcutKey::Next, Some(ReaderShortcut::Next)),
            (ShortcutKey::Character('m'), Some(ReaderShortcut::ToggleRead)),
            (ShortcutKey::Character('M'), Some(ReaderShortcut::ToggleRead)),
            (ShortcutKey::Character('f'), Some(ReaderShortcut::ToggleStarred)),
            (ShortcutKey::Character('F'), Some(ReaderShortcut::ToggleStarred)),
            (ShortcutKey::Character('ｍ'), None),
            (ShortcutKey::Character('r'), None),
            (ShortcutKey::Other, None),
        ] {
            assert_eq!(resolve(false, false, || key), expected);
        }
    }

    #[test]
    fn modifiers_and_composition_bypass_even_key_decoding() {
        for (modified, composing) in [(true, false), (false, true), (true, true)] {
            assert_eq!(resolve(modified, composing, || panic!("must not decode")), None);
        }
    }
}

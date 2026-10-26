use dioxus::prelude::*;

use super::reader_shortcut_policy::{self, ReaderShortcut, ShortcutKey};
use crate::{pages::reader_page::ReaderPageSession, router::AppRoute};

pub fn use_reader_shortcuts(session: ReaderPageSession) -> Callback<KeyboardEvent> {
    let navigator = use_navigator();

    use_callback(move |event: KeyboardEvent| {
        // Keep the modifier/IME boundary aligned with reading_position.js: it
        // captures position before the same plain arrow keys navigate away.
        let action = reader_shortcut_policy::resolve(
            !event.modifiers().is_empty(),
            event.is_composing(),
            || shortcut_key(&event.key()),
        );
        match action {
            Some(ReaderShortcut::Previous) => {
                if let Some(target) = session.previous_entry_target() {
                    navigator.push(AppRoute::ReaderPage { entry_id: target });
                }
            }
            Some(ReaderShortcut::Next) => {
                if let Some(target) = session.next_entry_target() {
                    navigator.push(AppRoute::ReaderPage { entry_id: target });
                }
            }
            Some(ReaderShortcut::ToggleRead) => session.toggle_read(true),
            Some(ReaderShortcut::ToggleStarred) => session.toggle_starred(true),
            None => {}
        }
    })
}

fn shortcut_key(key: &Key) -> ShortcutKey {
    match key {
        Key::ArrowLeft => ShortcutKey::Previous,
        Key::ArrowRight => ShortcutKey::Next,
        Key::Character(text) => {
            let mut chars = text.chars();
            match (chars.next(), chars.next()) {
                (Some(character), None) => ShortcutKey::Character(character),
                _ => ShortcutKey::Other,
            }
        }
        _ => ShortcutKey::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dioxus_keys_map_without_formatting_or_case_conversion() {
        for (key, expected) in [
            (Key::ArrowLeft, ShortcutKey::Previous),
            (Key::ArrowRight, ShortcutKey::Next),
            (Key::Character("M".into()), ShortcutKey::Character('M')),
            (Key::Character("f".into()), ShortcutKey::Character('f')),
            (Key::Character("ｍ".into()), ShortcutKey::Character('ｍ')),
            (Key::Character("".into()), ShortcutKey::Other),
            (Key::Character("mf".into()), ShortcutKey::Other),
            (Key::ArrowUp, ShortcutKey::Other),
            (Key::Unidentified, ShortcutKey::Other),
        ] {
            assert_eq!(shortcut_key(&key), expected);
        }
    }
}

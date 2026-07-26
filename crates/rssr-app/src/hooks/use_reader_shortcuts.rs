use dioxus::prelude::*;

use crate::{pages::reader_page::ReaderPageSession, router::AppRoute};

pub fn use_reader_shortcuts(session: ReaderPageSession) -> Callback<KeyboardEvent> {
    let navigator = use_navigator();

    use_callback(move |event: KeyboardEvent| {
        // 带修饰键的组合交给浏览器/系统：否则 Ctrl+F（查找）会命中下面的 "f" 分支去切换收藏，
        // Ctrl+←/→ 之类的组合也会被当成翻页。
        if !event.modifiers().is_empty() {
            return;
        }

        let key = event.key().to_string().to_lowercase();

        match key.as_str() {
            "arrowleft" => {
                if let Some(target) = session.previous_entry_target() {
                    navigator.push(AppRoute::ReaderPage { entry_id: target });
                }
            }
            "arrowright" => {
                if let Some(target) = session.next_entry_target() {
                    navigator.push(AppRoute::ReaderPage { entry_id: target });
                }
            }
            "m" => session.toggle_read(true),
            "f" => session.toggle_starred(true),
            _ => {}
        }
    })
}

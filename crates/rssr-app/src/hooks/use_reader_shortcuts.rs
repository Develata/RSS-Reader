use std::{cell::Cell, rc::Rc};

use dioxus::prelude::*;

use super::reader_shortcut_policy::{self, ReaderShortcut, ShortcutKey};

use crate::{pages::reader_page::ReaderPageSession, router::AppRoute};

/// 阅读页快捷键：J 或 → 下一篇未读，K 或 ← 上一篇未读，M 切换已读，F 切换收藏。
///
/// 监听整个页面，而不是只在阅读区有焦点时生效：此前挂在阅读区上，页面刚打开、点过阅读区
/// 以外的地方、或焦点所在的按钮变成禁用后，按键都没有反应。哪些按键交给浏览器（输入框里、
/// 带修饰键、组字中、图片查看器打开时）由 reader_shortcuts.js 判断。
pub fn use_reader_shortcuts(session: ReaderPageSession) {
    let navigator = use_navigator();
    // 每次渲染都换成最新的 session：换文章时组件不重建，页面监听只建一次。
    let handle = use_callback(move |key: String| {
        match reader_shortcut_policy::resolve(false, false, || match key.as_str() {
            "ArrowLeft" => ShortcutKey::Previous,
            "ArrowRight" => ShortcutKey::Next,
            value => {
                let mut chars = value.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) => ShortcutKey::Character(c),
                    _ => ShortcutKey::Other,
                }
            }
        }) {
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
            _ => {}
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
            let mut eval = document::eval(include_str!("reader_shortcuts.js"));
            bridge.set(Some(eval));
            while let Ok(action) = eval.recv::<String>().await {
                handle.call(action);
            }
        }
    });
}

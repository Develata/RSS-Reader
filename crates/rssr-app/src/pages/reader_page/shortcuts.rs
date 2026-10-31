use dioxus::prelude::*;

use super::{ReaderPageSession, webview};

use crate::{
    router::AppRoute,
    ui::reader_shortcuts::{self, ReaderShortcut},
};

/// 阅读页快捷键：J 或 → 下一篇未读，K 或 ← 上一篇未读，M 切换已读，F 切换收藏。
///
/// 监听整个页面，而不是只在阅读区有焦点时生效：此前挂在阅读区上，页面刚打开、点过阅读区
/// 以外的地方、或焦点所在的按钮变成禁用后，按键都没有反应。哪些按键交给浏览器（输入框里、
/// 带修饰键、组字中、图片查看器打开时）由 Reader 的 WebView 适配器判断。
pub(super) fn use_reader_shortcuts(session: ReaderPageSession) {
    let navigator = use_navigator();
    // 每次渲染都换成最新的 session：换文章时组件不重建，页面监听只建一次。
    let handle = use_callback(move |key: String| {
        match reader_shortcuts::resolve(false, false, || key.as_str()) {
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
    webview::use_shortcut_keys(handle);
}

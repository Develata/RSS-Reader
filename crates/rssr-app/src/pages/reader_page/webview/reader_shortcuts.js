// 阅读页快捷键：监听整个页面，不要求焦点在阅读区里。只发原始按键，跳转与切换由 Rust 决定。
// 输入框与可编辑区域里的按键、带修饰键或输入法组字中的按键、按住不放的重复、以及图片查看器
// 打开时，都交给浏览器。与 ui/reading_position.js 共用 shortcut_input.js 的同步输入边界。
const controller = new AbortController();
document.addEventListener('keydown', event => {
    if (!acceptsReaderShortcut(event)) return;
    event.preventDefault();
    dioxus.send(String(event.key));
}, {signal: controller.signal});
await dioxus.recv();
controller.abort();

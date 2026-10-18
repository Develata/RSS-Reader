// 阅读页快捷键：监听整个页面，不要求焦点在阅读区里。只发动作名，跳转与切换由 Rust 决定。
// 输入框与可编辑区域里的按键、带修饰键或输入法组字中的按键、按住不放的重复、以及图片查看器
// 打开时，都交给浏览器。ui/reading_position.js 的 onKeyNavigation 用同样的边界在切文前记位置。
const actions = {arrowleft: 'previous', k: 'previous', arrowright: 'next', j: 'next', m: 'read', f: 'star'};
const controller = new AbortController();
document.addEventListener('keydown', event => {
    const action = actions[String(event.key).toLowerCase()];
    if (!action || event.defaultPrevented || event.repeat || event.isComposing
        || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
    const target = event.target;
    if (target instanceof Element && (target.isContentEditable
        || target.closest('input,textarea,select,[contenteditable]:not([contenteditable="false"])'))) return;
    if (document.querySelector('dialog[open]') || !document.querySelector('[data-page="reader"]')) return;
    event.preventDefault();
    dioxus.send(action);
}, {signal: controller.signal});
await dioxus.recv();
controller.abort();

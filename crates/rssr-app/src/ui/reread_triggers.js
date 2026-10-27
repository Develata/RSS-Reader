// 只读刷新的触发（C4b 第 2 条）：页面重新可见、窗口获得焦点、重新联网。只发事件名，合并与刷新由
// Rust 决定（AppShellState::request_reread）。`online` 不证明服务器已恢复：刷新失败照常显示。
const controller = new AbortController();
const options = {signal: controller.signal};
document.addEventListener('visibilitychange', () => {
    if (!document.hidden) dioxus.send('visible');
}, options);
window.addEventListener('focus', () => dioxus.send('focus'), options);
window.addEventListener('online', () => dioxus.send('online'), options);
await dioxus.recv();
controller.abort();

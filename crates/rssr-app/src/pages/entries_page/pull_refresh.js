// Passive, page-scoped DOM facts. Rust owns gesture policy and refresh state.
await new Promise(requestAnimationFrame);
const root = document.querySelector('[data-page="entries"][data-entry-scope="all"]');
if (!root) return;
const controller = new AbortController();
let lastTouchY = null;
const nestedScrollerCanConsume = (target, fingerDeltaY) => {
    if (Math.abs(fingerDeltaY) < 0.5) return false;
    for (let node = target; node instanceof Element && node !== root; node = node.parentElement) {
        const style = getComputedStyle(node);
        if (!/auto|scroll/.test(style.overflowY)) continue;
        const maxScrollTop = node.scrollHeight - node.clientHeight;
        if (maxScrollTop <= 1) continue;

        // Finger down asks the nested scroller to move toward its top; finger up asks
        // it to move toward its bottom. Only block pull-refresh while it can still
        // consume movement in that direction.
        if (fingerDeltaY > 0 && node.scrollTop > 1) return true;
        if (fingerDeltaY < 0 && node.scrollTop < maxScrollTop - 1) return true;
    }
    return false;
};
const emit = (kind, event) => {
    const touch = event.touches[0] ?? event.changedTouches[0];
    if (!touch) {
        if (kind === 'end' || kind === 'cancel') lastTouchY = null;
        return;
    }
    const fingerDeltaY = lastTouchY === null ? 0 : touch.clientY - lastTouchY;
    const nestedScroll = nestedScrollerCanConsume(event.target, fingerDeltaY);
    if (kind === 'start' || kind === 'move') {
        lastTouchY = touch.clientY;
    } else {
        lastTouchY = null;
    }
    dioxus.send({kind, x: touch.clientX, y: touch.clientY,
        at_top: (document.scrollingElement?.scrollTop ?? scrollY) <= 1,
        eligible: !nestedScroll && !event.target.closest('input, textarea, select, button, a, [contenteditable]')
            && !window.getSelection()?.toString(),
        touches: event.touches.length});
};
for (const [type, kind] of [['touchstart', 'start'], ['touchmove', 'move'], ['touchend', 'end'], ['touchcancel', 'cancel']]) {
    root.addEventListener(type, event => emit(kind, event), {passive: true, signal: controller.signal});
}
await dioxus.recv();
controller.abort();

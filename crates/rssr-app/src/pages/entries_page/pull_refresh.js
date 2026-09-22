// Passive, page-scoped DOM facts. Rust owns gesture policy and refresh state.
await new Promise(requestAnimationFrame);
const root = document.querySelector('[data-page="entries"][data-entry-scope="all"]');
if (!root) return;
const controller = new AbortController();
const emit = (kind, event) => {
    const touch = event.touches[0] ?? event.changedTouches[0];
    if (!touch) return;
    let nestedScroll = false;
    for (let node = event.target; node instanceof Element && node !== root; node = node.parentElement) {
        if (node.scrollHeight > node.clientHeight && /auto|scroll/.test(getComputedStyle(node).overflowY)) {
            nestedScroll = true;
        }
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

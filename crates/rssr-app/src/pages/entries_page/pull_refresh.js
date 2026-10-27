// Passive, page-scoped DOM facts. Rust owns gesture policy and refresh state.
await new Promise(requestAnimationFrame);
const root = document.querySelector('[data-page="entries"][data-entry-scope="all"]');
if (!root) return;
const controller = new AbortController();
const nestedScrollCapacity = (target) => {
    let above = false;
    let below = false;
    for (let node = target; node instanceof Element && node !== root; node = node.parentElement) {
        const style = getComputedStyle(node);
        if (!/auto|scroll/.test(style.overflowY)) continue;
        const maxScrollTop = node.scrollHeight - node.clientHeight;
        if (maxScrollTop <= 1) continue;

        // Summarize both directions in one traversal. Rust decides whether this
        // movement can be consumed; don't stop until both facts are known.
        above ||= node.scrollTop > 1;
        below ||= node.scrollTop < maxScrollTop - 1;
        if (above && below) break;
    }
    return {above, below};
};
const emit = (kind, event) => {
    const touch = event.touches[0] ?? event.changedTouches[0];
    const nested = nestedScrollCapacity(event.target);
    // A contactless end/cancel is still an event, so Rust can clear an armed pull.
    dioxus.send({kind, point: touch ? {x: touch.clientX, y: touch.clientY} : null,
        at_top: (document.scrollingElement?.scrollTop ?? scrollY) <= 1,
        on_control: !!event.target.closest('input, textarea, select, button, a, [contenteditable]'),
        has_selection: !!window.getSelection()?.toString(),
        nested_above: nested.above, nested_below: nested.below,
        touches: event.touches.length});
};
for (const [type, kind] of [['touchstart', 'start'], ['touchmove', 'move'], ['touchend', 'end'], ['touchcancel', 'cancel']]) {
    root.addEventListener(type, event => emit(kind, event), {passive: true, signal: controller.signal});
}
await dioxus.recv();
controller.abort();

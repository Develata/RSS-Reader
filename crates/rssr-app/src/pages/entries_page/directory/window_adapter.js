// Host measurements/input only. Rust chooses variable-height ranges and targets.
const root = document.getElementById(windowId);
const rail = root?.closest('[data-directory-consumer="rail"]');
if (!root || !rail) return;
const heading = rail.querySelector('.directory-heading-row');
function viewport() {
    const r = rail.getBoundingClientRect(), edge = r.top + rail.clientTop;
    const bottom = edge + rail.clientHeight;
    const top = Math.min(bottom, Math.max(edge, heading?.getBoundingClientRect().bottom || edge));
    return {top, height: bottom - top};
}
root.__rssrVirtualWindow?.cleanup();
const directoryEpoch = rail.dataset.directoryEpoch;
let disposed = false, frame = 0, sequence = 0, inFlight = null, reply = null, signature = '';
let keyboardReveal = null;
const pending = new Map(), abort = new AbortController();
const listen = (target, type, fn, options = {}) => target.addEventListener(type, fn, {...options, signal: abort.signal});
const number = value => Math.round(value * 4) / 4;
function current() {
    return !disposed && root.isConnected && rail.isConnected && rail.dataset.directoryEpoch === directoryEpoch &&
        root.__rssrVirtualWindow?.cleanup === cleanup;
}
function sendNext() {
    if (!current()) { cleanup(); return; }
    if (inFlight || !pending.size) return;
    const [key, request] = pending.entries().next().value;
    pending.delete(key); inFlight = request;
    dioxus.send({...request.fact, sequence: ++sequence});
}
function queue(key, fact, resolve) {
    if (!current()) { resolve?.(false); cleanup(); return; }
    pending.get(key)?.resolve?.(false);
    pending.set(key, {fact, resolve}); sendNext();
}
function measure() {
    frame = 0;
    if (!current()) { cleanup(); return; }
    const view = viewport(), b = root.getBoundingClientRect();
    if (!view.height || !b.width) return;
    const fact = {kind: 'measure', top: number(view.top - b.top), viewport: view.height,
        width: number(b.width), gap: parseFloat(getComputedStyle(root).rowGap) || 0,
        rows: [...root.querySelectorAll('[data-virtual-index]')].map(e => [Number(e.dataset.virtualIndex), number(e.getBoundingClientRect().height)]),
        focused: root.contains(document.activeElement) && document.activeElement.hasAttribute('data-virtual-index') ? Number(document.activeElement.dataset.virtualIndex) : null};
    const next = JSON.stringify(fact);
    if (next !== signature) { signature = next; queue('measure', fact); }
    else keyboardReveal = null;
}
function schedule() { if (!disposed && !frame) frame = requestAnimationFrame(measure); }
function keepKeyboardFocusVisible() {
    if (!keyboardReveal || document.activeElement !== keyboardReveal) return;
    const view = viewport(), row = keyboardReveal.getBoundingClientRect();
    if (row.top < view.top || row.height > view.height) rail.scrollTop += row.top - view.top;
    else if (row.bottom > view.top + view.height) rail.scrollTop += row.bottom - view.top - view.height;
}
function finish() {
    if (!current()) { cleanup(); return; }
    if (!reply || Number(root.dataset.windowRevision) < reply.revision) return;
    const command = reply;
    reply = null;
    if (command.top != null && command.found) {
        // A queued measurement describes the viewport before this seek. Applying
        // it afterwards would unmount the newly focused row and lose DOM focus.
        pending.delete('measure'); signature = '';
    }
    if (command.delta) rail.scrollTop += command.delta;
    if (command.top != null && command.found && command.focus != null) {
        // Only explicit keyboard navigation owns scrolling here. Automatic
        // reveals materialize a row; the directory controller alone aligns it
        // after checking newer user intent. A late reveal must not undo a wheel.
        const b = root.getBoundingClientRect();
        rail.scrollTop += command.top - (viewport().top - b.top);
    }
    if (command.focus != null) {
        const target = root.querySelector('[data-virtual-index="' + command.focus + '"]');
        target?.focus({preventScroll: true});
        keyboardReveal = target;
    }
    // Newly mounted rows can change height after an End/Home seek was clamped
    // at the rail edge. Keep that keyboard target visible until measurements
    // settle, but never counteract a newer pointer or wheel gesture.
    keepKeyboardFocusVisible();
    const request = inFlight; inFlight = null;
    request?.resolve?.(command.found);
    sendNext(); schedule();
    root.dispatchEvent(new Event('rssr:directory-window-commit', {bubbles: true}));
}
const observer = new MutationObserver(() => { finish(); schedule(); });
observer.observe(root, {subtree: true, childList: true, attributes: true, attributeFilter: ['data-window-revision']});
const resize = new ResizeObserver(schedule);
resize.observe(rail); resize.observe(root);
if (heading) resize.observe(heading);
listen(rail, 'scroll', schedule, {passive:true});
listen(root, 'focusin', schedule);
listen(root, 'focusout', schedule);
for (const type of ['wheel', 'pointerdown', 'touchstart']) {
    listen(window, type, () => { keyboardReveal = null; }, {passive:true, capture:true});
}
listen(root, 'keydown', event => {
    keyboardReveal = null;
    const target = event.target.closest?.('[data-virtual-index]');
    if (!target || !root.contains(target) || event.ctrlKey || event.altKey || event.metaKey || event.isComposing) return;
    let index;
    if (event.key === 'Tab') index = event.shiftKey ? target.dataset.virtualPrev : target.dataset.virtualNext;
    else if (event.key === 'ArrowDown') index = target.dataset.virtualNext;
    else if (event.key === 'ArrowUp') index = target.dataset.virtualPrev;
    else if (event.key === 'Home') index = '0';
    else if (event.key === 'End') index = String(Number(root.dataset.virtualCount) - 1);
    if (index == null) return; // Native Tab still enters/leaves the branch at its true ends.
    event.preventDefault();
    queue('focus', {kind:'reveal', index:Number(index), focus:true});
});
function cleanup() {
    if (disposed) return;
    disposed = true; abort.abort(); observer.disconnect(); resize.disconnect();
    if (frame) cancelAnimationFrame(frame);
    inFlight?.resolve?.(false); pending.forEach(request => request.resolve?.(false)); pending.clear();
    inFlight = null; reply = null;
    if (root.__rssrVirtualWindow?.cleanup === cleanup) delete root.__rssrVirtualWindow;
}
root.__rssrVirtualWindow = {
    cleanup,
    get idle() { return !frame && !inFlight && !reply && !pending.size; },
    reveal(anchor) { return new Promise(resolve => queue('reveal', {kind:'reveal', anchor, focus:false}, resolve)); },
};
root.dispatchEvent(new Event('rssr:directory-window-commit', {bubbles: true}));
schedule();
try {
    while (!disposed) {
        const command = await dioxus.recv();
        // recv can resume after a filter/page change retired this DOM window.
        // Check ownership again before applying any old geometry or focus.
        if (!current()) { cleanup(); break; }
        if (!command) { cleanup(); break; }
        if (command.rejected) {
            inFlight?.resolve?.(false); inFlight = null; reply = null; signature = ''; sendNext(); continue;
        }
        if (command.sequence !== sequence) continue;
        reply = command; finish();
    }
} catch (_) { cleanup(); }

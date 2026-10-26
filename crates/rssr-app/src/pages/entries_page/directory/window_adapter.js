// Host measurements/input only. Rust chooses variable-height ranges and targets.
const root = document.getElementById(windowId);
const rail = root?.closest('[data-directory-consumer="rail"]');
if (!root || !rail) return;
root.__rssrVirtualWindow?.cleanup();
let disposed = false, frame = 0, sequence = 0, inFlight = null, reply = null, signature = '';
const pending = new Map(), abort = new AbortController();
const listen = (target, type, fn, options = {}) => target.addEventListener(type, fn, {...options, signal: abort.signal});
const number = value => Math.round(value * 4) / 4;
function sendNext() {
    if (disposed || inFlight || !pending.size) return;
    const [key, request] = pending.entries().next().value;
    pending.delete(key); inFlight = request;
    dioxus.send({...request.fact, sequence: ++sequence});
}
function queue(key, fact, resolve) {
    pending.get(key)?.resolve?.(false);
    pending.set(key, {fact, resolve}); sendNext();
}
function measure() {
    frame = 0;
    if (disposed || !root.isConnected) return;
    const r = rail.getBoundingClientRect(), b = root.getBoundingClientRect();
    if (!r.width || !r.height || !b.width) return;
    const fact = {kind: 'measure', top: number(r.top + rail.clientTop - b.top), viewport: rail.clientHeight,
        width: number(b.width), gap: parseFloat(getComputedStyle(root).rowGap) || 0,
        rows: [...root.querySelectorAll('[data-virtual-index]')].map(e => [Number(e.dataset.virtualIndex), number(e.getBoundingClientRect().height)]),
        focused: root.contains(document.activeElement) && document.activeElement.hasAttribute('data-virtual-index') ? Number(document.activeElement.dataset.virtualIndex) : null};
    const next = JSON.stringify(fact);
    if (next !== signature) { signature = next; queue('measure', fact); }
}
function schedule() { if (!disposed && !frame) frame = requestAnimationFrame(measure); }
function finish() {
    if (!reply || Number(root.dataset.windowRevision) < reply.revision) return;
    const command = reply;
    reply = null;
    if (command.top != null && command.found) {
        // A queued measurement describes the viewport before this seek. Applying
        // it afterwards would unmount the newly focused row and lose DOM focus.
        pending.delete('measure'); signature = '';
    }
    if (command.delta) rail.scrollTop += command.delta;
    if (command.top != null && command.found) {
        const r = rail.getBoundingClientRect(), b = root.getBoundingClientRect();
        rail.scrollTop += command.top - (r.top + rail.clientTop - b.top);
    }
    if (command.focus != null) {
        const target = root.querySelector('[data-virtual-index="' + command.focus + '"]');
        target?.focus({preventScroll: true});
        target?.scrollIntoView({block: 'nearest', behavior: 'instant'});
    }
    const request = inFlight; inFlight = null;
    request?.resolve?.(command.found);
    sendNext(); schedule();
    root.dispatchEvent(new Event('rssr:directory-window-commit', {bubbles: true}));
}
const observer = new MutationObserver(() => { finish(); schedule(); });
observer.observe(root, {subtree: true, childList: true, attributes: true, attributeFilter: ['data-window-revision']});
const resize = new ResizeObserver(schedule);
resize.observe(rail); resize.observe(root);
listen(rail, 'scroll', schedule, {passive:true});
listen(root, 'focusin', schedule);
listen(root, 'focusout', schedule);
listen(root, 'keydown', event => {
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
        if (!command) { cleanup(); break; }
        if (command.rejected) {
            inFlight?.resolve?.(false); inFlight = null; reply = null; signature = ''; sendNext(); continue;
        }
        if (command.sequence !== sequence) continue;
        reply = command; finish();
    }
} catch (_) { cleanup(); }

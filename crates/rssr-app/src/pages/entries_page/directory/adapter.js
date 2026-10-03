// DOM/input adapter. Rust owns mode, active/open state and reset rules.
const trackerKey = '__rssrEntryDirectoryTracker';
window[trackerKey]?.cleanup();
const root = document.querySelector('[data-page="entries"]');
if (!root) return;
const directorySelector = '[data-directory-consumer]';
let disposed = false, epoch = 0, sequence = 0, inFlight = null;
let raf = 0, dirty = true, anchors = [], lastPath = '';
let input = {source: null, serial: 0, at: 0}, reportedInput = 0;
let requestFollow = false, reflow = false, ack = null, navigation = null;
// One in flight. Latest explicit intent per control + observation/input keys;
// pending size is bounded by this context's rendered controls, not scroll rate.
const pending = new Map();
const abort = new AbortController();
const listen = (target, type, handler, options = {}) =>
    target.addEventListener(type, handler, {...options, signal: abort.signal});
const visible = e => !!e?.getClientRects().length && !!(e.offsetWidth || e.offsetHeight);
const consumer = target => target instanceof Element ? target.closest(directorySelector) : null;
const presentedGroup = target => consumer(target)?.dataset.directoryCurrentGroup || null;

function sendNext() {
    if (disposed || inFlight || !pending.size || !epoch) return;
    const [key, fact] = pending.entries().next().value;
    pending.delete(key);
    inFlight = fact;
    dioxus.send(fact);
}
function queue(key, kind, fields = {}) {
    if (disposed || !epoch) return;
    pending.delete(key);
    pending.set(key, {epoch, sequence: ++sequence, kind, ...fields});
    sendNext();
}
function selectAnchor() {
    if (dirty) {
        anchors = [...root.querySelectorAll('[data-entry-scroll-anchor]')];
        dirty = false;
    }
    let candidate = null;
    for (const anchor of anchors) {
        const rect = anchor.getBoundingClientRect();
        if (!rect.width && !rect.height) continue;
        if (rect.top <= 96) { candidate = anchor; continue; }
        if (!candidate && rect.bottom >= 0) candidate = anchor;
        break;
    }
    candidate ||= anchors.at(-1);
    return {group: candidate?.dataset.entryScrollGroupAnchor || null, item: candidate?.dataset.entryScrollAnchor || null};
}
function update() {
    raf = 0;
    if (disposed || !root.isConnected) { cleanup(); return; }
    refreshContext();
    const path = selectAnchor(), signature = JSON.stringify(path);
    if (requestFollow || signature !== lastPath) {
        queue(requestFollow ? 'follow' : 'observe', requestFollow ? 'follow' : 'observe', {...path, reflow});
        lastPath = signature;
    }
    requestFollow = false; reflow = false;
    finishCommand();
}
function scheduleUpdate(force = false) {
    if (disposed) return;
    if (force) { dirty = true; lastPath = ''; reflow = true; }
    if (!raf) raf = requestAnimationFrame(update);
}
function refreshContext() {
    const next = Number(root.querySelector('[data-directory-controller]')?.dataset.directoryEpoch || 0);
    if (!next || next === epoch) return;
    epoch = next;
    pending.clear();
    if (ack) inFlight = null;
    ack = null; lastPath = ''; dirty = true;
    // An in-flight prior epoch still receives its ack; its result is discarded.
    input = {source: null, serial: input.serial + 1, at: 0};
    requestFollow = false;
    scheduleUpdate(true);
}
function align(command) {
    const rail = root.querySelector('[data-directory-consumer="rail"]');
    if (visible(rail)) {
        const links = [...rail.querySelectorAll('[data-directory-anchor]')];
        const target = links.find(e => e.dataset.directoryAnchor === command.item && visible(e)) ||
            links.find(e => e.dataset.directoryAnchor === command.group && visible(e));
        if (target) {
            const r = rail.getBoundingClientRect(), t = target.getBoundingClientRect();
            const top = r.top + rail.clientTop, bottom = top + rail.clientHeight;
            if (t.top < top) rail.scrollTop += t.top - top;
            else if (t.bottom > bottom) rail.scrollTop += t.bottom - bottom;
        }
    } else {
        const top = root.querySelector('[data-directory-consumer="top"]');
        const target = top && [...top.querySelectorAll('[data-directory-anchor]')].find(e => e.dataset.directoryAnchor === command.group);
        if (visible(top) && target) {
            const r = top.getBoundingClientRect(), t = target.getBoundingClientRect();
            const left = r.left + top.clientLeft, right = left + top.clientWidth;
            if (t.left < left) top.scrollLeft += t.left - left;
            else if (t.right > right) top.scrollLeft += t.right - right;
        }
    }
}
function finishCommand() {
    if (disposed) return;
    refreshContext();
    if (navigation && navigation.contextKey !== root.dataset.positionKey) navigation = null;
    if (navigation && Number(root.dataset.positionPage) === navigation.page && root.dataset.positionReady === 'true') {
        const element = document.getElementById(navigation.anchor);
        if (element && root.contains(element)) {
            input = {source: null, serial: input.serial + 1, at: 0};
            if (location.hash !== '#' + encodeURIComponent(navigation.anchor)) location.hash = navigation.anchor;
            element.scrollIntoView({behavior: 'smooth', block: 'start', inline: 'nearest'});
            navigation = null;
        }
    }
    if (!ack) return;
    if (ack.epoch !== epoch) { ack = null; inFlight = null; sendNext(); return; }
    // MutationObserver runs after DOM mutations. Each mounted consumer must
    // acknowledge the version; a frame callback alone is NOT a commit fence.
    const consumers = [...root.querySelectorAll(directorySelector)];
    if (!consumers.every(e => Number(e.dataset.directoryEpoch) === ack.epoch && Number(e.dataset.directoryRevision) >= ack.revision)) return;
    if (ack.align && !pending.size) align(ack);
    ack = null; inFlight = null;
    sendNext();
}
function noteInput(event) {
    if (!event.isTrusted) return;
    const target = event.target;
    if (event.type === 'keydown') {
        if (!['ArrowUp','ArrowDown','PageUp','PageDown','Home','End',' '].includes(event.key) || event.ctrlKey || event.altKey || event.metaKey) return;
        if (target instanceof Element && (target.closest('input,textarea,select,[contenteditable="true"]') ||
            (event.key === ' ' && target.closest('button,a')))) return;
    }
    const inside = consumer(target);
    if (!inside && target instanceof Element && !root.contains(target) && target !== document.documentElement && target !== document.body) return;
    if (!inside && event.type === 'pointerdown') {
        // A click in content isn't a scroll gesture. Only arm pointer input
        // over the viewport scrollbar or a scroll container around the list.
        const groups = root.querySelector('[data-layout="entry-groups"]');
        const viewportBar = event.clientX >= document.documentElement.clientWidth;
        let containerBar = false;
        if (target instanceof Element && groups && target.contains(groups) && target.scrollHeight > target.clientHeight) {
            const rect = target.getBoundingClientRect();
            containerBar = event.clientX >= rect.left + target.clientLeft + target.clientWidth;
        }
        if (!viewportBar && !containerBar) return;
    }
    input = {source: inside ? 'directory' : 'main', serial: input.serial + 1, at: performance.now()};
    if (inside) {
        requestFollow = false;
        queue('manual', 'manual', {rendered_group: presentedGroup(target), epoch: Number(inside.dataset.directoryEpoch)});
    }
}
function onScroll(event) {
    const target = event.target;
    if (consumer(target)) return; // Includes our own rail/top alignment.
    const groups = root.querySelector('[data-layout="entry-groups"]');
    const isMain = target === document || target === window ||
        (target instanceof Element && groups && target.contains(groups));
    if (!isMain) {
        // A nested control consumed this gesture. Its late scroll/layout events
        // must not be attributed to a main-list gesture afterward.
        if (input.source === 'main') input.source = 'nested';
        return;
    }
    if (input.source === 'main' && performance.now() - input.at < 1500) {
        input.at = performance.now(); // Retain attribution through inertia.
        const manualIntent = fact => fact && ['manual','set_open','navigate'].includes(fact.kind);
        const needsResume = root.querySelector('[data-directory-mode="manual"]') ||
            manualIntent(inFlight) || [...pending.values()].some(manualIntent);
        if (reportedInput !== input.serial && needsResume) requestFollow = true;
        reportedInput = input.serial;
    }
    scheduleUpdate();
}
function onClick(event) {
    const button = event.target instanceof Element ? event.target.closest('[data-directory-anchor]') : null;
    if (!button || !consumer(button) || !root.contains(button)) return;
    // Explicit desired state from the presented button, never a Rust toggle
    // against a possibly newer observation. Click covers mouse/Enter/Space.
    const anchor = button.dataset.directoryAnchor;
    const rendered_group = presentedGroup(button);
    const renderedEpoch = Number(consumer(button).dataset.directoryEpoch);
    input = {source: 'directory', serial: input.serial + 1, at: performance.now()};
    requestFollow = false;
    if (button.dataset.layout === 'entry-directory-toggle') {
        queue('toggle:' + anchor, 'set_open', {anchor, open: button.dataset.open !== 'true', rendered_group, epoch: renderedEpoch});
    } else {
        queue('navigate', 'navigate', {anchor, page: Number(button.dataset.directoryPage), rendered_group,
            epoch: renderedEpoch, context_key: root.dataset.positionKey});
    }
}
function cleanup() {
    if (disposed) return;
    disposed = true;
    abort.abort(); observer.disconnect(); pending.clear(); navigation = null; ack = null;
    if (raf) cancelAnimationFrame(raf);
    if (window[trackerKey]?.cleanup === cleanup) delete window[trackerKey];
}
// A non-mutating bridge/DOM fence for host diagnostics and acceptance tools.
// It drains work already observed here; it does not promise future I/O or smooth
// scrolling has finished. Those callers must wait for their own completion facts.
function whenSettled() {
    return new Promise((resolve, reject) => {
        let stable = 0, frame = 0;
        const timer = setTimeout(() => { cancelAnimationFrame(frame); reject(new Error('Directory bridge did not settle')); }, 6000);
        const inspect = () => {
            if (disposed || !root.isConnected) { clearTimeout(timer); reject(new Error('Directory disposed')); return; }
            refreshContext();
            const consumers = [...root.querySelectorAll(directorySelector)];
            const idle = epoch && !raf && !inFlight && !ack && !pending.size && !navigation &&
                consumers.every(e => Number(e.dataset.directoryEpoch) === epoch);
            stable = idle ? stable + 1 : 0;
            if (stable >= 2) { clearTimeout(timer); resolve({epoch, sequence}); }
            else frame = requestAnimationFrame(inspect);
        };
        scheduleUpdate();
        frame = requestAnimationFrame(inspect);
    });
}
const observer = new MutationObserver(records => {
    if (!root.isConnected) { cleanup(); return; }
    refreshContext();
    if (records.some(m => m.type === 'childList' && !(m.target instanceof Element && consumer(m.target)))) {
        dirty = true; scheduleUpdate();
    }
    finishCommand();
});
observer.observe(root, {subtree: true, childList: true, attributes: true,
    attributeFilter: ['data-directory-epoch','data-directory-revision','data-position-page','data-position-ready']});
listen(window, 'scroll', onScroll, {capture: true, passive: true});
listen(window, 'resize', () => scheduleUpdate(true), {passive: true});
for (const name of ['wheel','touchstart','touchmove','pointerdown','keydown']) listen(document, name, noteInput, {capture: true, passive: true});
listen(root, 'click', onClick, {capture: true});
window[trackerKey] = {cleanup, scheduleUpdate, whenSettled};
scheduleUpdate(true);
try {
    while (!disposed) {
        const command = await dioxus.recv();
        if (!command) { cleanup(); break; }
        if (disposed) break;
        if (command.sequence !== inFlight?.sequence) continue;
        if (command.navigate) navigation = {anchor: command.navigate, page: command.page, contextKey: inFlight.context_key};
        ack = command;
        finishCommand();
    }
} finally { cleanup(); }

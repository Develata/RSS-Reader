// Delegation over sanitized HTML; no HTML rewriting or business/viewer state.
await new Promise(requestAnimationFrame);
const root = document.querySelector('[data-page="reader"]');
if (!root) return;
const controller = new AbortController();
const prepare = () => root.querySelectorAll('[data-slot="reader-body-html"] img').forEach(img => {
    img.tabIndex = 0;
    img.setAttribute('role', 'button');
    img.setAttribute('data-action', 'open-reader-image');
    img.setAttribute('aria-label', img.alt ? `放大图片：${img.alt}` : '放大图片');
});
const observer = new MutationObserver(prepare);
observer.observe(root, {childList: true, subtree: true});
prepare();
const open = event => {
    const img = event.target.closest?.('[data-slot="reader-body-html"] img');
    if (!img || event.ctrlKey || event.metaKey || event.altKey || event.shiftKey) return;
    if (event.type === 'keydown' && event.key !== 'Enter' && event.key !== ' ') return;
    if (event.type === 'click' && event.button !== 0) return;
    if (window.getSelection()?.toString()) return;
    const src = img.currentSrc || img.src;
    if (!src) return;
    // Only image activation consumes its native link/space action. Text selection is untouched.
    event.preventDefault();
    img.focus({preventScroll: true});
    dioxus.send({src, alt: img.alt || ''});
};
root.addEventListener('click', open, {signal: controller.signal});
root.addEventListener('keydown', open, {signal: controller.signal});
await dioxus.recv();
controller.abort();
observer.disconnect();

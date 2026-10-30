// Native modal focus/inert behavior and scroll restoration are host DOM capabilities.
const dialog = document.querySelector('[data-layout="reader-image-viewer"]');
if (!dialog) return;
const focus = document.activeElement;
const x = scrollX, y = scrollY;
const body = document.body;
const previous = {position: body.style.position, top: body.style.top, left: body.style.left, width: body.style.width};
Object.assign(body.style, {position: 'fixed', top: `${-y}px`, left: `${-x}px`, width: '100%'});
const controller = new AbortController();
dialog.addEventListener('cancel', event => {
    event.preventDefault();
    dioxus.send(null);
}, {signal: controller.signal});
dialog.addEventListener('click', event => {
    if (event.target === dialog || event.target.matches('[data-layout="reader-image-viewport"]')) dioxus.send(null);
}, {signal: controller.signal});
try {
    dialog.showModal();
    dialog.querySelector('[data-action="close-reader-image"]')?.focus({preventScroll: true});
    await dioxus.recv();
} finally {
    controller.abort();
    dialog.close();
    Object.assign(body.style, previous);
    window.scrollTo({left: x, top: y, behavior: 'instant'});
    if (focus?.isConnected) focus.focus({preventScroll: true});
}

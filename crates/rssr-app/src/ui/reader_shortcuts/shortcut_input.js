// readerShortcutBindings is projected from the Rust definition in this Eval.
// Capture runs before target/bubble handlers and deliberately ignores
// defaultPrevented. Dispatch retains the bubble-phase defaultPrevented gate.
const acceptsReaderShortcut = (event, capturePosition = false) => {
    const navigation = readerShortcutBindings[String(event.key).toLowerCase()];
    if ((capturePosition ? navigation !== true : navigation === undefined || event.defaultPrevented)
        || event.repeat || event.isComposing || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return false;
    const target = event.target;
    if (target instanceof Element && (target.isContentEditable
        || target.closest('input,textarea,select,[contenteditable]:not([contenteditable="false"])'))) return false;
    return !document.querySelector('dialog[open]') && !!document.querySelector('[data-page="reader"]');
};

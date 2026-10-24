use dioxus::prelude::*;

/// Native dialog owns focus trapping and modality; Rust owns the draft and actions.
#[component]
pub(crate) fn Dialog(
    id: &'static str,
    label: &'static str,
    #[props(default)] anchor: Option<&'static str>,
    on_close: EventHandler,
    children: Element,
) -> Element {
    rsx! {
        dialog {
            id, class: "workspace-dialog", "data-anchored": anchor.is_some(), aria_label: label,
            onmounted: move |_| {
                let script = format!(r#"(() => {{
                    const dialog = document.getElementById({id:?});
                    if (!dialog || dialog.open) return;
                    const previous = document.activeElement;
                    dialog.showModal();
                    const anchor = document.getElementById({anchor:?});
                    const position = () => {{ if (anchor) {{
                        const rect = anchor.getBoundingClientRect();
                        dialog.style.margin = '0';
                        dialog.style.left = Math.max(12, Math.min(rect.left, innerWidth - dialog.offsetWidth - 12)) + 'px';
                        dialog.style.top = Math.max(12, Math.min(rect.bottom + 8, innerHeight - dialog.offsetHeight - 12)) + 'px';
                    }} }};
                    position();
                    window.addEventListener('resize', position);
                    const cleanup = () => {{
                        window.removeEventListener('resize', position);
                        observer.disconnect();
                        if (previous?.isConnected) previous.focus({{preventScroll:true}});
                    }};
                    const observer = new MutationObserver(() => {{
                        if (!dialog.isConnected) cleanup();
                    }});
                    observer.observe(document.body, {{childList:true, subtree:true}});
                    dialog.addEventListener('close', cleanup, {{once:true}});
                }})();"#, anchor = anchor.unwrap_or(""));
                let _ = document::eval(&script);
            },
            onkeydown: move |event| {
                // Block the native dialog's default dismissal as well as the
                // Rust close action while the input method is composing text.
                if event.key() == Key::Escape {
                    event.prevent_default();
                    if !event.is_composing() {
                        on_close.call(());
                    }
                }
            },
            onclick: move |_| on_close.call(()),
            div { class: "workspace-dialog-body", onclick: move |event| event.stop_propagation(), {children} }
        }
    }
}

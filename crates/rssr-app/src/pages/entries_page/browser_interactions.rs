pub(super) fn scroll_to_page_start() {
    dioxus::prelude::document::eval(
        "requestAnimationFrame(() => window.scrollTo({top: 0, behavior: 'instant'}))",
    );
}

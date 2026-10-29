pub(super) fn scroll_to_page_start() {
    dioxus::prelude::document::eval(
        "requestAnimationFrame(() => window.scrollTo({top: 0, behavior: 'instant'}))",
    );
}

pub(super) fn focus_filter_control() {
    // The empty state's clear button disappears after the filters are reset.
    dioxus::prelude::document::eval(
        "(document.querySelector('[data-field=entry-search]') || document.querySelector('.entries-view-options > summary'))?.focus({preventScroll:true})",
    );
}

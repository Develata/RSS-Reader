use dioxus::prelude::*;

#[cfg(target_arch = "wasm32")]
fn initial_entry_controls_hidden_impl() -> Option<bool> {
    if let Some(window) = web_sys::window()
        && let Ok(Some(storage)) = window.local_storage()
        && let Ok(Some(value)) = storage.get_item("rssr-entry-controls-hidden")
    {
        return Some(value == "1");
    }

    None
}

pub(super) fn initial_entry_controls_hidden() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        return initial_entry_controls_hidden_impl().unwrap_or(true);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        true
    }
}

pub(super) fn remember_entry_controls_hidden(hidden: bool) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window()
            && let Ok(Some(storage)) = window.local_storage()
        {
            let _ = storage.set_item("rssr-entry-controls-hidden", if hidden { "1" } else { "0" });
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    let _ = hidden;
}

pub(super) fn scroll_to_entry_group(anchor_id: &str) {
    let Ok(anchor_id_json) = serde_json::to_string(anchor_id) else {
        return;
    };

    document::eval(&format!(
        r#"
        const targetId = {anchor_id_json};
        const scrollToTarget = () => {{
            const element = document.getElementById(targetId);
            if (!element) {{
                return false;
            }}

            if (window.location.hash !== `#${{targetId}}`) {{
                window.location.hash = targetId;
            }}

            element.scrollIntoView({{ behavior: "smooth", block: "start", inline: "nearest" }});
            return true;
        }};

        if (!scrollToTarget()) {{
            requestAnimationFrame(scrollToTarget);
        }} else {{
            requestAnimationFrame(scrollToTarget);
        }}
        "#
    ));
}

pub(super) fn scroll_directory_item(anchor_id: &str) {
    let Ok(anchor_id_json) = serde_json::to_string(anchor_id) else {
        return;
    };

    document::eval(&format!(
        r#"
        const targetId = {anchor_id_json};
        const selector = `[data-directory-anchor="${{targetId}}"]`;
        const scrollActiveDirectory = () => {{
            const elements = document.querySelectorAll(selector);
            if (!elements.length) {{
                return false;
            }}

            elements.forEach((element) => {{
                element.scrollIntoView({{
                    behavior: "smooth",
                    block: "nearest",
                    inline: "nearest"
                }});
            }});
            return true;
        }};

        if (!scrollActiveDirectory()) {{
            requestAnimationFrame(scrollActiveDirectory);
        }}
        "#
    ));
}

fn sync_directory_with_entry_scroll_impl(scroll_target: bool) {
    let scroll_target = if scroll_target { "true" } else { "false" };
    let script = r#"
        const trackerKey = "__rssrEntryDirectoryTracker";
        const existingTracker = window[trackerKey];
        if (existingTracker?.scheduleUpdate) {
            existingTracker.scheduleUpdate(true, __SCROLL_TARGET__);
            return;
        }

        const isVisible = (element) =>
            !!element && !!(element.offsetWidth || element.offsetHeight || element.getClientRects().length);

        const setDataState = (element, key, value) => {
            if (element.dataset[key] !== value) {
                element.dataset[key] = value;
            }
        };

        const setAttributeState = (element, key, value) => {
            if (element.getAttribute(key) !== value) {
                element.setAttribute(key, value);
            }
        };

        const syncGroupState = (groupAnchor) => {
            document.querySelectorAll('[data-directory-kind="group"]').forEach((element) => {
                const isActive = !!groupAnchor && element.dataset.directoryAnchor === groupAnchor;
                setDataState(element, "active", isActive ? "true" : "false");
            });

            document.querySelectorAll('[data-layout="entry-directory-toggle"]').forEach((element) => {
                const isActive = !!groupAnchor && element.dataset.directoryAnchor === groupAnchor;
                const baseOpen = element.dataset.openBase === "true";
                const nextOpen = isActive || baseOpen;
                const canToggle = isActive ? "false" : "true";
                const open = nextOpen ? "true" : "false";
                setDataState(element, "canToggle", canToggle);
                setDataState(element, "open", open);
                setAttributeState(element, "aria-disabled", isActive ? "true" : "false");
                setAttributeState(element, "aria-expanded", open);

                const section = element.parentElement?.querySelector('[data-directory-section-body="true"]');
                if (section) {
                    setDataState(section, "open", open);
                }
            });
        };

        const setActiveState = (selector, anchorId) => {
            document.querySelectorAll(selector).forEach((element) => {
                const isActive =
                    !!anchorId && element.dataset.directoryAnchor === anchorId ? "true" : "false";
                setDataState(element, "active", isActive);
            });
        };

        const findScrollTarget = (groupAnchor, itemAnchor) => {
            const items = Array.from(document.querySelectorAll('[data-directory-kind="item"]'));
            const groups = Array.from(document.querySelectorAll('[data-directory-kind="group"]'));
            return (
                items.find(
                    (element) =>
                        isVisible(element) && itemAnchor && element.dataset.directoryAnchor === itemAnchor
                ) ||
                groups.find(
                    (element) =>
                        isVisible(element) && groupAnchor && element.dataset.directoryAnchor === groupAnchor
                ) ||
                null
            );
        };

        const selectActiveAnchor = () => {
            const anchors = Array.from(document.querySelectorAll("[data-entry-scroll-anchor]")).filter(
                isVisible
            );
            if (!anchors.length) {
                return { groupAnchor: null, itemAnchor: null };
            }

            const threshold = 96;
            let candidate = null;

            for (const anchor of anchors) {
                const rect = anchor.getBoundingClientRect();
                if (rect.top <= threshold) {
                    candidate = anchor;
                    continue;
                }

                if (!candidate && rect.bottom >= 0) {
                    candidate = anchor;
                }
                break;
            }

            candidate ||= anchors[anchors.length - 1];
            return {
                groupAnchor: candidate?.dataset.entryScrollGroupAnchor || null,
                itemAnchor: candidate?.dataset.entryScrollAnchor || null,
            };
        };

        let rafId = 0;
        let forceSync = false;
        let shouldScrollTarget = true;
        let lastGroupAnchor = null;
        let lastItemAnchor = null;

        const cleanup = () => {
            if (rafId) {
                cancelAnimationFrame(rafId);
                rafId = 0;
            }
            window.removeEventListener("scroll", onScroll);
            window.removeEventListener("resize", onResize);
            delete window[trackerKey];
        };

        const update = () => {
            rafId = 0;
            const shouldForceSync = forceSync;
            const shouldScroll = shouldScrollTarget;
            forceSync = false;
            shouldScrollTarget = false;

            if (!document.querySelector('[data-page="entries"]')) {
                cleanup();
                return;
            }

            const { groupAnchor, itemAnchor } = selectActiveAnchor();
            const groupChanged = shouldForceSync || groupAnchor !== lastGroupAnchor;
            const itemChanged = shouldForceSync || itemAnchor !== lastItemAnchor;

            if (groupChanged) {
                syncGroupState(groupAnchor);
            }
            if (itemChanged) {
                setActiveState('[data-directory-kind="item"]', itemAnchor);
            }

            if (!groupChanged && !itemChanged) {
                return;
            }

            lastGroupAnchor = groupAnchor;
            lastItemAnchor = itemAnchor;

            if (shouldScroll) {
                const target = findScrollTarget(groupAnchor, itemAnchor);
                if (target) {
                    target.scrollIntoView({
                        block: "nearest",
                        inline: "nearest",
                    });
                }
            }
        };

        const scheduleUpdate = (shouldForce = false, shouldScroll = false) => {
            forceSync = forceSync || shouldForce;
            shouldScrollTarget = shouldScrollTarget || shouldScroll;
            if (rafId) {
                return;
            }
            rafId = requestAnimationFrame(update);
        };

        const onScroll = () => {
            scheduleUpdate(false, true);
        };

        const onResize = () => {
            scheduleUpdate(true);
        };

        window.addEventListener("scroll", onScroll, { passive: true });
        window.addEventListener("resize", onResize, { passive: true });

        window[trackerKey] = {
            cleanup,
            scheduleUpdate,
        };

        scheduleUpdate(true, __SCROLL_TARGET__);
        "#;
    document::eval(&script.replace("__SCROLL_TARGET__", scroll_target));
}

pub(super) fn sync_directory_with_entry_scroll() {
    sync_directory_with_entry_scroll_impl(true);
}

pub(super) fn refresh_directory_with_entry_scroll_state() {
    sync_directory_with_entry_scroll_impl(false);
}

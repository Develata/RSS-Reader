pub(super) fn scroll_to_page_start() {
    dioxus::prelude::document::eval(
        "requestAnimationFrame(() => window.scrollTo({top: 0, behavior: 'instant'}))",
    );
}

use dioxus::prelude::*;

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
        const isVisible = (element) =>
            !!element && !!(element.offsetWidth || element.offsetHeight || element.getClientRects().length);

        const alignVertical = (container, target) => {{
            const containerRect = container.getBoundingClientRect();
            const targetRect = target.getBoundingClientRect();
            const visibleTop = containerRect.top + container.clientTop;
            const visibleBottom = visibleTop + container.clientHeight;
            if (targetRect.top < visibleTop) {{
                container.scrollTop += targetRect.top - visibleTop;
            }} else if (targetRect.bottom > visibleBottom) {{
                container.scrollTop += targetRect.bottom - visibleBottom;
            }}
        }};

        const alignHorizontal = (container, target) => {{
            const containerRect = container.getBoundingClientRect();
            const targetRect = target.getBoundingClientRect();
            const visibleLeft = containerRect.left + container.clientLeft;
            const visibleRight = visibleLeft + container.clientWidth;
            if (targetRect.left < visibleLeft) {{
                container.scrollLeft += targetRect.left - visibleLeft;
            }} else if (targetRect.right > visibleRight) {{
                container.scrollLeft += targetRect.right - visibleRight;
            }}
        }};

        const scrollActiveDirectory = () => {{
            let found = false;
            const rail = document.querySelector('[data-layout="entry-directory-rail"]');
            const topDirectory = document.querySelector('[data-layout="entry-top-directory"]');

            if (isVisible(rail)) {{
                const target = Array.from(rail.querySelectorAll(selector)).find(isVisible);
                if (target) {{
                    alignVertical(rail, target);
                    found = true;
                }}
            }}

            if (isVisible(topDirectory)) {{
                const target = Array.from(topDirectory.querySelectorAll(selector)).find(isVisible);
                if (target) {{
                    alignHorizontal(topDirectory, target);
                    found = true;
                }}
            }}

            return found;
        }};

        if (!scrollActiveDirectory()) {{
            requestAnimationFrame(scrollActiveDirectory);
        }}
        "#
    ));
}

// Keep the tracker singleton alive across re-renders. Callers must choose whether this refresh
// should also align the directory viewport, or only reconcile active/open state in place.
fn refresh_entry_directory_tracker(align_directory_viewport: bool) {
    let align_directory_viewport = if align_directory_viewport { "true" } else { "false" };
    let script = r#"
        const trackerKey = "__rssrEntryDirectoryTracker";
        const existingTracker = window[trackerKey];
        if (existingTracker?.scheduleUpdate) {
            existingTracker.scheduleUpdate(true, __ALIGN_DIRECTORY_VIEWPORT__);
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

        const findDirectoryViewportTarget = (rail, groupAnchor, itemAnchor) => {
            const items = Array.from(rail.querySelectorAll('[data-directory-kind="item"]'));
            const groups = Array.from(rail.querySelectorAll('[data-directory-kind="group"]'));
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

        const alignDirectoryViewport = (groupAnchor, itemAnchor) => {
            const rail = document.querySelector('[data-layout="entry-directory-rail"]');
            if (!isVisible(rail)) {
                // Mobile renders the horizontal top directory instead of an independent
                // vertical rail. Never call scrollIntoView on that surface: doing so can
                // scroll the document itself and fight the user's vertical gesture.
                return;
            }

            const target = findDirectoryViewportTarget(rail, groupAnchor, itemAnchor);
            if (!target) {
                return;
            }

            const railRect = rail.getBoundingClientRect();
            const targetRect = target.getBoundingClientRect();
            const visibleTop = railRect.top + rail.clientTop;
            const visibleBottom = visibleTop + rail.clientHeight;

            // Move only the directory rail. scrollIntoView is intentionally avoided here
            // because an ancestor fallback may change window.scrollY.
            if (targetRect.top < visibleTop) {
                rail.scrollTop += targetRect.top - visibleTop;
            } else if (targetRect.bottom > visibleBottom) {
                rail.scrollTop += targetRect.bottom - visibleBottom;
            }
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
        // This flag only controls whether the directory rail should be scrolled into view.
        // Active/open state should still be refreshed even when viewport alignment is disabled.
        let shouldAlignDirectoryViewport = true;
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
            const shouldAlignViewport = shouldAlignDirectoryViewport;
            forceSync = false;
            shouldAlignDirectoryViewport = false;

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

            if (shouldAlignViewport) {
                alignDirectoryViewport(groupAnchor, itemAnchor);
            }
        };

        // `shouldAlignViewport` must only be true for entry-pane driven navigation/scrolling.
        // Directory-local toggles should call into the state-only path and leave the current
        // directory scroll position untouched.
        const scheduleUpdate = (shouldForce = false, shouldAlignViewport = false) => {
            forceSync = forceSync || shouldForce;
            shouldAlignDirectoryViewport = shouldAlignDirectoryViewport || shouldAlignViewport;
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

        scheduleUpdate(true, __ALIGN_DIRECTORY_VIEWPORT__);
        "#;
    document::eval(&script.replace("__ALIGN_DIRECTORY_VIEWPORT__", align_directory_viewport));
}

// Use this when the left article pane drove the change: initial render, page jump, page change,
// or article scrolling. Besides refreshing active/open state, it keeps the active directory item
// visible inside the directory rail.
pub(super) fn sync_entry_directory_with_viewport_alignment() {
    refresh_entry_directory_tracker(true);
}

// Use this for directory-local interactions such as expand/collapse. It must not move the
// directory viewport; otherwise a simple toggle would snap the rail back to the current article.
pub(super) fn sync_entry_directory_state_in_place() {
    refresh_entry_directory_tracker(false);
}

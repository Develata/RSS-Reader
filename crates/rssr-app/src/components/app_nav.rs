use dioxus::prelude::*;

use crate::{router::AppRoute, ui::use_app_nav_shell};

#[component]
pub fn AppNav(on_back: Option<EventHandler<()>>) -> Element {
    let shell = use_app_nav_shell();
    let home_shell = shell.clone();
    let mut toggle_shell = shell.clone();
    let mut collapse_shell = shell.clone();
    let mut escape_shell = shell.clone();
    let submit_shell = shell.clone();
    let update_shell = shell.clone();
    let refresh = shell.refresh_state();
    let home_title = if refresh.label().is_empty() {
        "Read / 首页；在首页再次点击刷新全部订阅".to_string()
    } else {
        format!("Read / 首页；{}；在首页再次点击刷新全部订阅", refresh.label())
    };
    let navigator = use_navigator();

    rsx! {
        nav { "data-layout": "app-nav-shell", "data-state": shell.nav_state(), aria_label: "主导航",
            div { "data-layout": "app-nav-topline", id: "app-nav-content",
              if !shell.is_collapsed() {
                if shell.is_reader() {
                    button {
                        class: "icon-link-button", "data-nav": "back", r#type: "button",
                        aria_label: "返回", title: "返回",
                        onclick: move |_| {
                            if let Some(on_back) = on_back { on_back.call(()); }
                            else { crate::hooks::use_mobile_back_navigation::navigate_back(navigator, Some(AppRoute::EntriesPage {})); }
                        },
                        span { aria_hidden: "true", "←" }
                    }
                }
                button {
                    class: "icon-link-button", "data-slot": "app-nav-brand",
                    "data-action": "activate-home", "data-refresh-state": refresh.phase(),
                    r#type: "button", aria_label: "Read / 首页", title: home_title,
                    aria_busy: refresh.is_refreshing(),
                    onclick: move |_| home_shell.activate_home(),
                    span { "data-slot": "app-nav-brand-mark", aria_hidden: "true", "R" }
                }
                button {
                    class: "icon-link-button", "data-action": "toggle-search", r#type: "button",
                    aria_label: "搜索", title: "展开 / 收起搜索", aria_expanded: shell.is_search(),
                    aria_controls: "app-nav-search-input",
                    onclick: move |_| toggle_shell.toggle_search(),
                    svg { width: "20", height: "20", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", "aria-hidden": "true",
                        circle { cx: "10", cy: "10", r: "6" }
                        path { d: "m15 15 6 6" }
                    }
                }
                if shell.is_search() {
                    form {
                        "data-layout": "app-nav-search",
                        onsubmit: move |event| { event.prevent_default(); submit_shell.submit_search(); },
                        input {
                            id: "app-nav-search-input", "data-slot": "app-nav-search-input", "data-field": "entry-search",
                            r#type: "search", placeholder: "搜索文章标题", aria_label: "搜索文章标题",
                            value: shell.entry_search(),
                            onmounted: move |event| async move { let _ = event.set_focus(true).await; },
                            oninput: move |event| update_shell.set_entry_search(event.value()),
                            onkeydown: move |event| {
                                if event.key() == Key::Escape && event.modifiers().is_empty() && !event.is_composing() {
                                    escape_shell.close_search();
                                    document::eval("document.querySelector('[data-action=toggle-search]')?.focus()");
                                }
                            },
                        }
                    }
                } else {
                    div { "data-layout": "app-nav-links",
                        Link { class: "icon-link-button", "data-nav": "feeds", to: AppRoute::FeedsPage {}, aria_label: "订阅", title: "Subscribe / 订阅",
                            // 订阅用通行的 feed 波纹图形；单个字母 "S" 需要用户记忆含义。
                            svg { width: "20", height: "20", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", stroke_linecap: "round", "aria-hidden": "true",
                                path { d: "M5 11a8 8 0 0 1 8 8" }
                                path { d: "M5 5a14 14 0 0 1 14 14" }
                                circle { cx: "6", cy: "18", r: "1.5", fill: "currentColor", stroke: "none" }
                            }
                        }
                        Link { class: "icon-link-button", "data-nav": "settings", to: AppRoute::SettingsPage {}, aria_label: "设置", title: "设置",
                            // 与搜索图标同一描边体系；"⚙" 字符在各平台会落到不同 emoji 字体。
                            svg { width: "20", height: "20", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", stroke_linecap: "round", "aria-hidden": "true",
                                path { d: "M4 7h10M18 7h2M4 17h4M12 17h8" }
                                circle { cx: "16", cy: "7", r: "2" }
                                circle { cx: "10", cy: "17", r: "2" }
                            }
                        }
                    }
                }
              }
            }
            button {
                class: "icon-link-button", r#type: "button",
                "data-action": "toggle-nav", "data-slot": "app-nav-toggle",
                aria_label: if shell.is_collapsed() { "向右展开导航" } else { "向左收起导航" },
                title: if shell.is_collapsed() { "向右展开导航" } else { "向左收起导航" },
                aria_expanded: !shell.is_collapsed(), aria_controls: "app-nav-content",
                onclick: move |_| collapse_shell.toggle_collapsed(),
                svg { width: "12", height: "20", view_box: "0 0 12 20", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round", "aria-hidden": "true",
                    path { d: if shell.is_collapsed() { "m4 5 5 5-5 5" } else { "m8 5-5 5 5 5" } }
                }
            }
            output { "data-slot": "manual-refresh-status", "data-state": refresh.phase(), role: "status", aria_live: "polite", "{refresh.label()}" }
        }
    }
}

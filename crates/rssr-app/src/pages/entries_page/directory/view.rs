//! Read-only directory rendering. State transitions stay in `controller.rs`;
//! event delegation consumes the stable `data-directory-*` attributes below.
use super::{DirectoryModel, DirectoryState, DirectoryStore};
use crate::pages::entries_page::{
    groups::{EntryCardRef, EntryDirectoryDate, EntryDirectorySourceMonth},
    state::EntryGroupingMode,
};
use dioxus::prelude::*;

const VIRTUAL_ARTICLE_THRESHOLD: usize = 80;

#[derive(Clone, Copy)]
enum ArticleBranchKind {
    Date,
    SourceMonth,
}

/// Borrowed display metadata shared by the two grouping trees; no second model.
struct ArticleBranch<'a> {
    kind: ArticleBranchKind,
    anchor: &'a str,
    title: &'a str,
    subtitle: &'a str,
    target_page: u32,
    cards: &'a [EntryCardRef],
}

impl<'a> ArticleBranch<'a> {
    fn date(date: &'a EntryDirectoryDate) -> Self {
        Self {
            kind: ArticleBranchKind::Date,
            anchor: &date.anchor_id,
            title: &date.title,
            subtitle: &date.subtitle,
            target_page: date.target_page,
            cards: &date.entry_cards,
        }
    }

    fn source_month(month: &'a EntryDirectorySourceMonth) -> Self {
        Self {
            kind: ArticleBranchKind::SourceMonth,
            anchor: &month.anchor_id,
            title: &month.title,
            subtitle: &month.subtitle,
            target_page: month.target_page,
            cards: &month.entry_cards,
        }
    }
}

#[component]
pub(crate) fn DirectoryRail(
    model: DirectoryModel,
    grouping_mode: EntryGroupingMode,
    on_close: EventHandler,
) -> Element {
    let DirectoryStore(state) = use_context();
    use_context_provider(super::virtual_articles::Cache::default);
    let state = state.read();
    let epoch = state.epoch();
    let revision = state.revision();
    rsx! {
        aside { id: "reading-directory", "data-layout": "entry-directory-rail", "data-directory-consumer": "rail",
            onkeydown: move |event| { if event.key() == Key::Escape { on_close.call(()); } },
            "data-directory-epoch": "{epoch}", "data-directory-revision": "{revision}",
            "data-directory-mode": if state.is_manual() { "manual" } else { "follow" },
            "data-directory-rendered": state.is_visible(),
            "data-directory-current-group": state.active_group().unwrap_or(""),
            "data-directory-current-item": state.active_item().unwrap_or(""),
            "data-directory-current-leaf": state.active_leaf().unwrap_or(""),
            div { class: "directory-heading-row",
                h2 { "data-slot": "entry-directory-heading", "目录" }
                button { class: "workspace-icon-button", title: "收起目录", aria_label: "收起目录", "data-action": "close-directory", onclick: move |_| on_close.call(()), crate::components::icon::Icon { name: "chevron-right" } }
            }
            if state.is_visible() {
              nav { "data-layout": "entry-directory-nav", "aria-label": "文章目录导航",
                if grouping_mode == EntryGroupingMode::Time {
                    for month in &model.directory_months {
                        div { "data-layout": "entry-directory-section", key: "{month.anchor_id}",
                            { group_toggle(&state, &month.anchor_id, &month.title, &month.subtitle, EntryGroupingMode::Time) }
                            div { "data-layout": "entry-directory-children", "data-directory-section-body": "true",
                                "data-open-base": boolean(state.is_open(&month.anchor_id)), "data-open": boolean(state.is_open(&month.anchor_id)),
                              if state.is_open(&month.anchor_id) {
                                { month.dates.iter().map(|date| article_branch(&state, &model, &month.anchor_id, ArticleBranch::date(date))) }
                              }
                            }
                        }
                    }
                } else {
                    for source in &model.directory_sources {
                        div { "data-layout": "entry-directory-section", key: "{source.anchor_id}",
                            { group_toggle(&state, &source.anchor_id, &source.title, &source.subtitle, EntryGroupingMode::Source) }
                            div { "data-layout": "entry-directory-grandchildren", "data-directory-section-body": "true",
                                "data-open-base": boolean(state.is_open(&source.anchor_id)), "data-open": boolean(state.is_open(&source.anchor_id)),
                              if state.is_open(&source.anchor_id) {
                                { source.months.iter().map(|month| article_branch(&state, &model, &source.anchor_id, ArticleBranch::source_month(month))) }
                              }
                            }
                        }
                    }
                }
              }
            }
        }
    }
}

fn article_branch(
    state: &DirectoryState,
    model: &DirectoryModel,
    parent_anchor: &str,
    branch: ArticleBranch<'_>,
) -> Element {
    let layout = match branch.kind {
        ArticleBranchKind::Date => "entry-directory-date-branch",
        ArticleBranchKind::SourceMonth => "entry-directory-month-branch",
    };
    rsx! {
        div { key: "{branch.anchor}", "data-layout": layout,
            div { class: "directory-branch-heading",
                // Navigation stays left of disclosure in visual and keyboard order.
                { branch_link(state, parent_anchor, &branch) }
                { branch_toggle(state, branch.anchor, branch.title) }
            }
            { article_rows(state, model, branch.anchor, branch.cards) }
        }
    }
}

fn article_rows(
    state: &DirectoryState,
    model: &DirectoryModel,
    anchor: &str,
    cards: &[EntryCardRef],
) -> Element {
    if state.is_open(anchor) && cards.len() > VIRTUAL_ARTICLE_THRESHOLD {
        return rsx! { super::virtual_articles::VirtualArticles {
            key: "{state.epoch()}:{anchor}", model: model.clone(), anchor: anchor.to_owned(),
            epoch: state.epoch(), active: state.active_leaf().map(str::to_owned),
        }};
    }
    rsx! {
        div { id: "directory-articles-{anchor}", "data-layout": "entry-directory-articles",
            // Only expanded paths mount article nodes; the projection keeps refs.
            if state.is_open(anchor) {
                for article in cards.iter().filter_map(|card| model.resolve_article(card)) {
                    DirectoryArticle { key: "{article.entry.id}", entry: std::sync::Arc::clone(article.entry),
                        active: state.active_leaf() == Some(&super::super::groups::entry_anchor_id(article.entry.id)),
                        page: article.page }
                }
            }
        }
    }
}

fn branch_toggle(state: &DirectoryState, anchor: &str, title: &str) -> Element {
    let open = state.is_open(anchor);
    let verb = if open { "收起" } else { "展开" };
    rsx! {
        button {
            r#type: "button", class: "workspace-icon-button directory-branch-toggle",
            "data-layout": "entry-directory-branch-toggle", "data-directory-anchor": anchor,
            "data-can-toggle": "true", "data-open": boolean(open),
            aria_expanded: boolean(open), aria_controls: "directory-articles-{anchor}",
            aria_label: "{verb}{title}的文章",
        }
    }
}

#[component]
pub(super) fn DirectoryArticle(
    entry: std::sync::Arc<rssr_domain::EntrySummary>,
    active: bool,
    page: u32,
    #[props(default)] window_index: Option<usize>,
    #[props(default)] window_count: usize,
) -> Element {
    let anchor = super::super::groups::entry_anchor_id(entry.id);
    let title = &entry.title;
    rsx! {
        button { r#type: "button", "data-layout": "entry-directory-link",
            "data-directory-kind": "entry", "data-directory-level": "article",
            "data-directory-anchor": "{anchor}", "data-directory-page": "{page}",
            "data-virtual-index": window_index,
            "data-virtual-prev": window_index.and_then(|index| index.checked_sub(1)),
            "data-virtual-next": window_index.filter(|index| index + 1 < window_count).map(|index| index + 1),
            "data-active": boolean(active), aria_current: if active { "location" } else { "false" },
            aria_label: "定位文章：{title}", title,
            span { "data-slot": "entry-directory-title", "{title}" }
        }
    }
}

fn boolean(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn group_toggle(
    state: &DirectoryState,
    anchor: &str,
    title: &str,
    subtitle: &str,
    grouping_mode: EntryGroupingMode,
) -> Element {
    let open = state.is_open(anchor);
    let (level, nav, action) = match grouping_mode {
        EntryGroupingMode::Time => ("month", Some("entry-directory-month"), None),
        EntryGroupingMode::Source => (
            "source",
            None,
            Some(if open { "collapse-directory-source" } else { "expand-directory-source" }),
        ),
    };
    rsx! {
        button { "data-layout": "entry-directory-toggle", r#type: "button",
            "data-directory-kind": "group", "data-directory-anchor": anchor,
            "data-directory-level": level, "data-nav": nav, "data-action": action,
            "data-active": boolean(state.active_group() == Some(anchor)),
            "data-can-toggle": "true", "data-open-base": boolean(open), "data-open": boolean(open),
            aria_disabled: "false", aria_expanded: boolean(open),
            span { "data-slot": "entry-directory-title", "{title}" }
            span { "data-slot": "entry-directory-meta", title: subtitle, "{compact_count(subtitle)}" }
        }
    }
}

fn branch_link(state: &DirectoryState, group: &str, branch: &ArticleBranch<'_>) -> Element {
    let (level, nav) = match branch.kind {
        ArticleBranchKind::Date => ("date", "entry-directory-date"),
        ArticleBranchKind::SourceMonth => ("month", "entry-directory-month"),
    };
    rsx! {
        button { key: "{branch.anchor}", "data-layout": "entry-directory-link", r#type: "button",
            "data-directory-kind": "item", "data-directory-group-anchor": group,
            "data-directory-level": level, "data-nav": nav,
            "data-directory-anchor": branch.anchor, "data-directory-page": "{branch.target_page}",
            "data-active": boolean(state.active_item() == Some(branch.anchor)),
            span { "data-slot": "entry-directory-title", "{branch.title}" }
            span { "data-slot": "entry-directory-meta", title: branch.subtitle, "{compact_count(branch.subtitle)}" }
        }
    }
}

fn compact_count(subtitle: &str) -> &str {
    subtitle.strip_suffix("篇文章").unwrap_or(subtitle).trim()
}

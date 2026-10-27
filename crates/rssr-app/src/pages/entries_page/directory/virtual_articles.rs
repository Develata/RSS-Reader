//! Dioxus/transport binding for the renderer-independent variable-height window.
use super::{
    DirectoryModel,
    window::{Part, View, Window},
};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

#[derive(Clone, Default)]
pub(super) struct Cache(Rc<RefCell<CacheData>>);
#[derive(Default)]
struct CacheData {
    epoch: u64,
    windows: HashMap<String, Rc<RefCell<Window>>>,
}
impl Cache {
    fn get(&self, epoch: u64, anchor: &str, count: usize) -> Rc<RefCell<Window>> {
        let mut cache = self.0.borrow_mut();
        if cache.epoch != epoch {
            cache.epoch = epoch;
            cache.windows.clear();
        }
        cache
            .windows
            .entry(anchor.to_owned())
            .or_insert_with(|| Rc::new(RefCell::new(Window::new(count))))
            .clone()
    }
}

#[derive(Clone, PartialEq)]
struct Rendered {
    view: View,
    revision: u64,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Input {
    Measure {
        sequence: u64,
        top: f64,
        viewport: f64,
        width: f64,
        gap: f64,
        rows: Vec<(usize, f64)>,
        focused: Option<usize>,
    },
    Reveal {
        sequence: u64,
        anchor: Option<String>,
        index: Option<usize>,
        #[serde(default)]
        focus: bool,
    },
}
#[derive(Serialize)]
struct Reply {
    sequence: u64,
    revision: u64,
    delta: f64,
    top: Option<f64>,
    focus: Option<usize>,
    found: bool,
}

#[component]
pub(super) fn VirtualArticles(
    model: DirectoryModel,
    anchor: String,
    epoch: u64,
    active: Option<String>,
) -> Element {
    let cache: Cache = use_context();
    let count = model.article_cards(&anchor).len();
    let window = use_hook(|| cache.get(epoch, &anchor, count));
    let mut rendered = use_signal(|| Rendered { view: window.borrow().view(), revision: 0 });
    let bridge = use_hook(|| Rc::new(Cell::new(None::<document::Eval>)));
    let cleanup = bridge.clone();
    use_drop(move || {
        if let Some(eval) = cleanup.get() {
            let _ = eval.send(serde_json::Value::Null);
        }
    });
    let id = format!("directory-articles-{anchor}");
    let script = format!(
        "const windowId = {};\n{}",
        serde_json::to_string(&id).unwrap(),
        include_str!("window_adapter.js")
    );
    let receiver_model = model.clone();
    let receiver_anchor = anchor.clone();
    let snapshot = rendered.read();
    // Keep the keyed row VNodes at the iterator root. An extra unkeyed RSX
    // fragment per part would recreate focused buttons when the range shifts.
    // A query can shrink the model one render before the reset effect advances
    // the epoch. Omit obsolete rows; an unkeyed empty VNode would violate the
    // keyed iterator contract and panic while diffing an archive/filter update.
    let children = snapshot.view.parts.iter().filter_map(|part| match part {
        Part::Space { start, height } => Some(rsx! {
            div { key: "space-{start}", "data-window-spacer": "true", aria_hidden: "true",
                style: "height: {height}px; min-height: {height}px; pointer-events: none;" }
        }),
        Part::Row(index) => model.article_cards(&anchor).get(*index)
            .and_then(|card| model.resolve_article(card))
            .map(|article| rsx! { super::view::DirectoryArticle {
                key: "{article.entry.id}", entry: article.entry.clone(), active: active.as_deref() == Some(&super::super::groups::entry_anchor_id(article.entry.id)),
                page: article.page,
                window_index: Some(*index), window_count: count,
            }}),
    });
    rsx! {
        div { id, "data-layout": "entry-directory-articles", "data-virtual-articles": "true",
            "data-virtual-count": count, "data-window-revision": snapshot.revision,
            style: "overflow-anchor: none;",
            onmounted: move |_| {
                if bridge.get().is_some() { return; }
                let window = window.clone();
                let bridge = bridge.clone();
                let model = receiver_model.clone();
                let anchor = receiver_anchor.clone();
                let script = script.clone();
                spawn(async move {
                    let mut eval = document::eval(&script);
                    bridge.set(Some(eval));
                    let mut revision = rendered.peek().revision;
                    while let Ok(input) = super::transport::receive_message::<Input>(&mut eval).await {
                        let mut reply = Reply { sequence: 0, revision, delta: 0.0, top: None, focus: None, found: true };
                        let view = {
                            let mut window = window.borrow_mut();
                            match input {
                                Input::Measure {sequence, top, viewport, width, gap, rows, focused} => {
                                    reply.sequence = sequence;
                                    reply.delta = window.measure(top, viewport, width, gap, &rows, focused);
                                }
                                Input::Reveal {sequence, anchor: target, index, focus} => {
                                    reply.sequence = sequence;
                                    let index = index.or_else(|| target.as_deref().and_then(|target| target.strip_prefix("entry-card-")?.parse::<i64>().ok())
                                        .and_then(|id| model.article_cards(&anchor).iter().position(|card| card.id == id)));
                                    reply.top = index.and_then(|index| window.reveal(index, focus));
                                    reply.found = reply.top.is_some();
                                    if focus && reply.found { reply.focus = index; }
                                }
                            }
                            window.view()
                        };
                        if rendered.peek().view != view {
                            revision += 1;
                            rendered.set(Rendered {view, revision});
                        }
                        reply.revision = revision;
                        if eval.send(reply).is_err() { break; }
                    }
                    let _ = eval.send(serde_json::Value::Null);
                    bridge.set(None);
                });
            },
            {children}
        }
    }
}

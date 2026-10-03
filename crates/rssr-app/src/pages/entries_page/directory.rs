//! Page-local directory interaction state. The presenter supplies initial paths only.
use std::{cell::Cell, collections::BTreeSet, rc::Rc, sync::Arc};

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use super::{
    intent::EntriesPageIntent, presenter::EntriesPagePresenter, session::EntriesPageSession,
};
use crate::ui::use_reactive_side_effect;

mod view;
pub(super) use view::{DirectoryRail, DirectoryTop};

// Cheap view-prop identity. Presenter metadata can change without changing the
// directory; resetting interaction state requires the narrower identity below.
#[derive(Clone)]
pub(crate) struct DirectoryModel(pub(super) Arc<EntriesPagePresenter>);

impl PartialEq for DirectoryModel {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl std::ops::Deref for DirectoryModel {
    type Target = EntriesPagePresenter;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone)]
struct DirectoryResetIdentity(DirectoryModel);

impl PartialEq for DirectoryResetIdentity {
    fn eq(&self, other: &Self) -> bool {
        let left = &self.0;
        let right = &other.0;
        left == right
            || (left.current_page == right.current_page
                && left.page_size == right.page_size
                && left.page_start == right.page_start
                && left.page_end == right.page_end
                && left.active_group_anchor == right.active_group_anchor
                && left.active_directory_anchor == right.active_directory_anchor
                && left.group_nav_items == right.group_nav_items
                && left.directory_months == right.directory_months
                && left.directory_sources == right.directory_sources)
    }
}

#[derive(Clone, Default, PartialEq, Eq)]
struct Path {
    group: Option<String>,
    item: Option<String>,
}

#[derive(Clone, Default, PartialEq, Eq)]
pub(super) struct DirectoryState {
    epoch: u64,
    revision: u64,
    initial: Path,
    observed: Option<Path>,
    manual: bool,
    expanded: BTreeSet<String>,
    sequence: u64,
}

impl DirectoryState {
    fn active(&self) -> &Path {
        self.observed.as_ref().unwrap_or(&self.initial)
    }

    fn is_open(&self, anchor: &str) -> bool {
        if self.manual {
            self.expanded.contains(anchor)
        } else {
            self.active().group.as_deref() == Some(anchor)
        }
    }

    fn enter_manual(&mut self, rendered_group: Option<&str>) -> bool {
        if !self.manual {
            self.expanded.clear();
            self.expanded.extend(rendered_group.map(str::to_owned));
            self.manual = true;
            return true;
        }
        false
    }

    fn apply(&mut self, fact: &Fact) -> bool {
        if fact.epoch != self.epoch || fact.sequence <= self.sequence {
            return false;
        }
        self.sequence = fact.sequence;
        let first_observation = self.observed.is_none();
        let path_changed = self.active().group != fact.group || self.active().item != fact.item;
        let was_manual = self.manual;
        let changed = match fact.kind {
            Kind::Observe | Kind::Follow => {
                let path = Path { group: fact.group.clone(), item: fact.item.clone() };
                let changed = self.observed.as_ref() != Some(&path)
                    || (matches!(fact.kind, Kind::Follow) && self.manual);
                self.observed = Some(path);
                if matches!(fact.kind, Kind::Follow) {
                    self.manual = false;
                    self.expanded.clear();
                }
                changed
            }
            Kind::Manual | Kind::Navigate => self.enter_manual(fact.rendered_group.as_deref()),
            Kind::SetOpen => {
                let mut changed = self.enter_manual(fact.rendered_group.as_deref());
                if let Some(anchor) = &fact.anchor {
                    if fact.open {
                        changed |= self.expanded.insert(anchor.clone());
                    } else {
                        changed |= self.expanded.remove(anchor);
                    }
                }
                changed
            }
        };
        if changed {
            self.revision += 1;
        }
        !self.manual
            && (first_observation
                || fact.reflow
                || path_changed
                || was_manual
                || matches!(fact.kind, Kind::Follow))
    }
}

#[derive(Clone, Copy)]
pub(super) struct DirectoryStore(pub(super) Signal<DirectoryState>);

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Observe,
    Manual,
    Follow,
    SetOpen,
    Navigate,
}

#[derive(Deserialize)]
struct Fact {
    epoch: u64,
    sequence: u64,
    kind: Kind,
    group: Option<String>,
    item: Option<String>,
    rendered_group: Option<String>,
    anchor: Option<String>,
    #[serde(default)]
    open: bool,
    page: Option<u32>,
    #[serde(default)]
    reflow: bool,
}

#[derive(Serialize)]
struct Command {
    epoch: u64,
    sequence: u64,
    revision: u64,
    align: bool,
    group: Option<String>,
    item: Option<String>,
    navigate: Option<String>,
    page: Option<u32>,
}

#[component]
pub(super) fn DirectoryBridge(
    model: DirectoryModel,
    context_key: String,
    session: EntriesPageSession,
) -> Element {
    let DirectoryStore(mut state) = use_context();
    use_reactive_side_effect((context_key, DirectoryResetIdentity(model)), move |(_, identity)| {
        let model = identity.0;
        let previous = state.peek();
        let next = DirectoryState {
            epoch: previous.epoch + 1,
            revision: previous.revision + 1,
            initial: Path {
                group: model.active_group_anchor.clone(),
                item: model.active_directory_anchor.clone(),
            },
            ..Default::default()
        };
        drop(previous);
        state.set(next);
    });
    let bridge = use_hook(|| Rc::new(Cell::new(None::<document::Eval>)));
    let cleanup = bridge.clone();
    use_drop(move || {
        if let Some(eval) = cleanup.get() {
            let _ = eval.send(serde_json::Value::Null);
        }
    });
    use_future(move || {
        let bridge = bridge.clone();
        async move {
            let mut eval = document::eval(include_str!("directory/adapter.js"));
            bridge.set(Some(eval));
            let mut current = state.peek().clone();
            while let Ok(fact) = eval.recv::<Fact>().await {
                if current.epoch != state.peek().epoch {
                    current = state.peek().clone();
                }
                let revision = current.revision;
                let valid = fact.epoch == current.epoch && fact.sequence > current.sequence;
                let align = current.apply(&fact);
                let navigate = if valid && matches!(fact.kind, Kind::Navigate) {
                    fact.anchor.clone()
                } else {
                    None
                };
                if navigate.is_some()
                    && let Some(page) = fact.page
                {
                    session.dispatch(EntriesPageIntent::SetCurrentPage(page));
                }
                let command = Command {
                    epoch: current.epoch,
                    sequence: fact.sequence,
                    revision: current.revision,
                    align,
                    group: current.active().group.clone(),
                    item: current.active().item.clone(),
                    navigate,
                    page: fact.page,
                };
                if current.revision != revision {
                    state.set(current.clone());
                }
                if eval.send(command).is_err() {
                    break;
                }
            }
        }
    });
    let state = state.read();
    rsx! { span { hidden: true, "data-directory-controller": "true", "data-directory-epoch": "{state.epoch}" } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_identity_ignores_feed_summary_but_tracks_page_and_directory_structure() {
        use crate::pages::entries_page::{
            groups::EntryGroupNavItem, presenter::EntriesPresenterInput, state::EntriesPageState,
        };
        let presenter = EntriesPagePresenter::from_input(&EntriesPresenterInput::from_state(
            &EntriesPageState::new(false),
            None,
        ));
        let before = DirectoryResetIdentity(DirectoryModel(Arc::new(presenter.clone())));
        let mut after = presenter.clone();
        after.source_filter_options.push((1, "Feed".into(), "https://feed.test".into(), 11));
        assert!(before == DirectoryResetIdentity(DirectoryModel(Arc::new(after.clone()))));
        after.current_page += 1;
        assert!(before != DirectoryResetIdentity(DirectoryModel(Arc::new(after))));
        let mut after = presenter;
        after.group_nav_items.push(EntryGroupNavItem {
            anchor_id: "new-group".into(),
            title: "New group".into(),
            subtitle: "1".into(),
            target_page: 1,
            is_active: false,
        });
        assert!(before != DirectoryResetIdentity(DirectoryModel(Arc::new(after))));
    }

    fn state() -> DirectoryState {
        DirectoryState {
            epoch: 1,
            initial: Path { group: Some("a".into()), item: Some("a1".into()) },
            ..Default::default()
        }
    }
    fn fact(sequence: u64, kind: Kind) -> Fact {
        Fact {
            epoch: 1,
            sequence,
            kind,
            group: Some("a".into()),
            item: Some("a1".into()),
            rendered_group: Some("a".into()),
            anchor: Some("a".into()),
            open: false,
            page: None,
            reflow: false,
        }
    }
    #[test]
    fn current_group_can_close_and_same_group_user_scroll_resets_all_manual_groups() {
        let mut state = state();
        assert!(state.is_open("a"));
        assert!(!state.apply(&fact(1, Kind::SetOpen)));
        assert!(!state.is_open("a"));
        let mut open_b = fact(2, Kind::SetOpen);
        open_b.anchor = Some("b".into());
        open_b.open = true;
        state.apply(&open_b);
        assert!(state.is_open("b"));
        assert!(state.apply(&fact(3, Kind::Follow)));
        assert!(state.is_open("a"));
        assert!(!state.is_open("b"));
        assert!(state.expanded.is_empty());
    }
    #[test]
    fn passive_observation_does_not_interrupt_manual_browsing() {
        let mut state = state();
        state.apply(&fact(1, Kind::Manual));
        let mut observe = fact(2, Kind::Observe);
        observe.group = Some("b".into());
        observe.item = Some("b1".into());
        assert!(!state.apply(&observe));
        assert!(state.is_open("a"));
        assert!(!state.is_open("b"));
        assert_eq!(state.active().group.as_deref(), Some("b"));
        assert_eq!(state.initial.group.as_deref(), Some("a"));
    }
    #[test]
    fn button_intent_uses_presented_state_instead_of_inverting_later_observation() {
        let mut state = state();
        let mut observe = fact(1, Kind::Observe);
        observe.group = Some("b".into());
        state.apply(&observe);
        state.apply(&fact(2, Kind::SetOpen));
        assert!(!state.is_open("a"));
        assert!(!state.is_open("b"));
    }
    #[test]
    fn rejects_prior_context_and_out_of_order_messages() {
        let mut state = state();
        state.apply(&fact(3, Kind::SetOpen));
        let prior = state.clone();
        assert!(!state.apply(&fact(2, Kind::Follow)));
        let mut stale = fact(4, Kind::Follow);
        stale.epoch = 0;
        assert!(!state.apply(&stale));
        assert!(state == prior);
    }
}

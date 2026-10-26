//! Renderer-independent directory state and adapter replies.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Default, PartialEq, Eq)]
struct Path {
    group: Option<String>,
    item: Option<String>,
    leaf: Option<String>,
}

#[derive(Clone, Default, PartialEq, Eq)]
pub(crate) struct DirectoryState {
    epoch: u64,
    revision: u64,
    initial: Path,
    observed: Option<Path>,
    manual: bool,
    visible: bool,
    expanded: BTreeSet<String>,
    sequence: u64,
}

impl DirectoryState {
    pub(super) fn epoch(&self) -> u64 {
        self.epoch
    }

    pub(super) fn revision(&self) -> u64 {
        self.revision
    }

    pub(super) fn is_manual(&self) -> bool {
        self.manual
    }

    pub(super) fn is_visible(&self) -> bool {
        self.visible
    }

    pub(super) fn active_group(&self) -> Option<&str> {
        self.active().group.as_deref()
    }

    pub(super) fn active_item(&self) -> Option<&str> {
        self.active().item.as_deref()
    }

    pub(super) fn active_leaf(&self) -> Option<&str> {
        self.active().leaf.as_deref()
    }

    pub(super) fn reset(&mut self, group: Option<String>, item: Option<String>) {
        *self = Self {
            epoch: self.epoch + 1,
            revision: self.revision + 1,
            visible: self.visible,
            initial: Path { group, item, leaf: None },
            ..Default::default()
        };
    }

    /// Every fact receives an acknowledgement, including stale facts. The DOM
    /// adapter keeps only one message in flight and needs its original sequence
    /// back to continue; stale input must never trigger navigation or scrolling.
    pub(super) fn receive(&mut self, fact: &Fact) -> Reply {
        let revision = self.revision;
        let valid = fact.epoch == self.epoch && fact.sequence > self.sequence;
        let align = if valid {
            self.sequence = fact.sequence;
            self.apply_valid(fact)
        } else {
            false
        };
        let navigate =
            if valid && matches!(fact.kind, Kind::Navigate) { fact.anchor.clone() } else { None };
        let set_page = navigate.as_ref().and(fact.page);
        Reply {
            command: Command {
                epoch: self.epoch,
                sequence: fact.sequence,
                revision: self.revision,
                align,
                group: self.active().group.clone(),
                item: self.active().item.clone(),
                leaf: self.active().leaf.clone(),
                navigate,
                page: fact.page,
                restore_scroll: valid
                    && matches!(fact.kind, Kind::Visibility)
                    && self.visible
                    && self.manual,
            },
            set_page,
            publish: self.revision != revision,
        }
    }

    fn active(&self) -> &Path {
        self.observed.as_ref().unwrap_or(&self.initial)
    }

    pub(super) fn is_open(&self, anchor: &str) -> bool {
        if self.manual {
            self.expanded.contains(anchor)
        } else {
            self.active().group.as_deref() == Some(anchor)
                || self.active().item.as_deref() == Some(anchor)
        }
    }

    fn enter_manual(&mut self, rendered_group: Option<&str>, rendered_item: Option<&str>) -> bool {
        if !self.manual {
            self.expanded.clear();
            self.expanded.extend(rendered_group.map(str::to_owned));
            self.expanded.extend(rendered_item.map(str::to_owned));
            self.manual = true;
            return true;
        }
        false
    }

    fn apply_valid(&mut self, fact: &Fact) -> bool {
        let first_observation = self.observed.is_none();
        let path_changed = self.active().group != fact.group
            || self.active().item != fact.item
            || self.active().leaf != fact.leaf;
        let was_manual = self.manual;
        let changed = match fact.kind {
            Kind::Visibility => {
                let changed = self.visible != fact.visible;
                self.visible = fact.visible;
                changed
            }
            Kind::Observe | Kind::Follow => {
                let path = Path {
                    group: fact.group.clone(),
                    item: fact.item.clone(),
                    leaf: fact.leaf.clone(),
                };
                let changed = self.observed.as_ref() != Some(&path)
                    || (matches!(fact.kind, Kind::Follow) && self.manual);
                self.observed = Some(path);
                if matches!(fact.kind, Kind::Follow) {
                    self.manual = false;
                    self.expanded.clear();
                }
                changed
            }
            Kind::Manual | Kind::Navigate => {
                self.enter_manual(fact.rendered_group.as_deref(), fact.rendered_item.as_deref())
            }
            Kind::SetOpen => {
                let mut changed = self
                    .enter_manual(fact.rendered_group.as_deref(), fact.rendered_item.as_deref());
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
        if matches!(fact.kind, Kind::Visibility) {
            return changed && self.visible && !self.manual;
        }
        !self.manual
            && (first_observation
                || fact.reflow
                || path_changed
                || was_manual
                || matches!(fact.kind, Kind::Follow))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Visibility,
    Observe,
    Manual,
    Follow,
    SetOpen,
    Navigate,
}

#[derive(Deserialize)]
pub(super) struct Fact {
    epoch: u64,
    sequence: u64,
    kind: Kind,
    group: Option<String>,
    item: Option<String>,
    leaf: Option<String>,
    rendered_group: Option<String>,
    rendered_item: Option<String>,
    anchor: Option<String>,
    #[serde(default)]
    open: bool,
    page: Option<u32>,
    #[serde(default)]
    reflow: bool,
    #[serde(default)]
    visible: bool,
}

#[derive(Serialize)]
pub(super) struct Command {
    epoch: u64,
    sequence: u64,
    revision: u64,
    align: bool,
    group: Option<String>,
    item: Option<String>,
    leaf: Option<String>,
    navigate: Option<String>,
    page: Option<u32>,
    restore_scroll: bool,
}

pub(super) struct Reply {
    pub(super) command: Command,
    pub(super) set_page: Option<u32>,
    pub(super) publish: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    impl DirectoryState {
        // Keep the state-transition cases concise, exercising the full entry
        // point including ordering checks rather than bypassing receive().
        fn apply(&mut self, fact: &Fact) -> bool {
            self.receive(fact).command.align
        }
    }

    fn state() -> DirectoryState {
        DirectoryState {
            epoch: 1,
            initial: Path { group: Some("a".into()), item: Some("a1".into()), leaf: None },
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
            leaf: None,
            rendered_group: Some("a".into()),
            rendered_item: Some("a1".into()),
            anchor: Some("a".into()),
            open: false,
            page: None,
            reflow: false,
            visible: false,
        }
    }

    #[test]
    fn stale_facts_still_acknowledge_the_request_without_effects() {
        let mut state = state();
        state.receive(&fact(3, Kind::Manual));
        for (epoch, sequence) in [(0, 4), (1, 2), (1, 3)] {
            let before = state.clone();
            let mut navigate = fact(sequence, Kind::Navigate);
            navigate.epoch = epoch;
            navigate.page = Some(7);
            let reply = state.receive(&navigate);
            assert!(!reply.publish);
            assert_eq!(reply.set_page, None);
            assert!(state == before);
            assert_eq!(
                serde_json::to_value(reply.command).unwrap(),
                serde_json::json!({
                    "epoch": 1, "sequence": sequence, "revision": 1,
                    "align": false, "group": "a", "item": "a1", "leaf": null,
                    "navigate": null, "page": 7, "restore_scroll": false
                })
            );
        }
    }

    #[test]
    fn navigation_page_requires_an_accepted_anchor() {
        let mut state = state();
        let mut navigate = fact(1, Kind::Navigate);
        navigate.anchor = Some("entry-card-72".into());
        navigate.page = Some(2);
        let reply = state.receive(&navigate);
        assert!(reply.publish);
        assert_eq!(reply.set_page, Some(2));
        assert_eq!(reply.command.navigate.as_deref(), Some("entry-card-72"));
        assert!(state.is_manual());

        navigate.sequence = 2;
        navigate.anchor = None;
        let reply = state.receive(&navigate);
        assert_eq!(reply.set_page, None);
        assert!(reply.command.navigate.is_none());
        navigate.sequence = 3;
        navigate.anchor = Some("a1".into());
        navigate.page = None;
        let reply = state.receive(&navigate);
        assert_eq!(reply.set_page, None);
        assert_eq!(reply.command.navigate.as_deref(), Some("a1"));
    }

    #[test]
    fn visibility_reply_restores_manual_scroll_and_aligns_following() {
        let mut state = state();
        let mut show = fact(1, Kind::Visibility);
        show.visible = true;
        let reply = state.receive(&show);
        assert!(reply.publish && reply.command.align && !reply.command.restore_scroll);
        state.receive(&fact(2, Kind::Manual));
        show.sequence = 3;
        let reply = state.receive(&show);
        assert!(!reply.publish && !reply.command.align && reply.command.restore_scroll);
        // Repeated visibility still restores scroll even without a render change.
        show.sequence = 4;
        assert!(state.receive(&show).command.restore_scroll);
        let stale = state.receive(&show);
        assert!(!stale.publish && !stale.command.restore_scroll && !stale.command.align);
        show.epoch = 0;
        show.sequence = 5;
        let stale_context = state.receive(&show);
        assert!(!stale_context.command.restore_scroll && !stale_context.publish);
        let reply = state.receive(&fact(5, Kind::Visibility));
        assert!(reply.publish && !reply.command.align && !reply.command.restore_scroll);
    }

    #[test]
    fn identical_observations_acknowledge_without_publishing_a_new_render() {
        let mut state = state();
        assert!(state.receive(&fact(1, Kind::Observe)).publish);
        let revision = state.revision();
        let reply = state.receive(&fact(2, Kind::Observe));
        assert!(!reply.publish && !reply.command.align);
        assert_eq!(reply.command.sequence, 2);
        assert_eq!(reply.command.revision, revision);
        assert_eq!(state.sequence, 2);
        let reply = state.receive(&fact(3, Kind::Follow));
        assert!(!reply.publish && reply.command.align);
        assert_eq!(reply.command.revision, revision);
    }

    #[test]
    fn reset_invalidates_old_input_and_preserves_only_visibility() {
        let mut state = state();
        let mut show = fact(1, Kind::Visibility);
        show.visible = true;
        state.receive(&show);
        state.receive(&fact(2, Kind::Observe));
        state.receive(&fact(3, Kind::Manual));
        let revision = state.revision();
        state.reset(Some("b".into()), Some("b2".into()));
        assert_eq!(state.epoch(), 2);
        assert_eq!(state.revision(), revision + 1);
        assert!(state.is_visible() && !state.is_manual());
        assert!(state.observed.is_none() && state.expanded.is_empty());
        assert_eq!(state.sequence, 0);
        assert_eq!(state.active_group(), Some("b"));
        assert_eq!(state.active_item(), Some("b2"));
        assert_eq!(state.active_leaf(), None);
        let before = state.clone();
        let reply = state.receive(&fact(4, Kind::Navigate));
        assert_eq!(reply.command.epoch, 2);
        assert_eq!(reply.command.sequence, 4);
        assert!(!reply.publish && reply.set_page.is_none() && state == before);
    }

    #[test]
    fn adapter_fact_names_and_optional_fields_remain_compatible() {
        for (name, expected) in [
            ("visibility", Kind::Visibility),
            ("observe", Kind::Observe),
            ("manual", Kind::Manual),
            ("follow", Kind::Follow),
            ("set_open", Kind::SetOpen),
            ("navigate", Kind::Navigate),
        ] {
            let fact: Fact = serde_json::from_value(serde_json::json!({
                "epoch": 1, "sequence": 9, "kind": name
            }))
            .unwrap();
            assert_eq!(std::mem::discriminant(&fact.kind), std::mem::discriminant(&expected));
            assert!(fact.group.is_none() && fact.item.is_none() && fact.leaf.is_none());
            assert!(fact.rendered_group.is_none() && fact.rendered_item.is_none());
            assert!(fact.anchor.is_none() && fact.page.is_none());
            assert!(!fact.visible && !fact.open && !fact.reflow);
        }
    }

    #[test]
    fn hiding_and_showing_preserves_manual_expansion_and_active_path() {
        let mut state = state();
        let mut show = fact(1, Kind::Visibility);
        show.visible = true;
        assert!(state.apply(&show));
        state.apply(&fact(2, Kind::Manual));
        let mut expand = fact(3, Kind::SetOpen);
        expand.anchor = Some("b".into());
        expand.open = true;
        state.apply(&expand);
        let expanded = state.expanded.clone();
        let active = state.active().clone();
        assert!(!state.apply(&fact(4, Kind::Visibility)));
        assert!(!state.visible);
        show.sequence = 5;
        assert!(!state.apply(&show), "manual reopening must restore scroll, not follow");
        assert!(state.visible && state.manual);
        assert!(state.expanded == expanded && *state.active() == active);
    }

    #[test]
    fn hidden_directory_tracks_main_scroll_and_reopens_at_current_path() {
        let mut state = state();
        state.apply(&fact(1, Kind::Manual));
        let mut follow = fact(2, Kind::Follow);
        follow.group = Some("b".into());
        follow.item = Some("b1".into());
        state.apply(&follow);
        assert!(!state.visible && !state.manual);
        let mut show = fact(3, Kind::Visibility);
        show.visible = true;
        assert!(state.apply(&show));
        assert!(state.is_open("b") && state.is_open("b1") && !state.is_open("a"));
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

    #[test]
    fn manual_browsing_preserves_the_rendered_month_and_can_close_it() {
        let mut state = state();
        assert!(state.is_open("a1"));
        state.apply(&fact(1, Kind::Manual));
        assert!(state.is_open("a"));
        assert!(state.is_open("a1"));
        let mut close_month = fact(2, Kind::SetOpen);
        close_month.anchor = Some("a1".into());
        state.apply(&close_month);
        assert!(state.is_open("a"));
        assert!(!state.is_open("a1"));
        let mut observe = fact(3, Kind::Observe);
        observe.leaf = Some("entry-card-42".into());
        assert!(!state.apply(&observe));
        assert!(!state.is_open("a1"));
        assert_eq!(state.active().leaf.as_deref(), Some("entry-card-42"));
        assert!(state.apply(&fact(4, Kind::Follow)));
        assert!(state.is_open("a1"));
    }

    #[test]
    fn follow_reopens_only_the_current_source_and_month_and_tracks_articles() {
        let mut state = state();
        state.apply(&fact(1, Kind::Manual));
        let mut follow = fact(2, Kind::Follow);
        follow.group = Some("b".into());
        follow.item = Some("b2".into());
        follow.leaf = Some("entry-card-8".into());
        assert!(state.apply(&follow));
        assert!(!state.is_open("a"));
        assert!(!state.is_open("a1"));
        assert!(state.is_open("b"));
        assert!(state.is_open("b2"));
        assert!(!state.is_open("b1"));
        follow.sequence = 3;
        follow.kind = Kind::Observe;
        follow.leaf = Some("entry-card-9".into());
        assert!(state.apply(&follow));
        assert_eq!(state.active().leaf.as_deref(), Some("entry-card-9"));
    }
}

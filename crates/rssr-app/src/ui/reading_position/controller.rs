//! Run-local position policy. The host supplies facts and applies commands;
//! neither storage nor restoration needs Dioxus or a DOM to run.
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize)]
struct Point {
    y: f64,
    anchor: Option<String>,
    offset: f64,
}

struct Saved {
    page: u32,
    point: Point,
}

#[derive(Default)]
pub(super) struct SavedPositions(HashMap<String, Saved>);

impl SavedPositions {
    pub(super) fn remembered_page(&self, key: &str) -> Option<u32> {
        self.0.get(key).map(|saved| saved.page)
    }

    pub(super) fn capture(&mut self, fact: Fact) {
        if fact.kind != FactKind::Capture {
            return;
        }
        let mut point = fact.point;
        if point.y <= 0.0 {
            point.anchor = None;
            point.offset = 0.0;
        }
        self.0.insert(fact.key, Saved { page: fact.page, point });
    }

    pub(super) fn clear(&mut self) {
        self.0.clear();
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum FactKind {
    Visit,
    Context,
    Input,
    Capture,
    Layout,
    #[serde(other)]
    Unknown,
}

#[derive(Deserialize)]
pub(super) struct Fact {
    kind: FactKind,
    visit: u64,
    sequence: u64,
    key: String,
    page: u32,
    now: f64,
    point: Point,
    max_y: f64,
    anchor_top: Option<f64>,
}

#[derive(Debug, Serialize)]
pub(super) struct Command {
    visit: u64,
    sequence: u64,
    anchor: Option<String>,
    y: Option<f64>,
    watch: bool,
    highlight: Option<String>,
}

struct Restoration {
    visit: u64,
    started: f64,
    point: Point,
}

impl Restoration {
    fn is_active(&self, fact: &Fact) -> bool {
        self.visit == fact.visit
            && !matches!(fact.kind, FactKind::Input | FactKind::Capture | FactKind::Context)
            && fact.now - self.started < 2000.0
    }
}

fn target_y(point: &Point, actual_y: f64, anchor_top: Option<f64>, max_y: f64) -> f64 {
    let y = anchor_top.map_or(point.y, |top| actual_y + top - point.offset);
    if !y.is_finite() || !max_y.is_finite() {
        return 0.0;
    }
    y.clamp(0.0, max_y.max(0.0))
}

/// One adapter lifetime. A remounted adapter starts its visit counter again;
/// its restorer must also be new, even when saved positions are shared.
#[derive(Default)]
pub(super) struct Restorer {
    restoration: Option<Restoration>,
    latest_visit: u64,
}

impl Restorer {
    pub(super) fn on_fact(
        &mut self,
        positions: &mut SavedPositions,
        fact: Fact,
    ) -> Option<Command> {
        if fact.visit < self.latest_visit {
            return None;
        }
        self.latest_visit = fact.visit;
        if fact.kind == FactKind::Visit {
            let point = positions
                .0
                .get(&fact.key)
                .filter(|saved| saved.page == fact.page)
                .map(|saved| saved.point.clone());
            // New reader visits start at the top; a new list context keeps its
            // existing pagination/filter positioning instead of restoring.
            let point = point.or_else(|| fact.key.starts_with("reader:").then(Point::default));
            self.restoration =
                point.map(|point| Restoration { visit: fact.visit, started: fact.now, point });
        }
        if self.restoration.as_ref().is_some_and(|active| !active.is_active(&fact)) {
            self.restoration = None;
        }
        let mut command = Command {
            visit: fact.visit,
            sequence: fact.sequence,
            anchor: None,
            y: None,
            watch: false,
            highlight: None,
        };
        if let Some(active) = &self.restoration {
            command.anchor = active.point.anchor.clone();
            command.watch = true;
            if fact.key.starts_with("list:") {
                command.highlight = active.point.anchor.clone();
            }
            // Unknown future facts must neither stop decoding nor treat an
            // absent layout range as a request to scroll to zero.
            if fact.kind == FactKind::Layout {
                command.y =
                    Some(target_y(&active.point, fact.point.y, fact.anchor_top, fact.max_y));
            }
        } else {
            positions.capture(fact);
        }
        Some(command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fact(kind: FactKind) -> Fact {
        Fact {
            kind,
            visit: 1,
            sequence: 1,
            key: "reader:1".into(),
            page: 1,
            now: 100.0,
            point: Point { y: 600.0, anchor: Some("entry:7".into()), offset: 120.0 },
            max_y: 1000.0,
            anchor_top: None,
        }
    }

    #[test]
    fn only_capture_saves_and_top_clears_the_anchor() {
        let mut positions = SavedPositions::default();
        positions.capture(fact(FactKind::Capture));
        for kind in [
            FactKind::Input,
            FactKind::Layout,
            FactKind::Visit,
            FactKind::Context,
            FactKind::Unknown,
        ] {
            positions.capture(Fact { point: Point::default(), ..fact(kind) });
        }
        assert_eq!(positions.0["reader:1"].point.y, 600.0);
        assert_eq!(positions.remembered_page("reader:1"), Some(1));
        positions.capture(Fact {
            point: Point { y: 0.0, anchor: Some("entry:7".into()), offset: 120.0 },
            ..fact(FactKind::Capture)
        });
        assert!(positions.0["reader:1"].point.anchor.is_none());
        assert_eq!(positions.0["reader:1"].point.offset, 0.0);
        positions.clear();
        assert_eq!(positions.remembered_page("reader:1"), None);
    }

    #[test]
    fn list_restoration_waits_for_layout_and_highlights_the_saved_entry() {
        let mut positions = SavedPositions::default();
        positions.capture(Fact { key: "list:all".into(), ..fact(FactKind::Capture) });
        let mut restorer = Restorer::default();
        let command = restorer
            .on_fact(&mut positions, Fact { key: "list:all".into(), ..fact(FactKind::Visit) })
            .unwrap();
        assert!(command.watch);
        assert_eq!(command.y, None);
        assert_eq!(command.anchor.as_deref(), Some("entry:7"));
        assert_eq!(command.highlight.as_deref(), Some("entry:7"));
        let command = restorer
            .on_fact(
                &mut positions,
                Fact {
                    key: "list:all".into(),
                    sequence: 2,
                    anchor_top: Some(350.0),
                    point: Point { y: 100.0, ..Point::default() },
                    ..fact(FactKind::Layout)
                },
            )
            .unwrap();
        assert_eq!(command.y, Some(330.0));
        assert_eq!(command.sequence, 2);
    }

    #[test]
    fn page_mismatch_and_missing_positions_preserve_reader_and_list_fallbacks() {
        for saved in [false, true] {
            for (key, watch) in [("reader:1", true), ("list:all", false)] {
                let mut positions = SavedPositions::default();
                if saved {
                    positions.capture(Fact { key: key.into(), page: 2, ..fact(FactKind::Capture) });
                }
                let mut restorer = Restorer::default();
                let command = restorer
                    .on_fact(&mut positions, Fact { key: key.into(), ..fact(FactKind::Visit) })
                    .unwrap();
                assert_eq!(command.watch, watch);
                assert_eq!(command.y, None);
                assert_eq!(command.highlight, None);
                let command = restorer
                    .on_fact(&mut positions, Fact { key: key.into(), ..fact(FactKind::Layout) })
                    .unwrap();
                assert_eq!(command.y, watch.then_some(0.0));
            }
        }
    }

    #[test]
    fn saved_reader_restores_without_a_list_highlight() {
        let mut positions = SavedPositions::default();
        positions.capture(fact(FactKind::Capture));
        let mut restorer = Restorer::default();
        let command = restorer.on_fact(&mut positions, fact(FactKind::Visit)).unwrap();
        assert_eq!(command.anchor.as_deref(), Some("entry:7"));
        assert!(command.highlight.is_none());
        let command = restorer.on_fact(&mut positions, fact(FactKind::Layout)).unwrap();
        assert_eq!(command.y, Some(600.0));
    }

    #[test]
    fn input_capture_and_context_cancel_restoration_and_capture_also_saves() {
        for kind in [FactKind::Input, FactKind::Capture, FactKind::Context] {
            let mut positions = SavedPositions::default();
            let mut restorer = Restorer::default();
            restorer.on_fact(&mut positions, fact(FactKind::Visit));
            let command = restorer.on_fact(&mut positions, fact(kind)).unwrap();
            assert!(!command.watch);
            assert_eq!(command.y, None);
            assert_eq!(positions.remembered_page("reader:1").is_some(), kind == FactKind::Capture);
            assert!(!restorer.on_fact(&mut positions, fact(FactKind::Layout)).unwrap().watch);
        }
    }

    #[test]
    fn every_fact_advances_visit_fencing_but_stale_visits_get_no_reply() {
        let mut positions = SavedPositions::default();
        let mut restorer = Restorer::default();
        restorer.on_fact(&mut positions, fact(FactKind::Visit));
        let command = restorer
            .on_fact(&mut positions, Fact { visit: 3, sequence: 9, ..fact(FactKind::Input) })
            .unwrap();
        assert_eq!((command.visit, command.sequence, command.watch), (3, 9, false));
        for kind in [FactKind::Visit, FactKind::Capture, FactKind::Layout] {
            assert!(restorer.on_fact(&mut positions, fact(kind)).is_none());
        }
        assert!(positions.0.is_empty());
    }

    #[test]
    fn deadline_uses_the_adapter_clock_and_sends_a_stop_command() {
        let mut positions = SavedPositions::default();
        let mut restorer = Restorer::default();
        restorer.on_fact(&mut positions, fact(FactKind::Visit));
        assert!(
            restorer
                .on_fact(&mut positions, Fact { now: 2099.0, ..fact(FactKind::Layout) })
                .unwrap()
                .watch
        );
        let command = restorer
            .on_fact(&mut positions, Fact { now: 2100.0, ..fact(FactKind::Layout) })
            .unwrap();
        assert!(!command.watch);
        assert_eq!(command.y, None);
    }

    #[test]
    fn native_capture_and_adapter_remount_do_not_share_visit_fences() {
        let mut positions = SavedPositions::default();
        let mut old = Restorer::default();
        old.on_fact(&mut positions, Fact { visit: 10, ..fact(FactKind::Visit) });
        // A host-side capture saves directly even if its visit precedes the
        // bridge's latest fact; it must not cancel that bridge's restoration.
        positions.capture(fact(FactKind::Capture));
        assert!(
            old.on_fact(&mut positions, Fact { visit: 10, ..fact(FactKind::Layout) })
                .unwrap()
                .watch
        );
        let mut remounted = Restorer::default();
        assert!(remounted.on_fact(&mut positions, fact(FactKind::Visit)).unwrap().watch);
        assert_eq!(
            remounted.on_fact(&mut positions, fact(FactKind::Layout)).unwrap().y,
            Some(600.0)
        );
    }

    #[test]
    fn wire_kinds_and_command_fields_remain_compatible() {
        for (name, expected) in [
            ("visit", FactKind::Visit),
            ("input", FactKind::Input),
            ("context", FactKind::Context),
            ("capture", FactKind::Capture),
            ("layout", FactKind::Layout),
            ("future-event", FactKind::Unknown),
        ] {
            let decoded: Fact = serde_json::from_value(serde_json::json!({
                "kind": name, "visit": 1, "sequence": 8, "key": "reader:1", "page": 1,
                "now": 100.0, "point": {"y": 0.0, "anchor": null, "offset": 0.0},
                "max_y": 1000.0, "anchor_top": null
            }))
            .unwrap();
            assert_eq!(decoded.kind, expected);
        }
        let mut positions = SavedPositions::default();
        let command = Restorer::default()
            .on_fact(&mut positions, Fact { sequence: 8, ..fact(FactKind::Input) })
            .unwrap();
        assert_eq!(
            serde_json::to_value(command).unwrap(),
            serde_json::json!({
                "visit": 1, "sequence": 8, "anchor": null, "y": null, "watch": false, "highlight": null
            })
        );
    }

    #[test]
    fn unknown_facts_cannot_scroll_using_an_absent_layout_range() {
        let mut positions = SavedPositions::default();
        positions.capture(fact(FactKind::Capture));
        let mut restorer = Restorer::default();
        restorer.on_fact(&mut positions, fact(FactKind::Visit));
        let command = restorer
            .on_fact(&mut positions, Fact { max_y: 0.0, ..fact(FactKind::Unknown) })
            .unwrap();
        assert!(command.watch);
        assert_eq!(command.y, None);
        assert_eq!(
            restorer.on_fact(&mut positions, fact(FactKind::Layout)).unwrap().y,
            Some(600.0)
        );
        let command = restorer
            .on_fact(
                &mut positions,
                Fact {
                    visit: 2,
                    page: 2,
                    point: Point { y: 50.0, ..Point::default() },
                    ..fact(FactKind::Unknown)
                },
            )
            .unwrap();
        assert!(!command.watch);
        assert_eq!(command.y, None);
        assert!(restorer.on_fact(&mut positions, fact(FactKind::Visit)).is_none());
        assert_eq!(positions.0["reader:1"].point.y, 600.0);
        assert_eq!(positions.remembered_page("reader:1"), Some(1));
    }

    #[test]
    fn anchor_moves_with_content_and_missing_anchor_uses_clamped_pixels() {
        let point = Point { y: 400.0, anchor: Some("entry:7".into()), offset: 100.0 };
        assert_eq!(target_y(&point, 500.0, Some(250.0), 1000.0), 650.0);
        assert_eq!(target_y(&point, 0.0, None, 300.0), 300.0);
        assert_eq!(target_y(&point, 0.0, None, 0.0), 0.0);
        assert_eq!(target_y(&point, 0.0, Some(-500.0), 1000.0), 0.0);
    }

    #[test]
    fn top_bottom_and_invalid_measurements_are_bounded() {
        assert_eq!(target_y(&Point::default(), 40.0, None, 400.0), 0.0);
        let point = Point { y: 900.0, ..Point::default() };
        assert_eq!(target_y(&point, 0.0, None, 900.0), 900.0);
        assert_eq!(target_y(&point, 0.0, Some(f64::NAN), 900.0), 0.0);
    }
}

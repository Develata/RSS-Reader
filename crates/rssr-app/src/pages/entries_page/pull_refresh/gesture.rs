//! Pure gesture policy. Adapters report points, target facts and scroll capacity;
//! this module owns movement history, eligibility, phase and release decisions.
use serde::Deserialize;

const THRESHOLD: f64 = 80.0;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum TouchKind {
    Start,
    Move,
    End,
    Cancel,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct Point {
    x: f64,
    y: f64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub(super) struct TouchSample {
    kind: TouchKind,
    point: Option<Point>,
    at_top: bool,
    on_control: bool,
    has_selection: bool,
    nested_above: bool,
    nested_below: bool,
    touches: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) enum Phase {
    #[default]
    Idle,
    Pulling,
    Armed,
}

struct Tracking {
    start: Point,
    last_y: f64,
    distance: f64,
}

#[derive(Default)]
pub(super) struct Gesture(Option<Tracking>);

impl Gesture {
    pub(super) fn phase(&self) -> Phase {
        match &self.0 {
            Some(tracking) if tracking.distance >= THRESHOLD => Phase::Armed,
            Some(tracking) if tracking.distance > 8.0 => Phase::Pulling,
            _ => Phase::Idle,
        }
    }

    pub(super) fn update(&mut self, sample: TouchSample, refreshing: bool) -> bool {
        let expected_touches = match sample.kind {
            TouchKind::Start | TouchKind::Move => 1,
            TouchKind::End | TouchKind::Cancel => 0,
        };
        if matches!(sample.kind, TouchKind::Cancel)
            || refreshing
            || !sample.at_top
            || sample.on_control
            || sample.has_selection
            || sample.touches != expected_touches
        {
            self.0 = None;
            return false;
        }
        let Some(point) = sample.point else {
            self.0 = None;
            return false;
        };
        match sample.kind {
            TouchKind::Start => {
                // There is no movement yet, even if a nested scroller has room.
                self.0 = Some(Tracking { start: point, last_y: point.y, distance: 0.0 });
            }
            TouchKind::Move | TouchKind::End => {
                let Some(tracking) = &self.0 else {
                    return false;
                };
                let delta_y = point.y - tracking.last_y;
                let nested_consumes = delta_y.abs() >= 0.5
                    && ((delta_y > 0.0 && sample.nested_above)
                        || (delta_y < 0.0 && sample.nested_below));
                let distance = point.y - tracking.start.y;
                let valid_direction =
                    distance >= -8.0 && (point.x - tracking.start.x).abs() <= distance.max(12.0);
                if nested_consumes || !valid_direction {
                    self.0 = None;
                    return false;
                }
                // Recheck the final contact against both the origin and the
                // previous sample; the last delivered move may be out of date.
                if matches!(sample.kind, TouchKind::End) {
                    self.0 = None;
                    return distance >= THRESHOLD;
                }
                self.0 = Some(Tracking {
                    start: tracking.start,
                    last_y: point.y,
                    distance: distance.max(0.0),
                });
            }
            TouchKind::Cancel => unreachable!("cancel is handled before reading a point"),
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(kind: TouchKind, y: f64) -> TouchSample {
        let touches = match kind {
            TouchKind::Start | TouchKind::Move => 1,
            TouchKind::End | TouchKind::Cancel => 0,
        };
        TouchSample {
            kind,
            point: Some(Point { x: 10.0, y }),
            at_top: true,
            on_control: false,
            has_selection: false,
            nested_above: false,
            nested_below: false,
            touches,
        }
    }

    fn armed() -> Gesture {
        let mut gesture = Gesture::default();
        gesture.update(sample(TouchKind::Start, 100.0), false);
        gesture.update(sample(TouchKind::Move, 200.0), false);
        assert_eq!(gesture.phase(), Phase::Armed);
        gesture
    }

    #[test]
    fn requires_threshold_and_release_and_can_disarm() {
        let mut gesture = Gesture::default();
        gesture.update(sample(TouchKind::Start, 100.0), false);
        for (y, phase) in [
            (108.0, Phase::Idle),
            (109.0, Phase::Pulling),
            (179.0, Phase::Pulling),
            (180.0, Phase::Armed),
            (150.0, Phase::Pulling),
        ] {
            assert!(!gesture.update(sample(TouchKind::Move, y), false));
            assert_eq!(gesture.phase(), phase);
        }
        assert!(!gesture.update(sample(TouchKind::End, 150.0), false));
        gesture.update(sample(TouchKind::Start, 100.0), false);
        gesture.update(sample(TouchKind::Move, 200.0), false);
        assert!(gesture.update(sample(TouchKind::End, 200.0), false));
        assert!(!gesture.update(sample(TouchKind::End, 200.0), false));
    }

    #[test]
    fn release_with_another_finger_still_down_does_not_refresh() {
        let mut gesture = armed();
        assert!(
            !gesture.update(TouchSample { touches: 1, ..sample(TouchKind::End, 200.0) }, false)
        );
        assert_eq!(gesture.phase(), Phase::Idle);
        assert!(!gesture.update(sample(TouchKind::End, 200.0), false));
    }

    #[test]
    fn release_uses_final_position_instead_of_previous_move() {
        for point in [Point { x: 10.0, y: 179.0 }, Point { x: 150.0, y: 200.0 }] {
            let mut gesture = armed();
            assert!(!gesture.update(
                TouchSample { point: Some(point), ..sample(TouchKind::End, 200.0) },
                false,
            ));
            assert_eq!(gesture.phase(), Phase::Idle);
        }
    }

    #[test]
    fn release_at_threshold_refreshes_once_and_cancel_never_refreshes() {
        let mut gesture = Gesture::default();
        gesture.update(sample(TouchKind::Start, 100.0), false);
        gesture.update(sample(TouchKind::Move, 179.0), false);
        assert_eq!(gesture.phase(), Phase::Pulling);
        assert!(gesture.update(sample(TouchKind::End, 180.0), false));
        assert!(!gesture.update(sample(TouchKind::End, 180.0), false));

        let mut gesture = armed();
        assert!(!gesture.update(sample(TouchKind::Cancel, 200.0), false));
        assert_eq!(gesture.phase(), Phase::Idle);
        assert!(!gesture.update(sample(TouchKind::End, 200.0), false));
    }

    #[test]
    fn each_exclusion_cancels_until_a_new_start() {
        let normal = sample(TouchKind::Move, 200.0);
        for (sample, refreshing) in [
            (TouchSample { at_top: false, ..normal }, false),
            (TouchSample { on_control: true, ..normal }, false),
            (TouchSample { has_selection: true, ..normal }, false),
            (TouchSample { touches: 2, ..normal }, false),
            (TouchSample { point: Some(Point { x: 220.0, y: 200.0 }), ..normal }, false),
            (normal, true),
        ] {
            let mut gesture = armed();
            assert!(!gesture.update(sample, refreshing));
            assert_eq!(gesture.phase(), Phase::Idle, "{sample:?}, refreshing={refreshing}");
            assert!(!gesture.update(normal, false));
            assert_eq!(gesture.phase(), Phase::Idle);
        }
    }

    #[test]
    fn start_has_no_direction_and_resets_previous_history() {
        let mut gesture = armed();
        gesture.update(
            TouchSample {
                nested_above: true,
                nested_below: true,
                ..sample(TouchKind::Start, 500.0)
            },
            false,
        );
        assert_eq!(gesture.phase(), Phase::Idle);
        // A sub-pixel first move must compare with the new start, not 200.
        gesture
            .update(TouchSample { nested_above: true, ..sample(TouchKind::Move, 500.25) }, false);
        assert!(gesture.update(sample(TouchKind::End, 600.0), false));
    }

    #[test]
    fn nested_capacity_blocks_only_the_direction_of_this_movement() {
        for (above, below, expected) in
            [(true, false, Phase::Idle), (false, true, Phase::Armed), (true, true, Phase::Idle)]
        {
            let mut gesture = Gesture::default();
            gesture.update(sample(TouchKind::Start, 100.0), false);
            gesture.update(
                TouchSample {
                    nested_above: above,
                    nested_below: below,
                    ..sample(TouchKind::Move, 200.0)
                },
                false,
            );
            assert_eq!(gesture.phase(), expected);
        }
    }

    #[test]
    fn nested_reversal_cancels_instead_of_using_total_downward_distance() {
        for (below, expected) in [(false, Phase::Pulling), (true, Phase::Idle)] {
            let mut gesture = armed();
            gesture.update(
                TouchSample { nested_below: below, ..sample(TouchKind::Move, 170.0) },
                false,
            );
            assert_eq!(gesture.phase(), expected);
            if below {
                gesture.update(sample(TouchKind::Move, 220.0), false);
                assert_eq!(gesture.phase(), Phase::Idle);
                assert!(!gesture.update(sample(TouchKind::End, 220.0), false));
            }
        }
    }

    #[test]
    fn tiny_movement_skips_nested_consumption_but_half_pixel_does_not() {
        for delta in [0.0, 0.49, -0.49, 0.5, -0.5] {
            let mut gesture = armed();
            gesture.update(
                TouchSample {
                    nested_above: true,
                    nested_below: true,
                    ..sample(TouchKind::Move, 200.0 + delta)
                },
                false,
            );
            assert_eq!(gesture.phase(), if delta.abs() < 0.5 { Phase::Armed } else { Phase::Idle });
        }
    }

    #[test]
    fn release_rechecks_nested_movement_since_the_last_sample() {
        for below in [false, true] {
            let mut gesture = armed();
            assert_eq!(
                gesture.update(
                    TouchSample { nested_below: below, ..sample(TouchKind::End, 199.0) },
                    false,
                ),
                !below,
            );
            assert_eq!(gesture.phase(), Phase::Idle);
        }
    }

    #[test]
    fn missing_point_always_cancels_and_never_refreshes() {
        for kind in [TouchKind::Start, TouchKind::Move, TouchKind::End, TouchKind::Cancel] {
            let mut gesture = armed();
            assert!(!gesture.update(TouchSample { point: None, ..sample(kind, 200.0) }, false));
            assert_eq!(gesture.phase(), Phase::Idle);
            assert!(!gesture.update(sample(TouchKind::End, 200.0), false));
        }
    }

    #[test]
    fn invalid_start_never_creates_a_gesture() {
        for invalid in [
            TouchSample { on_control: true, ..sample(TouchKind::Start, 100.0) },
            TouchSample { has_selection: true, ..sample(TouchKind::Start, 100.0) },
            TouchSample { at_top: false, ..sample(TouchKind::Start, 100.0) },
            TouchSample { touches: 2, ..sample(TouchKind::Start, 100.0) },
        ] {
            let mut gesture = Gesture::default();
            gesture.update(invalid, false);
            gesture.update(sample(TouchKind::Move, 200.0), false);
            assert_eq!(gesture.phase(), Phase::Idle);
            assert!(!gesture.update(sample(TouchKind::End, 200.0), false));
        }
    }

    #[test]
    fn wire_facts_decode_points_and_contactless_cancellation() {
        for kind in ["start", "move", "end", "cancel"] {
            for point in [serde_json::json!({"x": 10.0, "y": 200.0}), serde_json::Value::Null] {
                let facts: TouchSample = serde_json::from_value(serde_json::json!({
                    "kind": kind, "point": point, "at_top": true,
                    "on_control": false, "has_selection": false,
                    "nested_above": false, "nested_below": true,
                    "touches": if kind == "start" || kind == "move" { 1 } else { 0 },
                }))
                .unwrap();
                assert_eq!(facts.point.is_some(), !point.is_null());
                if point.is_null() {
                    let mut gesture = armed();
                    assert!(!gesture.update(facts, false));
                    assert_eq!(gesture.phase(), Phase::Idle);
                }
            }
        }
    }
}

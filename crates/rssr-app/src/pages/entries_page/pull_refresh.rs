use std::{cell::Cell, rc::Rc};

use dioxus::prelude::*;
use serde::Deserialize;

use crate::ui::AppShellState;

const THRESHOLD: f64 = 80.0;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum TouchKind {
    Start,
    Move,
    End,
    Cancel,
}

/// DOM facts only; eligibility, direction and threshold policy are resolved below.
#[derive(Debug, Clone, Copy, Deserialize)]
struct TouchSample {
    kind: TouchKind,
    x: f64,
    y: f64,
    at_top: bool,
    eligible: bool,
    touches: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
enum PullState {
    #[default]
    Idle,
    Tracking {
        x: f64,
        y: f64,
        distance: f64,
    },
}

impl PullState {
    fn phase(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Tracking { distance, .. } if distance >= THRESHOLD => "armed",
            Self::Tracking { distance, .. } if distance > 8.0 => "pulling",
            Self::Tracking { .. } => "idle",
        }
    }

    fn update(&mut self, sample: TouchSample, refreshing: bool) -> bool {
        if refreshing || !sample.at_top || !sample.eligible || sample.touches > 1 {
            *self = Self::Idle;
            return false;
        }
        match sample.kind {
            TouchKind::Start => *self = Self::Tracking { x: sample.x, y: sample.y, distance: 0.0 },
            TouchKind::Move => {
                if let Self::Tracking { x, y, .. } = *self {
                    let distance = sample.y - y;
                    if distance < -8.0 || (sample.x - x).abs() > distance.max(12.0) {
                        *self = Self::Idle;
                    } else {
                        *self = Self::Tracking { x, y, distance: distance.max(0.0) };
                    }
                }
            }
            TouchKind::End => {
                let armed =
                    matches!(*self, Self::Tracking { distance, .. } if distance >= THRESHOLD);
                *self = Self::Idle;
                return armed;
            }
            TouchKind::Cancel => *self = Self::Idle,
        }
        false
    }
}

#[component]
pub(super) fn PullRefresh() -> Element {
    let shell = use_context::<AppShellState>();
    let mut state = use_signal(PullState::default);
    let bridge = use_hook(|| Rc::new(Cell::new(None::<document::Eval>)));
    let cleanup = Rc::clone(&bridge);
    use_drop(move || {
        if let Some(eval) = cleanup.get() {
            let _ = eval.send(());
        }
    });
    use_future(move || {
        let bridge = Rc::clone(&bridge);
        async move {
            let mut eval = document::eval(include_str!("pull_refresh.js"));
            bridge.set(Some(eval));
            while let Ok(sample) = eval.recv::<TouchSample>().await {
                let mut next = *state.peek();
                let refresh = next.update(sample, shell.refresh_state().is_refreshing());
                if next != *state.peek() {
                    state.set(next);
                }
                if refresh {
                    shell.manual_refresh();
                }
            }
        }
    });
    let phase = state.read().phase();
    rsx! {
        output { "data-slot": "pull-refresh", "data-state": phase, role: "status", aria_live: "polite",
            if phase == "armed" { "松开刷新" }
            else if phase == "pulling" { "继续下拉刷新" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample(kind: TouchKind, y: f64) -> TouchSample {
        TouchSample { kind, x: 10.0, y, at_top: true, eligible: true, touches: 1 }
    }

    #[test]
    fn requires_top_threshold_and_release_and_can_disarm() {
        let mut state = PullState::default();
        state.update(sample(TouchKind::Start, 100.0), false);
        state.update(sample(TouchKind::Move, 179.0), false);
        assert_eq!(state.phase(), "pulling");
        state.update(sample(TouchKind::Move, 180.0), false);
        assert_eq!(state.phase(), "armed");
        state.update(sample(TouchKind::Move, 150.0), false);
        assert_eq!(state.phase(), "pulling");
        assert!(!state.update(sample(TouchKind::End, 150.0), false));
        state.update(sample(TouchKind::Start, 100.0), false);
        state.update(sample(TouchKind::Move, 200.0), false);
        assert!(state.update(sample(TouchKind::End, 200.0), false));
        assert!(!state.update(sample(TouchKind::End, 200.0), false));
    }

    #[test]
    fn rejects_scroll_multitouch_horizontal_selection_and_inflight() {
        for (at_top, eligible, touches, x, refreshing) in [
            (false, true, 1, 10.0, false),
            (true, false, 1, 10.0, false),
            (true, true, 2, 10.0, false),
            (true, true, 1, 220.0, false),
            (true, true, 1, 10.0, true),
        ] {
            let mut state = PullState::default();
            state.update(sample(TouchKind::Start, 100.0), false);
            state.update(
                TouchSample { at_top, eligible, touches, x, ..sample(TouchKind::Move, 200.0) },
                refreshing,
            );
            assert!(!state.update(sample(TouchKind::End, 200.0), false));
        }
    }
}

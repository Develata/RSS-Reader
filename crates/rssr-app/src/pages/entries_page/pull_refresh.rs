use std::{cell::Cell, rc::Rc};

use dioxus::prelude::*;

use crate::ui::AppShellState;

mod gesture;
use gesture::{Gesture, Phase, TouchSample};

#[component]
pub(super) fn PullRefresh() -> Element {
    let shell = use_context::<AppShellState>();
    let mut phase = use_signal(Phase::default);
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
            // Coordinate changes stay local; only a new visible phase updates UI.
            let mut gesture = Gesture::default();
            while let Ok(sample) = eval.recv::<TouchSample>().await {
                let refresh = gesture.update(sample, shell.refresh_state().is_refreshing());
                let next = gesture.phase();
                if next != *phase.peek() {
                    phase.set(next);
                }
                if refresh {
                    shell.manual_refresh();
                }
            }
        }
    });
    let phase = *phase.read();
    let state = match phase {
        Phase::Idle => "idle",
        Phase::Pulling => "pulling",
        Phase::Armed => "armed",
    };
    rsx! {
        output { "data-slot": "pull-refresh", "data-state": state, role: "status", aria_live: "polite",
            if phase == Phase::Armed { "松开刷新" }
            else if phase == Phase::Pulling { "继续下拉刷新" }
        }
    }
}

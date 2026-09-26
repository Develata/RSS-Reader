//! 仅本次运行的阅读位置。DOM bridge 提供测量，保存与恢复策略在 Rust 中执行。
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct Point {
    y: f64,
    anchor: Option<String>,
    offset: f64,
}
#[derive(Clone, Debug)]
struct Saved {
    page: u32,
    point: Point,
}
thread_local! {
    static POSITIONS: RefCell<HashMap<String, Saved>> = RefCell::default();
}
pub(crate) fn remembered_page(key: &str) -> Option<u32> {
    POSITIONS.with_borrow(|positions| positions.get(key).map(|saved| saved.page))
}
#[derive(Deserialize)]
struct Fact {
    kind: String,
    visit: u64,
    sequence: u64,
    key: String,
    page: u32,
    now: f64,
    point: Point,
    max_y: f64,
    anchor_top: Option<f64>,
}
#[derive(Serialize)]
struct Command {
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
            && !matches!(fact.kind.as_str(), "input" | "capture" | "context")
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

fn save_capture(fact: Fact) {
    if fact.kind != "capture" {
        return;
    }
    let mut point = fact.point;
    if point.y <= 0.0 {
        point.anchor = None;
        point.offset = 0.0;
    }
    POSITIONS.with_borrow_mut(|positions| {
        positions.insert(fact.key, Saved { page: fact.page, point });
    });
}

// Android 系统返回键不经过 DOM 点击；先取得旧正文位置，再让宿主执行导航。
#[cfg(target_os = "android")]
pub(crate) async fn capture_current_position() {
    let eval = document::eval(
        "const event = new CustomEvent('rssr-capture-position', {detail:{fact:null}});\n\
         document.dispatchEvent(event); return event.detail.fact;",
    );
    match tokio::time::timeout(std::time::Duration::from_millis(500), eval).await {
        Ok(Ok(value)) => {
            if let Ok(Some(fact)) = serde_json::from_value::<Option<Fact>>(value) {
                save_capture(fact);
            }
        }
        result => tracing::warn!(?result, "返回前读取位置失败，继续导航"),
    }
}

pub(crate) fn use_reading_positions() {
    let bridge = use_hook(|| Rc::new(Cell::new(None::<document::Eval>)));
    let cleanup = bridge.clone();
    use_drop(move || {
        if let Some(eval) = cleanup.get() {
            let _ = eval.send(serde_json::Value::Null);
        }
        POSITIONS.with_borrow_mut(HashMap::clear);
    });
    use_future(move || {
        let bridge = bridge.clone();
        async move {
            let mut eval = document::eval(include_str!("reading_position.js"));
            bridge.set(Some(eval));
            let mut restoration: Option<Restoration> = None;
            let mut latest_visit = 0;
            while let Ok(fact) = eval.recv::<Fact>().await {
                if fact.visit < latest_visit {
                    continue;
                }
                latest_visit = fact.visit;
                if fact.kind == "visit" {
                    let saved =
                        POSITIONS.with_borrow(|positions| positions.get(&fact.key).cloned());
                    let point = saved.filter(|saved| saved.page == fact.page).map(|s| s.point);
                    // 新文章从顶部开始；列表首次进入仍沿用既有翻页/筛选行为。
                    let point =
                        point.or_else(|| fact.key.starts_with("reader:").then(Point::default));
                    restoration = point.map(|point| Restoration {
                        visit: fact.visit,
                        started: fact.now,
                        point,
                    });
                }
                if restoration.as_ref().is_some_and(|active| !active.is_active(&fact)) {
                    restoration = None;
                }
                let mut command = Command {
                    visit: fact.visit,
                    sequence: fact.sequence,
                    anchor: None,
                    y: None,
                    watch: false,
                    highlight: None,
                };
                if let Some(active) = &restoration {
                    command.anchor = active.point.anchor.clone();
                    command.watch = true;
                    if fact.key.starts_with("list:") {
                        command.highlight = active.point.anchor.clone();
                    }
                    if fact.kind != "visit" {
                        command.y = Some(target_y(
                            &active.point,
                            fact.point.y,
                            fact.anchor_top,
                            fact.max_y,
                        ));
                    }
                } else if fact.kind == "capture" {
                    save_capture(fact);
                }
                if eval.send(command).is_err() {
                    break;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_operation_capture_saves_position_and_top_clears_anchor() {
        POSITIONS.with_borrow_mut(HashMap::clear);
        let fact = |kind: &str, y| Fact {
            kind: kind.into(),
            visit: 1,
            sequence: 1,
            key: "reader:1".into(),
            page: 1,
            now: 0.0,
            point: Point { y, anchor: Some("block:2:P:text".into()), offset: 120.0 },
            max_y: 1000.0,
            anchor_top: None,
        };
        save_capture(fact("capture", 600.0));
        for kind in ["input", "layout", "visit", "context", "scroll"] {
            save_capture(fact(kind, 0.0));
        }
        POSITIONS.with_borrow(|positions| {
            assert_eq!(positions["reader:1"].point.y, 600.0);
        });
        save_capture(fact("capture", 0.0));
        POSITIONS.with_borrow(|positions| {
            assert_eq!(positions["reader:1"].point.y, 0.0);
            assert!(positions["reader:1"].point.anchor.is_none());
        });
        POSITIONS.with_borrow_mut(HashMap::clear);
    }
    #[test]
    fn restoration_stops_on_input_navigation_context_change_and_deadline() {
        let active = Restoration { visit: 2, started: 100.0, point: Point::default() };
        let mut fact = Fact {
            kind: "layout".into(),
            visit: 2,
            sequence: 1,
            key: "reader:1".into(),
            page: 1,
            now: 200.0,
            point: Point::default(),
            max_y: 1000.0,
            anchor_top: None,
        };
        assert!(active.is_active(&fact));
        for kind in ["input", "capture", "context"] {
            fact.kind = kind.into();
            assert!(!active.is_active(&fact));
        }
        fact.kind = "layout".into();
        fact.visit = 3;
        assert!(!active.is_active(&fact));
        fact.visit = 2;
        fact.now = 2100.0;
        assert!(!active.is_active(&fact));
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

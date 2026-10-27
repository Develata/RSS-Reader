//! 仅本次运行的阅读位置。策略由纯 Rust 控制器管理，当前模块负责宿主桥接与生命周期。
use dioxus::prelude::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

mod controller;
use controller::{Fact, Restorer, SavedPositions};

// Only saved positions are shared with page initialization and Android back.
// Restoration/visit state belongs to one adapter lifetime inside the future.
thread_local! {
    static POSITIONS: RefCell<SavedPositions> = RefCell::default();
}

pub(crate) fn remembered_page(key: &str) -> Option<u32> {
    POSITIONS.with_borrow(|positions| positions.remembered_page(key))
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
                POSITIONS.with_borrow_mut(|positions| positions.capture(fact));
            }
        }
        result => tracing::warn!(?result, "返回前读取位置失败，继续导航"),
    }
}

pub(crate) fn use_reading_positions() {
    let bridge = use_hook(|| Rc::new(Cell::new(None::<document::Eval>)));
    let cleanup = Rc::clone(&bridge);
    use_drop(move || {
        if let Some(eval) = cleanup.get() {
            let _ = eval.send(serde_json::Value::Null);
        }
        POSITIONS.with_borrow_mut(SavedPositions::clear);
    });
    use_future(move || {
        let bridge = Rc::clone(&bridge);
        async move {
            let mut eval = document::eval(include_str!("reading_position.js"));
            bridge.set(Some(eval));
            let mut restorer = Restorer::default();
            while let Ok(fact) = eval.recv::<Fact>().await {
                let command =
                    POSITIONS.with_borrow_mut(|positions| restorer.on_fact(positions, fact));
                if let Some(command) = command
                    && eval.send(command).is_err()
                {
                    break;
                }
            }
        }
    });
}

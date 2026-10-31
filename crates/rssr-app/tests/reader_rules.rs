//! Compile the production rules together without the app's Dioxus assembly.
//! `scripts/check_reader_rule_boundary.py` also builds this exact harness with rustc and
//! the existing API/domain/infra, serde and url crates; no substitute state, rules or cleaner.
#![allow(dead_code)]

#[path = "../src/ui/failure.rs"]
pub(crate) mod failure;
mod ui {
    pub(crate) use crate::failure;
}

#[path = "../src/pages/reader_page/image_viewer_state.rs"]
mod image_viewer_state;
#[path = "../src/pages/reader_page/intent.rs"]
pub(crate) mod intent;
#[path = "../src/ui/runtime/reader/body.rs"]
mod reader_body;
#[path = "../src/ui/reader_shortcuts/policy.rs"]
mod reader_shortcut_policy;
#[path = "../src/pages/reader_page/reducer.rs"]
pub(crate) mod reducer;
#[path = "../src/pages/reader_page/result_lifecycle.rs"]
mod result_lifecycle;
#[path = "../src/pages/reader_page/state.rs"]
pub(crate) mod state;

// Preserve the paths used by the existing production-module tests, not their types.
mod pages {
    pub(crate) mod reader_page {
        pub(crate) use crate::{reducer, state};
    }
}

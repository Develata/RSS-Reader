//! Synchronous WebView input shared by the Reader and reading-position adapters.
//! This module builds script text; listener ownership stays with each caller.

// Each bridge gets the same synchronous predicate and a projection of the Rust
// bindings. Keep them local to Eval: no global registration or readiness race.
pub(crate) fn script(body: &str) -> String {
    let bindings = super::policy::BINDINGS
        .map(|(key, action)| (key.to_ascii_lowercase(), action.captures_position()))
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
    format!(
        "const readerShortcutBindings = {};\n{}\n{body}",
        serde_json::json!(bindings),
        include_str!("shortcut_input.js"),
    )
}

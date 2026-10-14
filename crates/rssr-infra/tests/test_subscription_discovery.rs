#![cfg(not(target_arch = "wasm32"))]
#[path = "support/discovery_cases.rs"]
mod cases;

#[test]
fn parser_contract() {
    cases::assert_discovery_cases();
}

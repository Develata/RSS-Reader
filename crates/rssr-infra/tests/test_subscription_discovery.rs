#![cfg(not(target_arch = "wasm32"))]
#[path = "support/discovery_cases.rs"]
mod cases;

#[test]
fn parser_contract() {
    cases::assert_discovery_cases();
}

#[test]
fn discovery_is_bounded_while_the_page_is_read() {
    cases::assert_discovery_bounds();
}

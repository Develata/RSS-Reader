#![cfg(not(target_arch = "wasm32"))]
#[path = "support/discovery_cases.rs"]
mod cases;

#[test]
fn parser_contract() {
    cases::assert_discovery_cases();
}

#[test]
fn discovery_is_bounded_while_the_page_is_read() {
    // Without the bound on a single tag, its 100 000 attributes alone take minutes.
    let started = std::time::Instant::now();
    cases::assert_discovery_bounds();
    assert!(started.elapsed() < std::time::Duration::from_secs(10), "{:?}", started.elapsed());
}

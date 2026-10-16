#![cfg(not(target_arch = "wasm32"))]
#[path = "support/discovery_cases.rs"]
mod cases;

#[test]
fn parser_contract() {
    cases::assert_discovery_cases();
}

#[test]
fn discovery_is_bounded_while_the_page_is_read() {
    // The bounds are asserted by what is found. This is a backstop for work without a bound:
    // a tag of 100 000 attributes alone takes tens of minutes in a debug build, while the bounded
    // cases take about 7 seconds there (up to a minute on a loaded machine) and 4 in a release
    // build.
    let started = std::time::Instant::now();
    cases::assert_discovery_bounds();
    assert!(started.elapsed() < std::time::Duration::from_secs(300), "{:?}", started.elapsed());
}

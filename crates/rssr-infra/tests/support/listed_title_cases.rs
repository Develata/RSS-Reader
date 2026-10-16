//! Titles in lists, the same on every store (review C3-R1 F04).

use rssr_domain::MAX_LISTED_TITLE_CHARS;

/// A subscription title twice over the bound.
pub fn long_feed_title() -> String {
    "源".repeat(2 * MAX_LISTED_TITLE_CHARS)
}

/// An entry title one character over the bound.
pub fn long_entry_title() -> String {
    format!("{}x", "题".repeat(MAX_LISTED_TITLE_CHARS))
}

/// An entry title at the bound: kept whole.
pub fn bound_entry_title() -> String {
    "字".repeat(MAX_LISTED_TITLE_CHARS)
}

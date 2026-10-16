//! Size bounds of the results the user interface receives (v0.2 phase C, C3). A background job's
//! result is kept by the server and sent over HTTP, so its size is bounded where it is built,
//! never by cutting a finished result afterwards. The bounds are the same on every platform.
//!
//! - variable-length text is cut to its bound and marked ([`TRUNCATION_MARK`]);
//! - a refresh of all subscriptions lists at most [`MAX_LISTED_FAILURES`] failures, and the whole
//!   result serializes to at most [`MAX_RESULT_BYTES`]; failures that do not fit are counted
//!   (`failed_count`) but not listed (`failures_truncated`);
//! - subscription discovery offers at most [`MAX_DISCOVERY_CANDIDATES`] feeds.

use url::Url;

use crate::{
    AddSubscriptionOutcome, FeedDiscoveryCandidate, RefreshAllExecutionOutcome, RefreshFailureItem,
};

/// Failures a refresh result lists at most.
pub const MAX_LISTED_FAILURES: usize = 200;
/// A subscription's display name in a result.
pub const MAX_NAME_BYTES: usize = 256;
/// An address in a result.
pub const MAX_URL_BYTES: usize = 2048;
/// A failure's message.
pub const MAX_MESSAGE_BYTES: usize = 1024;
/// A whole result, serialized as JSON.
pub const MAX_RESULT_BYTES: usize = 256 * 1024;
/// Feeds a discovered page offers at most.
pub const MAX_DISCOVERY_CANDIDATES: usize = 50;
/// Appended where text was cut.
pub const TRUNCATION_MARK: &str = "…（已截断）";

/// `text` cut to at most `max` bytes (the mark included) at a character boundary.
///
/// Precondition: `max >= TRUNCATION_MARK.len()`. Postcondition: the result is `text` when it fits,
/// otherwise a prefix of `text` followed by the mark, and never longer than `max`.
pub fn truncate_marked(text: String, max: usize) -> String {
    if text.len() <= max {
        return text;
    }
    let mut keep = max.saturating_sub(TRUNCATION_MARK.len());
    while !text.is_char_boundary(keep) {
        keep -= 1;
    }
    format!("{}{TRUNCATION_MARK}", &text[..keep])
}

fn serialized_len(value: &impl serde::Serialize) -> usize {
    serde_json::to_vec(value).map_or(usize::MAX, |bytes| bytes.len())
}

impl RefreshFailureItem {
    /// This failure with its name, address and message cut to their bounds.
    pub fn bounded(self) -> Self {
        Self {
            url: truncate_marked(self.url, MAX_URL_BYTES),
            name: self.name.map(|name| truncate_marked(name, MAX_NAME_BYTES)),
            message: truncate_marked(self.message, MAX_MESSAGE_BYTES),
            ..self
        }
    }
}

impl RefreshAllExecutionOutcome {
    /// The result of refreshing every subscription, built within its bounds: each failure is
    /// bounded, at most [`MAX_LISTED_FAILURES`] are listed, and failures are listed only while the
    /// serialized result stays within [`MAX_RESULT_BYTES`] (measured item by item as they are
    /// added). `failed_count` is always the full count; `failures_truncated` says some are not
    /// listed: fewer are listed than `failed_count` (the caller may pass only the first ones).
    /// In the smallest form nothing is listed: the counts remain.
    pub fn bounded(
        inserted_count: u64,
        total_count: u64,
        failed_count: u64,
        failures: Vec<RefreshFailureItem>,
    ) -> Self {
        let mut outcome = Self {
            inserted_count,
            total_count,
            failed_count,
            failures: Vec::new(),
            // Measured as `true`, the longer of the two.
            failures_truncated: true,
        };
        let mut size = serialized_len(&outcome);
        for failure in failures {
            if outcome.failures.len() == MAX_LISTED_FAILURES {
                break;
            }
            let failure = failure.bounded();
            // The item and the comma before it.
            let item = serialized_len(&failure) + 1;
            if size.saturating_add(item) > MAX_RESULT_BYTES {
                break;
            }
            size += item;
            outcome.failures.push(failure);
        }
        outcome.failures_truncated = (outcome.failures.len() as u64) < failed_count;
        outcome
    }
}

impl AddSubscriptionOutcome {
    /// A page that lists several feeds, offering at most [`MAX_DISCOVERY_CANDIDATES`]: titles are
    /// cut to their bound, and an address longer than [`MAX_URL_BYTES`] is left out (an address
    /// cannot be cut and still lead to the feed).
    pub fn needs_selection(page_url: Url, candidates: Vec<FeedDiscoveryCandidate>) -> Self {
        let candidates = candidates
            .into_iter()
            .filter(|candidate| candidate.url.as_str().len() <= MAX_URL_BYTES)
            .take(MAX_DISCOVERY_CANDIDATES)
            .map(|candidate| FeedDiscoveryCandidate {
                title: candidate.title.map(|title| truncate_marked(title, MAX_NAME_BYTES)),
                ..candidate
            })
            .collect();
        Self::NeedsSelection { page_url, candidates }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorKind;

    fn failure(feed_id: i64, filler: char, bytes: usize) -> RefreshFailureItem {
        RefreshFailureItem {
            feed_id,
            url: format!("https://example.com/{}", filler.to_string().repeat(bytes)),
            name: Some(filler.to_string().repeat(bytes)),
            kind: ErrorKind::Network,
            message: filler.to_string().repeat(bytes),
        }
    }

    #[test]
    fn text_is_cut_at_a_character_boundary_and_marked() {
        assert_eq!(truncate_marked("short".into(), 64), "short");
        let cut = truncate_marked("订阅".repeat(100), 64);
        assert!(cut.len() <= 64 && cut.ends_with(TRUNCATION_MARK), "{cut}");
        assert!(cut.trim_end_matches(TRUNCATION_MARK).chars().all(|c| "订阅".contains(c)));
        let exact = "a".repeat(64);
        assert_eq!(truncate_marked(exact.clone(), 64), exact);
    }

    #[test]
    fn a_failure_is_cut_field_by_field() {
        let bounded = failure(1, 'x', 10_000).bounded();
        assert!(bounded.url.len() <= MAX_URL_BYTES);
        assert!(bounded.name.as_ref().unwrap().len() <= MAX_NAME_BYTES);
        assert!(bounded.message.len() <= MAX_MESSAGE_BYTES);
        assert!(bounded.message.ends_with(TRUNCATION_MARK));
        let small = failure(2, 'y', 10);
        assert_eq!(small.clone().bounded(), small, "what fits is unchanged");
    }

    /// The test the plan asks for: a result far over the bound shrinks to it, counts intact.
    #[test]
    fn an_oversized_refresh_result_shrinks_to_its_bound() {
        // Control characters serialize six times longer (\u0001): measuring item by item counts
        // the escaped form, not the raw bytes.
        let failures: Vec<_> = (0..1000).map(|id| failure(id, '\u{1}', 5_000)).collect();
        let outcome = RefreshAllExecutionOutcome::bounded(7, 1200, 1000, failures);
        let size = serde_json::to_vec(&outcome).unwrap().len();
        assert!(size <= MAX_RESULT_BYTES, "{size}");
        assert!(outcome.failures_truncated);
        assert!(!outcome.failures.is_empty() && outcome.failures.len() < MAX_LISTED_FAILURES);
        assert_eq!(
            (outcome.inserted_count, outcome.total_count, outcome.failed_count),
            (7, 1200, 1000)
        );

        let many: Vec<_> = (0..500).map(|id| failure(id, 'z', 5)).collect();
        let listed = RefreshAllExecutionOutcome::bounded(0, 500, 500, many);
        assert_eq!(listed.failures.len(), MAX_LISTED_FAILURES);
        assert!(listed.failures_truncated);

        let few: Vec<_> = (0..3).map(|id| failure(id, 'z', 5)).collect();
        let whole = RefreshAllExecutionOutcome::bounded(1, 3, 3, few.clone());
        assert_eq!((whole.failures, whole.failures_truncated), (few.clone(), false));
        // Passed fewer than failed (the caller trimmed them first): still marked.
        let trimmed = RefreshAllExecutionOutcome::bounded(1, 9, 9, few);
        assert!(trimmed.failures_truncated);
    }

    #[test]
    fn discovery_offers_a_bounded_list() {
        let page = Url::parse("https://example.com/").unwrap();
        let long_url = Url::parse(&format!("https://example.com/{}", "a".repeat(3000))).unwrap();
        let mut candidates = vec![FeedDiscoveryCandidate { url: long_url, title: None }];
        candidates.extend((0..80).map(|index| FeedDiscoveryCandidate {
            url: Url::parse(&format!("https://example.com/{index}.xml")).unwrap(),
            title: Some("题".repeat(500)),
        }));
        let AddSubscriptionOutcome::NeedsSelection { candidates, .. } =
            AddSubscriptionOutcome::needs_selection(page, candidates)
        else {
            unreachable!()
        };
        assert_eq!(candidates.len(), MAX_DISCOVERY_CANDIDATES);
        assert!(
            candidates[0].url.as_str().ends_with("/0.xml"),
            "the over-long address is left out"
        );
        assert!(candidates.iter().all(|c| c.title.as_ref().unwrap().len() <= MAX_NAME_BYTES));
    }
}

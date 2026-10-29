//! A digest of the configuration a replacement overwrites, so the replacement can refuse when
//! the configuration changed after it was read (`docs/design/v0.2-phase-b2-contracts.md`, D4).
//!
//! The digest covers the raw configuration, not what the pages show: each active subscription's
//! id, normalized address, generation, custom name (never the source title) and group, plus the
//! settings. Both stores build the same projection from their own fields and hash it here, so the
//! definition exists once.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::UserSettings;

/// One active subscription as the fingerprint sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FingerprintFeed<'a> {
    pub id: i64,
    /// The stored, normalized address.
    pub url: &'a str,
    /// Raised each time the subscription is reactivated; a removed and re-added subscription
    /// is a different configuration even with the same id, address and names.
    pub generation: i64,
    /// The name the user or an import gave it; `None` when it has none (the source title does
    /// not count: a refresh that changes it changes no configuration).
    pub custom_title: Option<&'a str>,
    pub folder: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConfigFingerprint(String);

#[derive(Serialize)]
struct Projection<'a> {
    feeds: &'a [FingerprintFeed<'a>],
    settings: &'a UserSettings,
}

impl ConfigFingerprint {
    /// The fingerprint of the active subscriptions `feeds` (any order) and `settings`.
    ///
    /// Canonical form: subscriptions sorted by id, every field written (`None` as `null`, so it
    /// differs from an empty name), serialized as JSON, then SHA-256 in lower-case hex.
    pub fn compute(mut feeds: Vec<FingerprintFeed<'_>>, settings: &UserSettings) -> Self {
        feeds.sort_unstable_by_key(|feed| feed.id);
        let projection = Projection { feeds: &feeds, settings };
        // Plain data with string keys: serializing it cannot fail.
        let encoded = serde_json::to_vec(&projection).expect("serialize configuration projection");
        let digest = Sha256::digest(&encoded);
        Self(digest.iter().map(|byte| format!("{byte:02x}")).collect())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::{ConfigFingerprint, FingerprintFeed};
    use crate::{ThemeMode, UserSettings};

    fn feed(id: i64, custom_title: Option<&'static str>) -> FingerprintFeed<'static> {
        FingerprintFeed {
            id,
            url: "https://example.com/feed.xml",
            generation: 0,
            custom_title,
            folder: None,
        }
    }

    #[test]
    fn the_order_of_subscriptions_does_not_matter() {
        let settings = UserSettings::default();
        assert_eq!(
            ConfigFingerprint::compute(vec![feed(1, None), feed(2, None)], &settings),
            ConfigFingerprint::compute(vec![feed(2, None), feed(1, None)], &settings)
        );
    }

    #[test]
    fn every_field_counts_and_no_name_differs_from_an_empty_one() {
        let settings = UserSettings::default();
        let base = ConfigFingerprint::compute(vec![feed(1, None)], &settings);
        let variants = [
            vec![feed(1, Some(""))],
            vec![FingerprintFeed { generation: 1, ..feed(1, None) }],
            vec![FingerprintFeed { url: "https://example.com/other.xml", ..feed(1, None) }],
            vec![FingerprintFeed { folder: Some("Tech"), ..feed(1, None) }],
            vec![feed(3, None)],
            vec![feed(1, None), feed(2, None)],
            vec![],
        ];
        for feeds in variants {
            assert_ne!(ConfigFingerprint::compute(feeds.clone(), &settings), base, "{feeds:?}");
        }
        let dark = UserSettings { theme: ThemeMode::Dark, ..UserSettings::default() };
        assert_ne!(ConfigFingerprint::compute(vec![feed(1, None)], &dark), base);
    }

    #[test]
    fn the_fingerprint_is_lower_case_hex_sha256() {
        let fingerprint = ConfigFingerprint::compute(Vec::new(), &UserSettings::default());
        assert_eq!(fingerprint.as_str().len(), 64);
        assert!(fingerprint.as_str().chars().all(|c| c.is_ascii_digit() || c.is_ascii_lowercase()));
    }
}

//! Source-body identity shared by refresh candidates and storage adapters.
//! This byte format is persisted in existing libraries; changing it requires a
//! compatibility decision, not a host-specific alternative hash implementation.
use sha2::{Digest, Sha256};

pub fn entry_content_hash(
    html: Option<&str>,
    text: Option<&str>,
    title: Option<&str>,
    url: Option<&str>,
) -> Option<String> {
    let parts = [title, text, html, url];
    if parts.iter().all(Option::is_none) {
        return None;
    }
    let mut hasher = Sha256::new();
    hasher.update(b"rssr-content-v2");
    for part in parts {
        match part {
            Some(part) => {
                hasher.update([1]);
                hasher.update((part.len() as u64).to_le_bytes());
                hasher.update(part.as_bytes());
            }
            None => hasher.update([0]),
        }
    }
    Some(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::entry_content_hash;

    #[test]
    fn persisted_v2_identity_keeps_its_byte_format() {
        assert_eq!(
            entry_content_hash(
                Some("<p>source</p>"),
                None,
                Some("Title"),
                Some("https://example.com/article"),
            )
            .as_deref(),
            Some("675aba15e90cfe9e2cf24b53b04a1bdfb049101f971c1f54aacabacfdc36267c")
        );
        assert_eq!(entry_content_hash(None, None, None, None), None);
        assert_ne!(
            entry_content_hash(None, None, None, None),
            entry_content_hash(Some(""), None, None, None)
        );
    }
}

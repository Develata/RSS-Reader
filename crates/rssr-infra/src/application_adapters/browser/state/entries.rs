use rssr_domain::{Entry, EntryContent, EntryRecord};
use url::Url;

use crate::application_adapters::browser::{
    feed::{ParsedEntry, hash_content},
    now_utc,
};

use super::{
    BrowserState, PersistedEntryContent, PersistedEntryContentSlice, PersistedEntryFlag,
    PersistedEntryIndex,
};

pub fn entry_flags(state: &BrowserState, entry_id: i64) -> Option<&PersistedEntryFlag> {
    state.entry_flags.entries.iter().find(|flag| flag.id == entry_id)
}

pub fn entry_content(state: &BrowserState, entry_id: i64) -> Option<&PersistedEntryContent> {
    state.entry_content.entries.iter().find(|content| content.entry_id == entry_id)
}

pub fn to_domain_entry(state: &BrowserState, entry: &PersistedEntryIndex) -> anyhow::Result<Entry> {
    Ok(to_domain_entry_record(state, entry)?.into_entry(to_domain_content(state, entry.id)?))
}

pub fn to_domain_entry_record(
    state: &BrowserState,
    entry: &PersistedEntryIndex,
) -> anyhow::Result<EntryRecord> {
    let flags = entry_flags(state, entry.id);
    Ok(EntryRecord {
        id: entry.id,
        feed_id: entry.feed_id,
        external_id: entry.external_id.clone(),
        dedup_key: entry.dedup_key.clone(),
        url: entry.url.as_ref().map(|raw| Url::parse(raw)).transpose()?,
        title: entry.title.clone(),
        author: entry.author.clone(),
        summary: entry.summary.clone(),
        published_at: entry.published_at,
        updated_at_source: entry.updated_at_source,
        first_seen_at: entry.first_seen_at,
        has_content: entry.has_content,
        is_read: flags.map(|flag| flag.is_read).unwrap_or(false),
        is_starred: flags.map(|flag| flag.is_starred).unwrap_or(false),
        read_at: flags.and_then(|flag| flag.read_at),
        starred_at: flags.and_then(|flag| flag.starred_at),
        created_at: entry.created_at,
        updated_at: entry.updated_at,
    })
}

pub fn to_domain_content(
    state: &BrowserState,
    entry_id: i64,
) -> anyhow::Result<Option<EntryContent>> {
    entry_content(state, entry_id)
        .map(|content| {
            Ok(EntryContent {
                entry_id: content.entry_id,
                content_html: content.content_html.clone(),
                content_text: content.content_text.clone(),
                content_hash: content.content_hash.clone(),
                updated_at: content.updated_at,
            })
        })
        .transpose()
}

#[derive(Debug, Default)]
pub struct EntryUpsertOutcome {
    pub inserted_count: u64,
    pub content_changed: bool,
}

pub fn upsert_entries(
    state: &mut BrowserState,
    feed_id: i64,
    entries: Vec<ParsedEntry>,
) -> anyhow::Result<EntryUpsertOutcome> {
    let mut outcome = EntryUpsertOutcome::default();
    for entry in entries {
        let content_hash = hash_content(
            entry.content_html.as_deref(),
            entry.content_text.as_deref(),
            Some(&entry.title),
        );
        let now = now_utc();
        let has_content = entry.content_html.is_some() || entry.content_text.is_some();

        promote_legacy_hex_guid_identity(state, feed_id, &entry);

        let entry_id = if let Some(existing) = state
            .core
            .entries
            .iter_mut()
            .find(|current| current.feed_id == feed_id && current.dedup_key == entry.dedup_key)
        {
            existing.external_id = entry.external_id;
            if let Some(url) = entry.url.as_ref() {
                existing.url = Some(url.to_string());
            }
            existing.title = entry.title;
            existing.author = entry.author;
            existing.summary = entry.summary;
            existing.published_at = entry.published_at.or(existing.published_at);
            existing.updated_at_source = entry.updated_at_source.or(existing.updated_at_source);
            existing.has_content = existing.has_content || has_content;
            existing.updated_at = now;
            existing.id
        } else {
            state.core.next_entry_id += 1;
            outcome.inserted_count += 1;
            let entry_id = state.core.next_entry_id;
            state.core.entries.push(PersistedEntryIndex {
                id: entry_id,
                feed_id,
                external_id: entry.external_id,
                dedup_key: entry.dedup_key,
                url: entry.url.as_ref().map(ToString::to_string),
                title: entry.title,
                author: entry.author,
                summary: entry.summary,
                published_at: entry.published_at,
                updated_at_source: entry.updated_at_source,
                first_seen_at: now,
                has_content,
                created_at: now,
                updated_at: now,
            });
            entry_id
        };

        if has_content {
            outcome.content_changed |= upsert_entry_content(
                &mut state.entry_content,
                PersistedEntryContent {
                    entry_id,
                    feed_id,
                    content_html: entry.content_html,
                    content_text: entry.content_text,
                    content_hash,
                    updated_at: now,
                },
            );
        }
    }
    Ok(outcome)
}

fn promote_legacy_hex_guid_identity(state: &mut BrowserState, feed_id: i64, entry: &ParsedEntry) {
    let Some(url) = entry.url.as_ref().map(Url::as_str) else {
        return;
    };
    if entry.external_id != entry.dedup_key
        || entry.dedup_key == url
        || !matches!(entry.dedup_key.len(), 32 | 40 | 64)
        || !entry.dedup_key.chars().all(|ch| ch.is_ascii_hexdigit())
    {
        return;
    }
    if state.core.entries.iter().any(|current| {
        current.feed_id == feed_id
            && (current.external_id == entry.dedup_key || current.dedup_key == entry.dedup_key)
    }) {
        return;
    }

    if let Some(legacy) = state.core.entries.iter_mut().find(|current| {
        current.feed_id == feed_id
            && current.external_id == url
            && current.dedup_key == url
            && current.url.as_deref() == Some(url)
    }) {
        legacy.external_id = entry.dedup_key.clone();
        legacy.dedup_key = entry.dedup_key.clone();
    }
}

fn upsert_entry_content(
    slice: &mut PersistedEntryContentSlice,
    content: PersistedEntryContent,
) -> bool {
    if let Some(existing) =
        slice.entries.iter_mut().find(|current| current.entry_id == content.entry_id)
    {
        // Compare the merged values, not just the hash: absent fields preserve cached data,
        // and older hashes do not distinguish every possible partition of HTML/text/title.
        if existing.feed_id == content.feed_id
            && existing.content_hash == content.content_hash
            && content
                .content_html
                .as_ref()
                .is_none_or(|html| existing.content_html.as_ref() == Some(html))
            && content
                .content_text
                .as_ref()
                .is_none_or(|text| existing.content_text.as_ref() == Some(text))
        {
            return false;
        }
        existing.feed_id = content.feed_id;
        if content.content_html.is_some() {
            existing.content_html = content.content_html;
        }
        if content.content_text.is_some() {
            existing.content_text = content.content_text;
        }
        existing.content_hash = content.content_hash;
        existing.updated_at = content.updated_at;
    } else {
        slice.entries.push(content);
    }
    true
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use super::*;

    const GUID: &str = "0123456789abcdef0123456789abcdef";
    const ARTICLE_URL: &str = "https://example.com/article";

    #[test]
    fn legacy_url_identity_is_promoted_without_losing_browser_state() {
        let timestamp = OffsetDateTime::UNIX_EPOCH;
        let mut state = BrowserState::default();
        state.core.next_entry_id = 1;
        state.core.entries.push(PersistedEntryIndex {
            id: 1,
            feed_id: 7,
            external_id: ARTICLE_URL.into(),
            dedup_key: ARTICLE_URL.into(),
            url: Some(ARTICLE_URL.into()),
            title: "Stable article".into(),
            author: None,
            summary: Some("legacy".into()),
            published_at: Some(timestamp),
            updated_at_source: None,
            first_seen_at: timestamp,
            has_content: true,
            created_at: timestamp,
            updated_at: timestamp,
        });
        state.entry_flags.entries.push(PersistedEntryFlag {
            id: 1,
            is_read: true,
            is_starred: true,
            read_at: Some(timestamp),
            starred_at: Some(timestamp),
        });
        state.entry_content.entries.push(PersistedEntryContent {
            entry_id: 1,
            feed_id: 7,
            content_html: Some("<p>legacy</p>".into()),
            content_text: Some("legacy".into()),
            content_hash: Some("legacy-hash".into()),
            updated_at: timestamp,
        });

        let outcome = upsert_entries(
            &mut state,
            7,
            vec![ParsedEntry {
                external_id: GUID.into(),
                dedup_key: GUID.into(),
                url: Some(Url::parse(ARTICLE_URL).unwrap()),
                title: "Stable article".into(),
                author: None,
                summary: Some("new body".into()),
                content_html: Some("<p>new body</p>".into()),
                content_text: Some("new body".into()),
                published_at: Some(timestamp),
                updated_at_source: None,
            }],
        )
        .unwrap();

        assert_eq!(outcome.inserted_count, 0);
        assert_eq!(state.core.entries.len(), 1);
        assert_eq!(state.core.entries[0].id, 1);
        assert_eq!(state.core.entries[0].external_id, GUID);
        assert_eq!(state.core.entries[0].dedup_key, GUID);
        assert!(state.entry_flags.entries[0].is_read);
        assert!(state.entry_flags.entries[0].is_starred);
        assert_eq!(state.entry_content.entries.len(), 1);
        assert_eq!(state.entry_content.entries[0].entry_id, 1);
        assert_eq!(state.entry_content.entries[0].content_text.as_deref(), Some("new body"));
    }
}

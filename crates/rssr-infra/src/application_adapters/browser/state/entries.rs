use rssr_domain::Entry;
use url::Url;

use crate::application_adapters::browser::{
    feed::{ParsedEntry, hash_content},
    now_utc,
};

use super::{BrowserState, PersistedEntry, PersistedEntryFlag, PersistedState};

pub fn entry_flags(state: &BrowserState, entry_id: i64) -> Option<&PersistedEntryFlag> {
    state.entry_flags.entries.iter().find(|flag| flag.id == entry_id)
}

pub fn to_domain_entry(state: &BrowserState, entry: &PersistedEntry) -> anyhow::Result<Entry> {
    let flags = entry_flags(state, entry.id);
    Ok(Entry {
        id: entry.id,
        feed_id: entry.feed_id,
        external_id: entry.external_id.clone(),
        dedup_key: entry.dedup_key.clone(),
        url: entry.url.as_ref().map(|raw| Url::parse(raw)).transpose()?,
        title: entry.title.clone(),
        author: entry.author.clone(),
        summary: entry.summary.clone(),
        content_html: entry.content_html.clone(),
        content_text: entry.content_text.clone(),
        published_at: entry.published_at,
        updated_at_source: entry.updated_at_source,
        first_seen_at: entry.first_seen_at,
        content_hash: entry.content_hash.clone(),
        is_read: flags.map(|flag| flag.is_read).unwrap_or(false),
        is_starred: flags.map(|flag| flag.is_starred).unwrap_or(false),
        read_at: flags.and_then(|flag| flag.read_at),
        starred_at: flags.and_then(|flag| flag.starred_at),
        created_at: entry.created_at,
        updated_at: entry.updated_at,
    })
}

pub fn upsert_entries(
    state: &mut PersistedState,
    feed_id: i64,
    entries: Vec<ParsedEntry>,
) -> anyhow::Result<()> {
    for entry in entries {
        let content_hash = hash_content(
            entry.content_html.as_deref(),
            entry.content_text.as_deref(),
            Some(&entry.title),
        );
        let now = now_utc();
        if let Some(existing) = state
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
            if entry.content_html.is_some() {
                existing.content_html = entry.content_html;
            }
            if entry.content_text.is_some() {
                existing.content_text = entry.content_text;
            }
            existing.published_at = entry.published_at.or(existing.published_at);
            existing.updated_at_source = entry.updated_at_source.or(existing.updated_at_source);
            existing.content_hash = content_hash;
            existing.updated_at = now;
        } else {
            state.next_entry_id += 1;
            state.entries.push(PersistedEntry {
                id: state.next_entry_id,
                feed_id,
                external_id: entry.external_id,
                dedup_key: entry.dedup_key,
                url: entry.url.map(|url| url.to_string()),
                title: entry.title,
                author: entry.author,
                summary: entry.summary,
                content_html: entry.content_html,
                content_text: entry.content_text,
                published_at: entry.published_at,
                updated_at_source: entry.updated_at_source,
                first_seen_at: now,
                content_hash,
                created_at: now,
                updated_at: now,
            });
        }
    }
    Ok(())
}

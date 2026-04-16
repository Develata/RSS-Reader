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

pub fn upsert_entries(
    state: &mut BrowserState,
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
        let has_content = entry.content_html.is_some() || entry.content_text.is_some();

        let entry_id = if let Some(existing) = state
            .core
            .entries
            .iter_mut()
            .find(|current| current.feed_id == feed_id && current.dedup_key == entry.dedup_key)
        {
            existing.external_id = entry.external_id.clone();
            if let Some(url) = entry.url.as_ref() {
                existing.url = Some(url.to_string());
            }
            existing.title = entry.title.clone();
            existing.author = entry.author.clone();
            existing.summary = entry.summary.clone();
            existing.published_at = entry.published_at.or(existing.published_at);
            existing.updated_at_source = entry.updated_at_source.or(existing.updated_at_source);
            existing.has_content = existing.has_content || has_content;
            existing.updated_at = now;
            existing.id
        } else {
            state.core.next_entry_id += 1;
            let entry_id = state.core.next_entry_id;
            state.core.entries.push(PersistedEntryIndex {
                id: entry_id,
                feed_id,
                external_id: entry.external_id.clone(),
                dedup_key: entry.dedup_key.clone(),
                url: entry.url.as_ref().map(ToString::to_string),
                title: entry.title.clone(),
                author: entry.author.clone(),
                summary: entry.summary.clone(),
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
            upsert_entry_content(
                &mut state.entry_content,
                PersistedEntryContent {
                    entry_id,
                    feed_id,
                    content_html: entry.content_html.clone(),
                    content_text: entry.content_text.clone(),
                    content_hash,
                    updated_at: now,
                },
            );
        }
    }
    Ok(())
}

fn upsert_entry_content(slice: &mut PersistedEntryContentSlice, content: PersistedEntryContent) {
    if let Some(existing) =
        slice.entries.iter_mut().find(|current| current.entry_id == content.entry_id)
    {
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
}

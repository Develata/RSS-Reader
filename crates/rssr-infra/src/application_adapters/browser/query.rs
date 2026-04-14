use std::collections::{HashMap, HashSet};

use rssr_domain::{
    Entry, EntryNavigation, EntryQuery, EntrySummary, FeedSummary, ReadFilter, StarredFilter,
};

use super::state::{BrowserState, PersistedEntry, PersistedEntryFlag, to_domain_entry};

fn build_entry_flag_index(state: &BrowserState) -> HashMap<i64, &PersistedEntryFlag> {
    state.entry_flags.entries.iter().map(|flag| (flag.id, flag)).collect()
}

pub fn list_feeds(state: &BrowserState) -> Vec<FeedSummary> {
    let entry_flags = build_entry_flag_index(state);
    let mut counts_by_feed = HashMap::<i64, (u32, u32)>::new();
    for entry in &state.core.entries {
        let counts = counts_by_feed.entry(entry.feed_id).or_insert((0, 0));
        counts.0 += 1;
        if !entry_flags.get(&entry.id).map(|flag| flag.is_read).unwrap_or(false) {
            counts.1 += 1;
        }
    }

    let mut feeds = state
        .core
        .feeds
        .iter()
        .filter(|feed| !feed.is_deleted)
        .map(|feed| FeedSummary {
            id: feed.id,
            title: feed.title.clone().unwrap_or_else(|| feed.url.clone()),
            url: feed.url.clone(),
            unread_count: counts_by_feed.get(&feed.id).map(|(_, unread)| *unread).unwrap_or(0),
            entry_count: counts_by_feed.get(&feed.id).map(|(all, _)| *all).unwrap_or(0),
            last_fetched_at: feed.last_fetched_at,
            last_success_at: feed.last_success_at,
            fetch_error: feed.fetch_error.clone(),
        })
        .collect::<Vec<_>>();
    feeds.sort_by(|left, right| left.title.cmp(&right.title));
    feeds
}

pub fn list_entries(state: &BrowserState, query: &EntryQuery) -> Vec<EntrySummary> {
    let entry_flags = build_entry_flag_index(state);
    let allowed_feed_ids = (!query.feed_ids.is_empty())
        .then(|| query.feed_ids.iter().copied().collect::<HashSet<_>>());
    let search_lower = query.search_title.as_ref().map(|search| search.to_lowercase());
    let active_feed_titles = state
        .core
        .feeds
        .iter()
        .filter(|feed| !feed.is_deleted)
        .map(|feed| (feed.id, feed.title.clone().unwrap_or_else(|| feed.url.clone())))
        .collect::<HashMap<_, _>>();

    let mut items = state
        .core
        .entries
        .iter()
        .filter(|entry| {
            let flags = entry_flags.get(&entry.id);
            let is_read = flags.map(|flag| flag.is_read).unwrap_or(false);
            let is_starred = flags.map(|flag| flag.is_starred).unwrap_or(false);
            if !active_feed_titles.contains_key(&entry.feed_id) {
                return false;
            }
            if let Some(feed_id) = query.feed_id
                && entry.feed_id != feed_id
            {
                return false;
            }
            if let Some(allowed_feed_ids) = &allowed_feed_ids
                && !allowed_feed_ids.contains(&entry.feed_id)
            {
                return false;
            }
            match query.read_filter {
                ReadFilter::All => {}
                ReadFilter::UnreadOnly if is_read => return false,
                ReadFilter::ReadOnly if !is_read => return false,
                _ => {}
            }
            match query.starred_filter {
                StarredFilter::All => {}
                StarredFilter::StarredOnly if !is_starred => return false,
                StarredFilter::UnstarredOnly if is_starred => return false,
                _ => {}
            }
            if let Some(search) = &search_lower
                && !title_matches_search(&entry.title, search)
            {
                return false;
            }
            true
        })
        .map(|entry| {
            let flags = entry_flags.get(&entry.id);
            EntrySummary {
                id: entry.id,
                feed_id: entry.feed_id,
                title: entry.title.clone(),
                feed_title: active_feed_titles.get(&entry.feed_id).cloned().unwrap_or_default(),
                published_at: entry.published_at,
                is_read: flags.map(|flag| flag.is_read).unwrap_or(false),
                is_starred: flags.map(|flag| flag.is_starred).unwrap_or(false),
            }
        })
        .collect::<Vec<_>>();

    items.sort_by(|left, right| {
        right.published_at.cmp(&left.published_at).then(right.id.cmp(&left.id))
    });
    if let Some(limit) = query.limit {
        items.truncate(limit as usize);
    }
    items
}

pub fn get_entry(state: &BrowserState, entry_id: i64) -> anyhow::Result<Option<Entry>> {
    state
        .core
        .entries
        .iter()
        .find(|entry| entry.id == entry_id)
        .map(|entry| to_domain_entry(state, entry))
        .transpose()
}

pub fn reader_navigation(state: &BrowserState, current_entry_id: i64) -> EntryNavigation {
    let entry_flags = build_entry_flag_index(state);
    let active_feed_ids = state
        .core
        .feeds
        .iter()
        .filter(|feed| !feed.is_deleted)
        .map(|feed| feed.id)
        .collect::<HashSet<_>>();
    let Some(current_entry) = state
        .core
        .entries
        .iter()
        .find(|entry| entry.id == current_entry_id && active_feed_ids.contains(&entry.feed_id))
    else {
        return EntryNavigation::default();
    };

    let mut ordered_entries = state
        .core
        .entries
        .iter()
        .filter(|entry| active_feed_ids.contains(&entry.feed_id))
        .collect::<Vec<_>>();
    ordered_entries.sort_by(|left, right| compare_entry_order(left, right));
    let mut navigation = EntryNavigation::default();

    if let Some(index) = ordered_entries.iter().position(|entry| entry.id == current_entry_id) {
        navigation.previous_unread_entry_id = ordered_entries[..index]
            .iter()
            .rev()
            .find(|entry| !entry_flags.get(&entry.id).map(|flag| flag.is_read).unwrap_or(false))
            .map(|entry| entry.id);
        navigation.next_unread_entry_id = ordered_entries[index + 1..]
            .iter()
            .find(|entry| !entry_flags.get(&entry.id).map(|flag| flag.is_read).unwrap_or(false))
            .map(|entry| entry.id);
        navigation.previous_feed_entry_id = ordered_entries[..index]
            .iter()
            .rev()
            .find(|entry| entry.feed_id == current_entry.feed_id)
            .map(|entry| entry.id);
        navigation.next_feed_entry_id = ordered_entries[index + 1..]
            .iter()
            .find(|entry| entry.feed_id == current_entry.feed_id)
            .map(|entry| entry.id);
    }

    navigation
}

pub fn title_matches_search(title: &str, search_lower: &str) -> bool {
    title.to_lowercase().contains(search_lower)
}

fn compare_entry_order(left: &PersistedEntry, right: &PersistedEntry) -> std::cmp::Ordering {
    right.published_at.cmp(&left.published_at).then(right.id.cmp(&left.id))
}

#[cfg(test)]
mod tests {
    use super::{list_entries, reader_navigation};
    use crate::application_adapters::browser::state::{
        BrowserState, PersistedEntry, PersistedEntryFlag, PersistedEntryFlagsSlice, PersistedFeed,
        PersistedState,
    };
    use rssr_domain::{EntryQuery, ReadFilter};
    use time::{OffsetDateTime, format_description::well_known::Rfc3339};

    fn parse_datetime(raw: &str) -> OffsetDateTime {
        OffsetDateTime::parse(raw, &Rfc3339).expect("parse datetime")
    }

    fn build_state(entry_count: i64) -> BrowserState {
        let now = parse_datetime("2026-04-13T08:00:00Z");
        let feeds = vec![PersistedFeed {
            id: 1,
            url: "https://example.com/feed.xml".to_string(),
            title: Some("Example Feed".to_string()),
            site_url: None,
            description: None,
            icon_url: None,
            folder: None,
            etag: None,
            last_modified: None,
            last_fetched_at: None,
            last_success_at: None,
            fetch_error: None,
            is_deleted: false,
            created_at: now,
            updated_at: now,
        }];
        let entries = (0..entry_count)
            .map(|id| PersistedEntry {
                id: id + 1,
                feed_id: 1,
                external_id: format!("external-{id}"),
                dedup_key: format!("dedup-{id}"),
                url: Some(format!("https://example.com/posts/{id}")),
                title: format!("Performance Article {id}"),
                author: None,
                summary: None,
                content_html: None,
                content_text: None,
                published_at: Some(now - time::Duration::minutes(id)),
                updated_at_source: None,
                first_seen_at: now,
                content_hash: None,
                created_at: now,
                updated_at: now,
            })
            .collect();
        let flags = (0..entry_count)
            .map(|id| PersistedEntryFlag {
                id: id + 1,
                is_read: id % 3 == 0,
                is_starred: id % 5 == 0,
                read_at: None,
                starred_at: None,
            })
            .collect();

        BrowserState {
            core: PersistedState {
                next_feed_id: 2,
                next_entry_id: entry_count + 1,
                feeds,
                entries,
                settings: rssr_domain::UserSettings::default(),
            },
            app_state: rssr_domain::AppStateSnapshot::default(),
            entry_flags: PersistedEntryFlagsSlice { entries: flags },
        }
    }

    #[test]
    fn list_entries_handles_large_browser_state_with_flag_sidecar() {
        let state = build_state(10_000);

        let all_entries = list_entries(&state, &EntryQuery::default());
        let unread_entries = list_entries(
            &state,
            &EntryQuery { read_filter: ReadFilter::UnreadOnly, ..EntryQuery::default() },
        );

        assert_eq!(all_entries.len(), 10_000);
        assert!(unread_entries.len() < all_entries.len());
        assert_eq!(all_entries[0].title, "Performance Article 0");
    }

    #[test]
    fn reader_navigation_handles_large_browser_state() {
        let state = build_state(10_000);

        let navigation = reader_navigation(&state, 5_000);

        assert!(navigation.previous_feed_entry_id.is_some());
        assert!(navigation.next_feed_entry_id.is_some());
    }
}

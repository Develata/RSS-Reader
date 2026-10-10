use std::collections::{BTreeSet, HashMap};

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
    // 每批只建一次查找表：刷新 k 条进 N 条的库从 O(k·N) 降到 O(N + k)（审计 D3）。
    let mut lookup = FeedEntryLookup::build(state, feed_id);
    let mut content_positions = content_positions(&state.entry_content);
    for entry in entries {
        let content_hash = hash_content(
            entry.content_html.as_deref(),
            entry.content_text.as_deref(),
            Some(&entry.title),
            entry.url.as_ref().map(Url::as_str),
        );
        let now = now_utc();
        let has_content = entry.content_html.is_some() || entry.content_text.is_some();

        promote_legacy_hex_guid_identity(state, &mut lookup, &entry);

        let entry_id = if let Some(&position) = lookup.by_dedup_key.get(&entry.dedup_key) {
            let existing = &mut state.core.entries[position];
            lookup.forget(position, existing);
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
            lookup.remember(position, existing);
            existing.id
        } else {
            state.core.next_entry_id += 1;
            outcome.inserted_count += 1;
            let entry_id = state.core.next_entry_id;
            let position = state.core.entries.len();
            lookup.by_dedup_key.insert(entry.dedup_key.clone(), position);
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
            lookup.remember(position, &state.core.entries[position]);
            entry_id
        };

        if has_content {
            outcome.content_changed |= upsert_entry_content(
                &mut state.entry_content,
                &mut content_positions,
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

/// 一个订阅的条目查找表，一批 upsert 只建一次，批内修改时同步维护，查找结果与逐条线性扫描
/// 一致（同一订阅内 `dedup_key` 唯一，重复时取最早的一条，与 `Iterator::find` 相同）。
struct FeedEntryLookup {
    feed_id: i64,
    by_dedup_key: HashMap<String, usize>,
    external_id_counts: HashMap<String, usize>,
    /// 旧版以 URL 为身份的条目（`external_id`、`dedup_key`、`url` 三者相同），按 URL 记位置。
    legacy_by_url: HashMap<String, BTreeSet<usize>>,
}

impl FeedEntryLookup {
    fn build(state: &BrowserState, feed_id: i64) -> Self {
        let mut lookup = Self {
            feed_id,
            by_dedup_key: HashMap::new(),
            external_id_counts: HashMap::new(),
            legacy_by_url: HashMap::new(),
        };
        for (position, entry) in state.core.entries.iter().enumerate() {
            if entry.feed_id == feed_id {
                lookup.by_dedup_key.entry(entry.dedup_key.clone()).or_insert(position);
                lookup.remember(position, entry);
            }
        }
        lookup
    }

    fn remember(&mut self, position: usize, entry: &PersistedEntryIndex) {
        if entry.feed_id != self.feed_id {
            return;
        }
        *self.external_id_counts.entry(entry.external_id.clone()).or_default() += 1;
        if let Some(url) = legacy_url_identity(entry) {
            self.legacy_by_url.entry(url.to_string()).or_default().insert(position);
        }
    }

    fn forget(&mut self, position: usize, entry: &PersistedEntryIndex) {
        if entry.feed_id != self.feed_id {
            return;
        }
        if let Some(count) = self.external_id_counts.get_mut(&entry.external_id) {
            *count -= 1;
            if *count == 0 {
                self.external_id_counts.remove(&entry.external_id);
            }
        }
        if let Some(url) = legacy_url_identity(entry)
            && let Some(positions) = self.legacy_by_url.get_mut(url)
        {
            positions.remove(&position);
            if positions.is_empty() {
                self.legacy_by_url.remove(url);
            }
        }
    }
}

fn legacy_url_identity(entry: &PersistedEntryIndex) -> Option<&str> {
    (entry.external_id == entry.dedup_key && entry.url.as_deref() == Some(&entry.dedup_key))
        .then_some(entry.dedup_key.as_str())
}

fn content_positions(slice: &PersistedEntryContentSlice) -> HashMap<i64, usize> {
    let mut positions = HashMap::with_capacity(slice.entries.len());
    for (position, content) in slice.entries.iter().enumerate() {
        positions.entry(content.entry_id).or_insert(position);
    }
    positions
}

fn promote_legacy_hex_guid_identity(
    state: &mut BrowserState,
    lookup: &mut FeedEntryLookup,
    entry: &ParsedEntry,
) {
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
    if lookup.external_id_counts.contains_key(&entry.dedup_key)
        || lookup.by_dedup_key.contains_key(&entry.dedup_key)
    {
        return;
    }

    let Some(position) =
        lookup.legacy_by_url.get(url).and_then(|positions| positions.first().copied())
    else {
        return;
    };
    let legacy = &mut state.core.entries[position];
    lookup.forget(position, legacy);
    if lookup.by_dedup_key.get(&legacy.dedup_key) == Some(&position) {
        lookup.by_dedup_key.remove(&legacy.dedup_key);
    }
    legacy.external_id = entry.dedup_key.clone();
    legacy.dedup_key = entry.dedup_key.clone();
    lookup.by_dedup_key.insert(entry.dedup_key.clone(), position);
    lookup.remember(position, legacy);
}

fn upsert_entry_content(
    slice: &mut PersistedEntryContentSlice,
    positions: &mut HashMap<i64, usize>,
    content: PersistedEntryContent,
) -> bool {
    if let Some(&position) = positions.get(&content.entry_id) {
        let existing = &mut slice.entries[position];
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
        positions.insert(content.entry_id, slice.entries.len());
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

    #[wasm_bindgen_test::wasm_bindgen_test]
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
    /// 索引化之前的逐条线性实现，作为差分测试的参照（审计 D3）。
    mod reference {
        use super::super::*;

        pub fn upsert_entries(
            state: &mut BrowserState,
            feed_id: i64,
            entries: Vec<ParsedEntry>,
        ) -> EntryUpsertOutcome {
            let mut outcome = EntryUpsertOutcome::default();
            for entry in entries {
                let content_hash = hash_content(
                    entry.content_html.as_deref(),
                    entry.content_text.as_deref(),
                    Some(&entry.title),
                    entry.url.as_ref().map(Url::as_str),
                );
                let now = now_utc();
                let has_content = entry.content_html.is_some() || entry.content_text.is_some();
                promote(state, feed_id, &entry);
                let entry_id = if let Some(existing) =
                    state.core.entries.iter_mut().find(|current| {
                        current.feed_id == feed_id && current.dedup_key == entry.dedup_key
                    }) {
                    existing.external_id = entry.external_id;
                    if let Some(url) = entry.url.as_ref() {
                        existing.url = Some(url.to_string());
                    }
                    existing.title = entry.title;
                    existing.author = entry.author;
                    existing.summary = entry.summary;
                    existing.published_at = entry.published_at.or(existing.published_at);
                    existing.updated_at_source =
                        entry.updated_at_source.or(existing.updated_at_source);
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
                    outcome.content_changed |= upsert_content(
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
            outcome
        }

        fn promote(state: &mut BrowserState, feed_id: i64, entry: &ParsedEntry) {
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
                    && (current.external_id == entry.dedup_key
                        || current.dedup_key == entry.dedup_key)
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

        fn upsert_content(
            slice: &mut PersistedEntryContentSlice,
            content: PersistedEntryContent,
        ) -> bool {
            if let Some(existing) =
                slice.entries.iter_mut().find(|current| current.entry_id == content.entry_id)
            {
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
    }

    /// 固定种子的 xorshift64，测试不依赖随机数库。
    struct Rng(u64);

    impl Rng {
        fn below(&mut self, bound: u64) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0 % bound
        }

        fn pick<'a>(&mut self, items: &'a [String]) -> &'a String {
            &items[self.below(items.len() as u64) as usize]
        }
    }

    const FEED: i64 = 7;

    fn identities() -> (Vec<String>, Vec<String>, Vec<String>) {
        let urls = (0..5).map(|i| format!("https://e.test/{i}")).collect();
        let guids = (0..5).map(|i| format!("{i:032x}")).collect();
        let plain = (0..5).map(|i| format!("id-{i}")).collect();
        (urls, guids, plain)
    }

    fn random_state(rng: &mut Rng) -> BrowserState {
        let (urls, guids, plain) = identities();
        let mut state = BrowserState::default();
        let mut used = std::collections::HashSet::new();
        for _ in 0..rng.below(10) {
            let feed_id = if rng.below(4) == 0 { FEED + 1 } else { FEED };
            let (external_id, dedup_key, url) = match rng.below(4) {
                0 => {
                    let url = rng.pick(&urls).clone();
                    (url.clone(), url.clone(), Some(url))
                }
                1 => {
                    let guid = rng.pick(&guids).clone();
                    (guid.clone(), guid, Some(rng.pick(&urls).clone()))
                }
                2 => (rng.pick(&plain).clone(), rng.pick(&urls).clone(), None),
                _ => {
                    let id = rng.pick(&plain).clone();
                    (id.clone(), id, Some(rng.pick(&urls).clone()))
                }
            };
            if !used.insert((feed_id, dedup_key.clone())) {
                continue;
            }
            state.core.next_entry_id += 1;
            let id = state.core.next_entry_id;
            state.core.entries.push(PersistedEntryIndex {
                id,
                feed_id,
                external_id,
                dedup_key,
                url,
                title: format!("title {}", rng.below(3)),
                author: None,
                summary: None,
                published_at: None,
                updated_at_source: None,
                first_seen_at: OffsetDateTime::UNIX_EPOCH,
                has_content: false,
                created_at: OffsetDateTime::UNIX_EPOCH,
                updated_at: OffsetDateTime::UNIX_EPOCH,
            });
            if rng.below(2) == 0 {
                state.entry_content.entries.push(PersistedEntryContent {
                    entry_id: id,
                    feed_id,
                    content_html: Some(format!("<p>{}</p>", rng.below(3))),
                    content_text: None,
                    content_hash: Some(format!("hash-{}", rng.below(3))),
                    updated_at: OffsetDateTime::UNIX_EPOCH,
                });
            }
        }
        state
    }

    fn random_batch(rng: &mut Rng) -> Vec<ParsedEntry> {
        let (urls, guids, plain) = identities();
        (0..rng.below(12))
            .map(|_| {
                let (external_id, dedup_key, url) = match rng.below(4) {
                    0 => {
                        let guid = rng.pick(&guids).clone();
                        (guid.clone(), guid, Some(rng.pick(&urls).clone()))
                    }
                    1 => {
                        let url = rng.pick(&urls).clone();
                        (url.clone(), url.clone(), Some(url))
                    }
                    2 => {
                        let id = rng.pick(&plain).clone();
                        (id.clone(), id, None)
                    }
                    _ => (rng.pick(&plain).clone(), rng.pick(&urls).clone(), None),
                };
                let body = |rng: &mut Rng| match rng.below(3) {
                    0 => None,
                    1 => Some(String::new()),
                    n => Some(format!("body {n}")),
                };
                ParsedEntry {
                    external_id,
                    dedup_key,
                    url: url.map(|url| Url::parse(&url).unwrap()),
                    title: format!("title {}", rng.below(3)),
                    author: None,
                    summary: None,
                    content_html: body(rng),
                    content_text: body(rng),
                    published_at: None,
                    updated_at_source: None,
                }
            })
            .collect()
    }

    /// 时间戳取自 `now_utc()`，两次运行必然不同；比较前统一抹掉。
    fn comparable(state: &BrowserState) -> serde_json::Value {
        let mut state = state.clone();
        for entry in &mut state.core.entries {
            entry.first_seen_at = OffsetDateTime::UNIX_EPOCH;
            entry.created_at = OffsetDateTime::UNIX_EPOCH;
            entry.updated_at = OffsetDateTime::UNIX_EPOCH;
        }
        for content in &mut state.entry_content.entries {
            content.updated_at = OffsetDateTime::UNIX_EPOCH;
        }
        serde_json::json!({
            "next_entry_id": state.core.next_entry_id,
            "entries": state.core.entries,
            "contents": state.entry_content.entries,
        })
    }

    #[wasm_bindgen_test::wasm_bindgen_test]
    fn indexed_upsert_matches_the_linear_reference() {
        let mut rng = Rng(0x2545_f491_4f6c_dd1d);
        for round in 0..500 {
            let state = random_state(&mut rng);
            let batch = random_batch(&mut rng);
            let mut indexed = state.clone();
            let mut linear = state;
            let fast = upsert_entries(&mut indexed, FEED, batch.clone()).unwrap();
            let slow = reference::upsert_entries(&mut linear, FEED, batch);
            assert_eq!(
                (fast.inserted_count, fast.content_changed),
                (slow.inserted_count, slow.content_changed),
                "round {round}"
            );
            assert_eq!(comparable(&indexed), comparable(&linear), "round {round}");
        }
    }
}

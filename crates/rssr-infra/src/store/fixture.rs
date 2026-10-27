//! Libraries for the browser regression (phase C, C4c): a seed under `tests/fixtures/browser_state/`
//! (four JSON files written by the former in-browser store: subscriptions and entries with their
//! settings, app state, flags and bodies) turned into a `library.sqlite3` with the same ids, times,
//! flags, bodies, settings and browsing preferences. Test tooling only
//! (`examples/browser_fixture_library.rs`); the product never calls it. Like the rest of `store`
//! it is native only: the Web build has no SQLite (`rssr-api/tests/dependency_graph.rs`).
//!
//! The rows are written here, next to the schema and its encodings, so a schema change before
//! v0.2.0 updates them in the same place. Mapping, as the v0.1.x import does: the seed's title is
//! the source title (no custom name); `title_search` is computed; published times are whole
//! seconds; `has_content` is true exactly when the seed has a body for the entry.

use std::collections::BTreeMap;
use std::path::Path;

use rssr_domain::{AppStateSnapshot, UserSettings};
use rusqlite::params;
use serde::Deserialize;
use time::OffsetDateTime;

use super::codec::{format_optional_time, format_time};
use super::documents::{APP_STATE_KEY, USER_SETTINGS_KEY, save};
use super::error::{StoreError, StoreResult};
use super::executor::StoreOptions;
use super::local::open_library_in;

/// One seed: the four files of the former in-browser store.
#[derive(Debug, Clone)]
pub struct BrowserStateSeed {
    core: Core,
    app_state: AppStateSnapshot,
    flags: BTreeMap<i64, FlagRow>,
    bodies: BTreeMap<i64, BodyRow>,
}

#[derive(Debug, Clone, Deserialize)]
struct Core {
    feeds: Vec<FeedRow>,
    entries: Vec<EntryRow>,
    settings: UserSettings,
}

#[derive(Debug, Clone, Deserialize)]
struct FeedRow {
    id: i64,
    url: String,
    title: Option<String>,
    site_url: Option<String>,
    description: Option<String>,
    icon_url: Option<String>,
    folder: Option<String>,
    etag: Option<String>,
    last_modified: Option<String>,
    last_fetched_at: Option<OffsetDateTime>,
    last_success_at: Option<OffsetDateTime>,
    fetch_error: Option<String>,
    is_deleted: bool,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Deserialize)]
struct EntryRow {
    id: i64,
    feed_id: i64,
    external_id: String,
    dedup_key: String,
    url: Option<String>,
    title: String,
    author: Option<String>,
    summary: Option<String>,
    published_at: Option<OffsetDateTime>,
    updated_at_source: Option<OffsetDateTime>,
    first_seen_at: OffsetDateTime,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Deserialize)]
struct FlagRow {
    id: i64,
    is_read: bool,
    is_starred: bool,
    read_at: Option<OffsetDateTime>,
    starred_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Deserialize)]
struct BodyRow {
    entry_id: i64,
    content_html: Option<String>,
    content_text: Option<String>,
    content_hash: Option<String>,
    updated_at: OffsetDateTime,
}

#[derive(Deserialize)]
struct Rows<T> {
    entries: Vec<T>,
}

impl BrowserStateSeed {
    /// Reads a seed from its four files' contents.
    pub fn from_json(core: &str, app_state: &str, flags: &str, bodies: &str) -> StoreResult<Self> {
        let core: Core = parse("core", core)?;
        let app_state: AppStateSnapshot = parse("app_state", app_state)?;
        let flags: Rows<FlagRow> = parse("entry_flags", flags)?;
        let bodies: Rows<BodyRow> = parse("entry_content", bodies)?;
        Ok(Self {
            core,
            app_state,
            flags: flags.entries.into_iter().map(|row| (row.id, row)).collect(),
            bodies: bodies.entries.into_iter().map(|row| (row.entry_id, row)).collect(),
        })
    }

    /// Reads the seed `name` (`<name>_core.json`, …) from `dir`.
    pub fn read(dir: &Path, name: &str) -> StoreResult<Self> {
        let file = |part: &str| std::fs::read_to_string(dir.join(format!("{name}_{part}.json")));
        Self::from_json(
            &file("core")?,
            &file("app_state")?,
            &file("entry_flags")?,
            &file("entry_content")?,
        )
    }

    /// Points every subscription at `<base>/<seed>/<id>.xml` on a local fixture server, with the
    /// entity tag `"<seed>-<id>"`: the server's own fetches (at its start, on a refresh) are
    /// conditional, the fixture server answers `304 Not Modified` unless a test asks for more, and
    /// nothing leaves the machine.
    pub fn serve_feeds_from(&mut self, base: &str, seed: &str) {
        let base = base.trim_end_matches('/');
        for feed in &mut self.core.feeds {
            feed.url = format!("{base}/{seed}/{}.xml", feed.id);
            feed.etag = Some(format!("\"{seed}-{}\"", feed.id));
            feed.last_modified = None;
        }
    }

    /// Replaces the custom CSS in the settings (a theme preset).
    pub fn set_custom_css(&mut self, css: String) {
        self.core.settings.custom_css = css;
    }

    /// The active subscriptions (the ones the server refreshes; a seed may keep a deleted
    /// subscription's entries).
    pub fn active_feeds(&self) -> Vec<ActiveFeed> {
        self.core
            .feeds
            .iter()
            .filter(|feed| !feed.is_deleted)
            .map(|feed| ActiveFeed {
                id: feed.id,
                url: feed.url.clone(),
                last_fetched_at: feed.last_fetched_at,
            })
            .collect()
    }
}

/// An active subscription of a seed, as written: a refresh of the library changes its fetch time.
#[derive(Debug, Clone, PartialEq)]
pub struct ActiveFeed {
    pub id: i64,
    pub url: String,
    pub last_fetched_at: Option<OffsetDateTime>,
}

fn parse<T: serde::de::DeserializeOwned>(part: &str, raw: &str) -> StoreResult<T> {
    serde_json::from_str(raw).map_err(|error| {
        StoreError::from(rssr_domain::DomainError::Persistence(format!(
            "夹具 {part} 无法解析：{error}"
        )))
    })
}

/// Writes `seed` into a new library in `data_dir`, which must not hold one yet.
///
/// The store is closed before this returns, whether or not the write succeeded: its last
/// connection checkpoints the log into the main file as it closes, so a process that exits right
/// after leaves a complete library ([`super::LibraryStore::close`]).
pub async fn write_library(data_dir: &Path, seed: &BrowserStateSeed) -> StoreResult<()> {
    let target = data_dir.join(super::local::LIBRARY_FILE);
    if target.exists() {
        return Err(StoreError::LibraryExists(target.display().to_string()));
    }
    let opened = open_library_in(data_dir, StoreOptions::default()).await?;
    let seed = seed.clone();
    let written = opened.store.write(move |tx| insert(tx, &seed)).await;
    let closed = opened.store.close().await;
    written.and(closed)
}

fn insert(tx: &rusqlite::Transaction<'_>, seed: &BrowserStateSeed) -> StoreResult<()> {
    for feed in &seed.core.feeds {
        tx.execute(
            "INSERT INTO feeds (id, url, custom_title, title, site_url, description, icon_url,
                 folder, etag, last_modified, last_fetched_at, last_success_at, fetch_error,
                 is_deleted, created_at, updated_at)
             VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                feed.id,
                feed.url,
                feed.title,
                feed.site_url,
                feed.description,
                feed.icon_url,
                feed.folder,
                feed.etag,
                feed.last_modified,
                format_optional_time(feed.last_fetched_at)?,
                format_optional_time(feed.last_success_at)?,
                feed.fetch_error,
                feed.is_deleted,
                format_time(feed.created_at)?,
                format_time(feed.updated_at)?,
            ],
        )?;
    }
    for entry in &seed.core.entries {
        let flags = seed.flags.get(&entry.id);
        let body = seed.bodies.get(&entry.id);
        let published_at = entry.published_at.map(rssr_domain::stored_published_at);
        tx.execute(
            "INSERT INTO entries (id, feed_id, external_id, dedup_key, url, title, title_search,
                 author, summary, published_at, updated_at_source, first_seen_at, has_content,
                 is_read, is_starred, read_at, starred_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
                 ?18, ?19)",
            params![
                entry.id,
                entry.feed_id,
                entry.external_id,
                entry.dedup_key,
                entry.url,
                entry.title,
                rssr_domain::fold_for_search(&entry.title),
                entry.author,
                entry.summary,
                format_optional_time(published_at)?,
                format_optional_time(entry.updated_at_source)?,
                format_time(entry.first_seen_at)?,
                body.is_some(),
                flags.is_some_and(|flags| flags.is_read),
                flags.is_some_and(|flags| flags.is_starred),
                format_optional_time(flags.and_then(|flags| flags.read_at))?,
                format_optional_time(flags.and_then(|flags| flags.starred_at))?,
                format_time(entry.created_at)?,
                format_time(entry.updated_at)?,
            ],
        )?;
        if let Some(body) = body {
            tx.execute(
                "INSERT INTO entry_contents (entry_id, content_html, content_text, content_hash,
                     updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    body.entry_id,
                    body.content_html,
                    body.content_text,
                    body.content_hash,
                    format_time(body.updated_at)?,
                ],
            )?;
        }
    }
    let now = super::codec::now();
    save(tx, USER_SETTINGS_KEY, &seed.core.settings, &now)?;
    save(tx, APP_STATE_KEY, &seed.app_state, &now)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rssr_domain::{EntryQuery, ReadFilter};

    use super::{BrowserStateSeed, write_library};
    use crate::store::{LibraryEntries, StoreOptions};

    fn seeds() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/browser_state")
    }

    fn temp_dir(label: &str) -> PathBuf {
        let nonce =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("rssr-fixture-{label}-{nonce}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Every seed becomes a library that opens, with its entries, ids, flags, bodies, settings
    /// and browsing preferences.
    #[tokio::test]
    async fn every_seed_becomes_a_library() {
        for name in ["reader_demo", "mobile_ui_overflow", "mobile_ui_short", "directory_contract"] {
            let seed = BrowserStateSeed::read(&seeds(), name).expect(name);
            let dir = temp_dir(name);
            write_library(&dir, &seed).await.expect(name);
            assert!(!dir.join("library.sqlite3-wal").exists(), "{name}: the log is written back");
            let opened = crate::store::local::open_library_in(&dir, StoreOptions::default())
                .await
                .expect("reopen");
            let entries = LibraryEntries::new(opened.store.clone());
            let listed = rssr_domain::EntryIndexRepository::list_entries(
                &entries,
                &EntryQuery {
                    archive_filter: rssr_domain::ArchiveFilter::All,
                    ..EntryQuery::default()
                },
            )
            .await
            .expect("list");
            assert_eq!(listed.len(), seed.core.entries.len(), "{name}");
            for entry in &seed.core.entries {
                assert!(listed.iter().any(|listed| listed.id == entry.id), "{name}: {}", entry.id);
            }
            let unread = rssr_domain::EntryIndexRepository::list_entries(
                &entries,
                &EntryQuery {
                    archive_filter: rssr_domain::ArchiveFilter::All,
                    read_filter: ReadFilter::UnreadOnly,
                    ..EntryQuery::default()
                },
            )
            .await
            .expect("unread");
            let read = seed.flags.values().filter(|flags| flags.is_read).count();
            assert_eq!(unread.len(), seed.core.entries.len() - read, "{name}: flags");
            if let Some(body) = seed.bodies.values().next() {
                let stored =
                    rssr_domain::EntryContentRepository::get_content(&entries, body.entry_id)
                        .await
                        .expect("body");
                assert_eq!(
                    stored.and_then(|content| content.content_html),
                    body.content_html,
                    "{name}"
                );
            }
        }
    }

    /// A data directory that already holds a library is refused, not overwritten.
    #[tokio::test]
    async fn an_existing_library_is_not_overwritten() {
        let seed = BrowserStateSeed::read(&seeds(), "reader_demo").expect("seed");
        let dir = temp_dir("existing");
        write_library(&dir, &seed).await.expect("first");
        assert!(write_library(&dir, &seed).await.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

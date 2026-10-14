//! One-way import of a v0.1.x local library into a new library file.
//!
//! v0.1.x kept a library in up to two sqlx-managed SQLite files: an index (`rss-reader.db`) and,
//! from v0.1.7, a body database (`rss-reader-content.db`). Two layouts were released:
//!
//! | releases | index migrations | bodies |
//! |---|---|---|
//! | v0.1.0–v0.1.6 | 0001 | inline in `entries` |
//! | v0.1.7–v0.1.23 | 0001–0002, up to 0007 | `entry_contents` in the body database |
//!
//! The import never changes the v0.1.x files. Holding the target's creation lock (see
//! [`super::files::CreationLock`]) from checking that the target is missing until it is published,
//! it
//! 1. copies a consistent snapshot of each (SQLite backup from a read-only connection, so data
//!    still in the WAL is included; a lock held longer than the busy timeout fails the import);
//! 2. checks the snapshot's `_sqlx_migrations` against the released migrations (prefix, success,
//!    checksum with LF or CRLF line endings); unreleased development schemas (0008, 0009) and
//!    anything unknown are refused;
//! 3. builds the new library in a scratch file in one transaction, maps every row, checks the
//!    counts and the database's integrity;
//! 4. publishes it by renaming the scratch file to the target.
//!
//! The two snapshots are taken one after the other, so v0.1.x programs must be closed first (the
//! documented upgrade rule): they do not know the creation lock.
//!
//! Mapping: identifiers are kept (and AUTOINCREMENT high-water marks, so ids are never reused);
//! `custom_title` is empty (v0.1.x names are source titles); `title_search` is computed; refresh
//! tickets start at 0; `has_content` is true exactly when a body row exists, and a body row exists
//! exactly when the source has HTML or text for the entry, in either layout (v0.1.0–v0.1.6 also
//! hashed title-only entries; a hash without a body has no use where a body row means a body to
//! show); bodies of entries that no longer exist are dropped. A split library without its body
//! database imports only when the index records no entry with a body. Documents keep their
//! `updated_at`. What the source holds inconsistently is reported, not hidden: entries marked as
//! having a body without one (every v0.1.6 library upgraded by v0.1.7, whose migration 0002 dropped
//! the inline bodies), bodies filed under another subscription than their entry's (kept with the
//! entry), and bodies with neither HTML nor text (dropped).
//!
//! Legacy documents: settings written by v0.1.6 carried five entry-list preferences that later
//! versions keep in app state, and v0.1.2–v0.1.6 kept the last opened feed under its own key
//! (`last_opened_feed_id`). When the source has no app state, both form it; the five keys are
//! removed from the settings either way.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use rssr_domain::{AppStateSnapshot, EntriesWorkspaceState, UserSettings};
use rusqlite::backup::{Backup, StepResult};
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};
use serde_json::{Map, Value};
use sha2::{Digest, Sha384};

use super::connection::{self, Role};
use super::documents::{APP_STATE_KEY, USER_SETTINGS_KEY};
use super::error::{StoreError, StoreResult};
use super::files::CreationLock;
use super::migrate;

/// Released v0.1.x index migrations, exactly as shipped (line endings aside).
const INDEX_MIGRATIONS: &[(i64, &str)] = &[
    (1, include_str!("../../../../migrations/0001_initial.sql")),
    (2, include_str!("../../../../migrations/0002_split_entry_content.sql")),
    (3, include_str!("../../../../migrations/0003_entry_sort_key_indexes.sql")),
    (4, include_str!("../../../../migrations/0004_drop_unused_entry_sort_index.sql")),
    (5, include_str!("../../../../migrations/0005_restore_entry_sort_index.sql")),
    (6, include_str!("../../../../migrations/0006_feed_generation.sql")),
    (7, include_str!("../../../../migrations/0007_content_gc_queue.sql")),
];

/// Released v0.1.x body migrations.
const CONTENT_MIGRATIONS: &[(i64, &str)] =
    &[(1, include_str!("../../../../migrations_content/0001_initial.sql"))];

/// Which v0.1.x layout a library had.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// v0.1.0–v0.1.6: bodies inline in `entries`.
    Inline,
    /// v0.1.7–v0.1.23: bodies in a separate database.
    Split,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportOutcome {
    pub layout: Layout,
    /// Highest index migration applied in the source.
    pub schema_version: i64,
    pub feeds: u64,
    pub entries: u64,
    pub bodies: u64,
    /// Body rows with HTML or text whose entry no longer existed (not imported).
    pub orphan_bodies: u64,
    /// Body rows with neither HTML nor text, whatever their entry (not imported). `bodies`,
    /// `orphan_bodies` and `empty_bodies` partition the source's body rows.
    pub empty_bodies: u64,
    /// Entries the source marked as having a body and that have none after the import (an entry
    /// whose only body row was empty counts here too).
    pub missing_bodies: u64,
    /// Imported bodies the source filed under another subscription than their entry's.
    pub misfiled_bodies: u64,
    /// Legacy keys moved to app state: the v0.1.6 settings keys and `last_opened_feed_id`.
    pub moved_legacy_keys: Vec<String>,
    /// App state that could not be read and was not imported.
    pub dropped_app_state: bool,
}

/// The v0.1.x files of one library.
#[derive(Debug, Clone)]
pub struct V01Library {
    pub index: PathBuf,
    /// `None` when the body database does not exist.
    pub content: Option<PathBuf>,
}

impl V01Library {
    /// The v0.1.x files in a data directory: `None` only when there is no index database; a path
    /// that cannot be examined or is not a file is an error (an upgrade must not mistake it for a
    /// fresh start).
    pub fn find(data_dir: &Path) -> StoreResult<Option<Self>> {
        let Some(index) = existing_file(data_dir.join("rss-reader.db"))? else { return Ok(None) };
        let content = existing_file(data_dir.join("rss-reader-content.db"))?;
        Ok(Some(Self { index, content }))
    }
}

fn existing_file(path: PathBuf) -> StoreResult<Option<PathBuf>> {
    // Only a name that is not there at all means "no file"; a link to nowhere is an error.
    match std::fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
        Ok(_) => {}
    }
    match std::fs::metadata(&path) {
        Ok(metadata) if metadata.is_file() => Ok(Some(path)),
        Ok(_) => Err(unsupported(format!("{} 不是普通文件", path.display()))),
        Err(error) => Err(unsupported(format!("{} 无法读取：{error}", path.display()))),
    }
}

fn unsupported(detail: impl Into<String>) -> StoreError {
    StoreError::Domain(rssr_domain::DomainError::InvalidInput(format!(
        "无法导入旧版资料库：{}",
        detail.into()
    )))
}

/// sqlx records the SHA-384 of the migration as compiled: LF or CRLF, depending on the checkout.
fn matches_released(sql: &str, recorded: &[u8]) -> bool {
    let lf = sql.replace("\r\n", "\n");
    let crlf = lf.replace('\n', "\r\n");
    [lf, crlf].iter().any(|variant| Sha384::digest(variant.as_bytes()).as_slice() == recorded)
}

/// Checks the applied migrations of a snapshot against `released`; returns the highest version.
fn check_history(conn: &Connection, released: &[(i64, &str)], label: &str) -> StoreResult<i64> {
    let has_table: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = '_sqlx_migrations')",
        [],
        |row| row.get(0),
    )?;
    if !has_table {
        return Err(unsupported(format!("{label}没有迁移记录")));
    }
    let mut statement =
        conn.prepare("SELECT version, success, checksum FROM _sqlx_migrations ORDER BY version")?;
    let applied: Vec<(i64, bool, Vec<u8>)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<Result<_, _>>()?;
    if applied.is_empty() {
        return Err(unsupported(format!("{label}没有已应用的迁移")));
    }
    for (index, (version, success, checksum)) in applied.iter().enumerate() {
        if !success {
            return Err(unsupported(format!("{label}的迁移 {version} 未完成")));
        }
        let Some(&(expected_version, sql)) = released.get(index) else {
            return Err(unsupported(format!(
                "{label}包含未发布的迁移 {version}（开发版程序建立的库不受支持）"
            )));
        };
        if *version != expected_version {
            return Err(unsupported(format!("{label}的迁移记录不连续（{version}）")));
        }
        if !matches_released(sql, checksum) {
            return Err(unsupported(format!("{label}的迁移 {version} 与发布版本不一致")));
        }
    }
    Ok(applied.last().map_or(0, |(version, _, _)| *version))
}

/// Opens a scratch database by file name (never as a URI, like every library connection).
fn open_scratch(path: &Path) -> rusqlite::Result<Connection> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
}

/// Copies a consistent snapshot of `source` to `copy` without writing to `source`.
///
/// One step copies every page under one read snapshot. Waiting for a lock is bounded by `busy`
/// (the source connection's busy timeout): `rusqlite`'s `run_to_completion` would retry a busy
/// source forever.
fn snapshot(source: &Path, copy: &Path, busy: Duration) -> StoreResult<()> {
    let from = Connection::open_with_flags(
        source,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    from.busy_timeout(busy)?;
    let mut to = open_scratch(copy)?;
    let backup = Backup::new(&from, &mut to)?;
    loop {
        match backup.step(-1)? {
            StepResult::Done => return Ok(()),
            StepResult::More => {}
            StepResult::Busy | StepResult::Locked => {
                return Err(unsupported(format!(
                    "{} 被其他程序占用，请先关闭旧版 RSS-Reader",
                    source.display()
                )));
            }
            _ => return Err(unsupported("备份返回了未知状态")),
        }
    }
}

/// Moves the five v0.1.6 entry-list preferences out of the settings document.
const LEGACY_WORKSPACE_KEYS: [(&str, &str); 5] = [
    ("entry_grouping_mode", "grouping_mode"),
    ("show_archived_entries", "show_archived"),
    ("entry_read_filter", "read_filter"),
    ("entry_starred_filter", "starred_filter"),
    ("entry_filtered_feed_urls", "selected_feed_urls"),
];

/// Key of the last opened feed in v0.1.2–v0.1.6 (a decimal feed id).
const LEGACY_LAST_OPENED_FEED_KEY: &str = "last_opened_feed_id";

/// v0.1.x documents as stored: `(value, updated_at)` per key.
struct SourceDocuments {
    settings: Option<(String, String)>,
    app_state: Option<(String, String)>,
    last_opened_feed: Option<(String, String)>,
}

struct Documents {
    /// `(key, value, updated_at)`; `None` time means the import's.
    rows: Vec<(&'static str, String, Option<String>)>,
    moved_legacy_keys: Vec<String>,
    dropped_app_state: bool,
}

/// Turns v0.1.x documents into ones v0.2 reads. Unknown settings fail the import (nothing is
/// silently lost) except the legacy keys, which move to app state.
fn convert_documents(source: SourceDocuments) -> StoreResult<Documents> {
    let mut workspace = Map::new();
    let mut moved_legacy_keys = Vec::new();
    let mut rows = Vec::new();
    match source.settings {
        None => {}
        Some((raw, updated_at)) => {
            let mut object: Map<String, Value> = serde_json::from_str(&raw)
                .map_err(|error| unsupported(format!("设置无法解析：{error}")))?;
            for (legacy, current) in LEGACY_WORKSPACE_KEYS {
                if let Some(value) = object.remove(legacy) {
                    workspace.insert(current.to_owned(), value);
                    moved_legacy_keys.push(legacy.to_owned());
                }
            }
            let parsed: UserSettings = serde_json::from_value(Value::Object(object))
                .map_err(|error| unsupported(format!("设置无法解析：{error}")))?;
            let value =
                serde_json::to_string(&parsed).map_err(|error| unsupported(error.to_string()))?;
            rows.push((USER_SETTINGS_KEY, value, Some(updated_at)));
        }
    }
    let mut dropped_app_state = false;
    match source.app_state {
        Some((raw, updated_at)) => match serde_json::from_str::<AppStateSnapshot>(&raw) {
            Ok(_) => rows.push((APP_STATE_KEY, raw, Some(updated_at))),
            Err(error) => {
                tracing::warn!(%error, "旧版应用状态无法解析，不导入");
                dropped_app_state = true;
            }
        },
        None => {
            let mut snapshot = AppStateSnapshot::default();
            let mut legacy = false;
            if !workspace.is_empty() {
                match serde_json::from_value::<EntriesWorkspaceState>(Value::Object(workspace)) {
                    Ok(entries_workspace) => {
                        snapshot.entries_workspace = entries_workspace;
                        legacy = true;
                    }
                    Err(error) => {
                        tracing::warn!(%error, "旧版文章列表偏好无法解析，不导入");
                        dropped_app_state = true;
                    }
                }
            }
            if let Some((raw, _)) = source.last_opened_feed {
                match raw.parse::<i64>() {
                    Ok(feed_id) => {
                        snapshot.last_opened_feed_id = Some(feed_id);
                        moved_legacy_keys.push(LEGACY_LAST_OPENED_FEED_KEY.to_owned());
                        legacy = true;
                    }
                    Err(error) => {
                        tracing::warn!(%error, "旧版上次打开的订阅无法解析，不导入");
                        dropped_app_state = true;
                    }
                }
            }
            if legacy {
                let value = serde_json::to_string(&snapshot)
                    .map_err(|error| unsupported(error.to_string()))?;
                rows.push((APP_STATE_KEY, value, None));
            }
        }
    }
    Ok(Documents { rows, moved_legacy_keys, dropped_app_state })
}

fn count(conn: &Connection, sql: &str) -> StoreResult<u64> {
    let value: i64 = conn.query_row(sql, [], |row| row.get(0))?;
    Ok(value as u64)
}

/// Imports the v0.1.x library `source` into a new library at `target`, which must not exist.
pub fn import_v01(source: &V01Library, target: &Path) -> StoreResult<ImportOutcome> {
    import_unless(source, target, &AtomicBool::new(false))
}

/// [`import_v01`] that does not start (or keeps waiting to start) once `gave_up` is set.
pub(super) fn import_unless(
    source: &V01Library,
    target: &Path,
    gave_up: &AtomicBool,
) -> StoreResult<ImportOutcome> {
    import_with(source, target, connection::BUSY_TIMEOUT, gave_up)
}

fn import_with(
    source: &V01Library,
    target: &Path,
    busy: Duration,
    gave_up: &AtomicBool,
) -> StoreResult<ImportOutcome> {
    // One interpretation of every path for the lock, the scratch files and SQLite (an absolute
    // path never reads as a `file:` URI), fixed before anything happens.
    let target = &std::path::absolute(target)?;
    let source = &V01Library {
        index: std::path::absolute(&source.index)?,
        content: source.content.as_deref().map(std::path::absolute).transpose()?,
    };
    // ATTACH names the scratch files by text: a path that is not UTF-8 is refused before anything
    // is created (scratch names only add ASCII to it).
    if target.to_str().is_none() {
        return Err(unsupported(format!("路径不是有效的 UTF-8：{}", target.display())));
    }
    // The target's scratch files are cleared before use: a source that is one of them (or sits
    // where one would) would be deleted.
    for source_file in std::iter::once(&source.index).chain(source.content.as_ref()) {
        if belongs_to_target(source_file, target)? {
            return Err(unsupported(format!(
                "旧版资料库文件 {} 与目标 {} 的临时文件同名",
                source_file.display(),
                target.display()
            )));
        }
    }
    let taken = || StoreError::LibraryExists(target.display().to_string());
    if target.exists() {
        return Err(taken());
    }
    let lock = CreationLock::acquire(target, gave_up)?;
    if target.exists() {
        return Err(taken());
    }
    if gave_up.load(Ordering::SeqCst) {
        return Err(StoreError::Interrupted);
    }

    let index_copy = lock.scratch("import-index")?;
    snapshot(&source.index, index_copy.path(), busy)?;
    let (index_version, bodies_claimed) = {
        let index = open_scratch(index_copy.path())?;
        let version = check_history(&index, INDEX_MIGRATIONS, "索引库")?;
        let claimed = if version >= 2 {
            count(&index, "SELECT COUNT(*) FROM entries WHERE has_content <> 0")?
        } else {
            0
        };
        (version, claimed)
    };
    let layout = if index_version >= 2 { Layout::Split } else { Layout::Inline };
    let content_copy = match (&source.content, layout) {
        (Some(path), Layout::Split) => {
            let copy = lock.scratch("import-content")?;
            snapshot(path, copy.path(), busy)?;
            check_history(&open_scratch(copy.path())?, CONTENT_MIGRATIONS, "正文库")?;
            Some(copy)
        }
        // v0.1.7+ creates the body database with the index: without it, bodies the index records
        // would be lost silently.
        (None, Layout::Split) if bodies_claimed > 0 => {
            return Err(unsupported(format!(
                "找不到正文库 rss-reader-content.db，而索引库记录了 {bodies_claimed} 篇有正文的文章"
            )));
        }
        _ => None,
    };

    let staging = lock.scratch("importing")?;
    let mut conn = open_scratch(staging.path())?;
    connection::configure(&conn, Role::Writer)?;
    migrate::migrate(&mut conn, &super::codec::now())?;
    conn.execute("ATTACH DATABASE ?1 AS v01", [&index_copy.path().to_string_lossy()])?;
    if let Some(copy) = &content_copy {
        conn.execute("ATTACH DATABASE ?1 AS v01c", [&copy.path().to_string_lossy()])?;
    }
    let outcome = {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let outcome = copy_library(&tx, layout, index_version, content_copy.is_some())?;
        tx.commit()?;
        outcome
    };
    conn.execute_batch("DETACH DATABASE v01")?;
    if content_copy.is_some() {
        conn.execute_batch("DETACH DATABASE v01c")?;
    }
    let integrity: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    let foreign_keys: Option<String> =
        conn.query_row("PRAGMA foreign_key_check", [], |row| row.get(0)).optional()?;
    if integrity != "ok" || foreign_keys.is_some() {
        return Err(unsupported(format!("新资料库校验失败：{integrity}")));
    }
    conn.close().map_err(|(_, error)| error)?;
    lock.publish(staging)?;
    // Logged here: the caller may have gone away while the import ran.
    tracing::info!(?outcome, target = %target.display(), "已导入 v0.1 资料库");
    Ok(outcome)
}

/// Whether `file` is the target or a file the target's creators own next to it
/// (`<target name>.<anything>`), comparing the real directories.
fn belongs_to_target(file: &Path, target: &Path) -> StoreResult<bool> {
    let (Some(file_dir), Some(file_name), Some(target_dir), Some(target_name)) =
        (file.parent(), file.file_name(), target.parent(), target.file_name())
    else {
        return Ok(false);
    };
    let same_dir = match (std::fs::canonicalize(file_dir), std::fs::canonicalize(target_dir)) {
        (Ok(file_dir), Ok(target_dir)) => file_dir == target_dir,
        // A target directory that does not exist holds no scratch files.
        (_, Err(error)) if error.kind() == std::io::ErrorKind::NotFound => false,
        (Err(error), _) | (_, Err(error)) => return Err(error.into()),
    };
    let mut prefix = target_name.to_os_string();
    prefix.push(".");
    let (name, prefix) = (file_name.to_string_lossy(), prefix.to_string_lossy());
    Ok(same_dir && (file_name == target_name || name.starts_with(prefix.as_ref())))
}

/// Published times in the library are whole-second UTC RFC 3339 text: the archive cutoff (a
/// whole second, B2-D7) is compared with them as text, which orders correctly only when both
/// have the same form. The feed parser only produces such times; an imported library may hold
/// others (a fraction of a second, an offset), so those rows are rewritten here. A time that
/// does not parse is left as it was: reading it fails as it did in v0.1.x.
fn normalize_published_times(tx: &Connection) -> StoreResult<()> {
    let stray: Vec<(i64, String)> = {
        let mut statement = tx.prepare(
            "SELECT id, published_at FROM entries
             WHERE published_at IS NOT NULL
               AND (published_at GLOB '*.*' OR published_at NOT GLOB '*Z')",
        )?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<Result<_, _>>()?
    };
    let mut update = tx.prepare("UPDATE entries SET published_at = ?2 WHERE id = ?1")?;
    for (id, raw) in stray {
        let Ok(time) = super::codec::parse_time(&raw) else { continue };
        let whole = time.replace_nanosecond(0).expect("zero nanoseconds are valid");
        update.execute(rusqlite::params![id, super::codec::format_time(whole)?])?;
    }
    Ok(())
}

fn copy_library(
    tx: &Connection,
    layout: Layout,
    schema_version: i64,
    content: bool,
) -> StoreResult<ImportOutcome> {
    let generation = if schema_version >= 6 { "generation" } else { "0" };
    tx.execute_batch(&format!(
        "INSERT INTO feeds (id, url, title, site_url, description, icon_url, folder, etag,
                            last_modified, last_fetched_at, last_success_at, fetch_error,
                            is_deleted, generation, created_at, updated_at)
         SELECT id, url, title, site_url, description, icon_url, folder, etag, last_modified,
                last_fetched_at, last_success_at, fetch_error, is_deleted, {generation},
                created_at, updated_at
         FROM v01.feeds;

         INSERT INTO entries (id, feed_id, external_id, dedup_key, url, title, title_search,
                              author, summary, published_at, updated_at_source, first_seen_at,
                              has_content, is_read, is_starred, read_at, starred_at,
                              created_at, updated_at)
         SELECT id, feed_id, external_id, dedup_key, url, title, fold_for_search(title), author,
                summary, published_at, updated_at_source, first_seen_at, 0, is_read, is_starred,
                read_at, starred_at, created_at, updated_at
         FROM v01.entries;"
    ))?;
    normalize_published_times(tx)?;
    let bodies_sql = match (layout, content) {
        (Layout::Inline, _) => Some((
            "INSERT INTO entry_contents (entry_id, content_html, content_text, content_hash,
                                         updated_at)
             SELECT id, content_html, content_text, content_hash, updated_at FROM v01.entries
             WHERE content_html IS NOT NULL OR content_text IS NOT NULL",
            "SELECT COUNT(*) FROM v01.entries
             WHERE content_html IS NOT NULL OR content_text IS NOT NULL",
        )),
        (Layout::Split, true) => Some((
            "INSERT INTO entry_contents (entry_id, content_html, content_text, content_hash,
                                         updated_at)
             SELECT entry_id, content_html, content_text, content_hash, updated_at
             FROM v01c.entry_contents
             WHERE entry_id IN (SELECT id FROM main.entries)
               AND (content_html IS NOT NULL OR content_text IS NOT NULL)",
            "SELECT COUNT(*) FROM v01c.entry_contents
             WHERE content_html IS NOT NULL OR content_text IS NOT NULL",
        )),
        (Layout::Split, false) => None,
    };
    let (bodies, source_bodies) = match bodies_sql {
        Some((insert, source)) => (tx.execute(insert, [])? as u64, count(tx, source)?),
        None => (0, 0),
    };
    let orphan_bodies = source_bodies
        .checked_sub(bodies)
        .ok_or_else(|| unsupported(format!("正文行数不一致：{source_bodies} → {bodies}")))?;
    let (empty_bodies, misfiled_bodies) = if layout == Layout::Split && content {
        (
            count(
                tx,
                "SELECT COUNT(*) FROM v01c.entry_contents
                 WHERE content_html IS NULL AND content_text IS NULL",
            )?,
            count(
                tx,
                "SELECT COUNT(*) FROM v01c.entry_contents AS body
                 JOIN main.entry_contents AS kept ON kept.entry_id = body.entry_id
                 JOIN main.entries AS entry ON entry.id = body.entry_id
                 WHERE body.feed_id IS NOT entry.feed_id",
            )?,
        )
    } else {
        (0, 0)
    };
    let missing_bodies = if layout == Layout::Split {
        count(
            tx,
            "SELECT COUNT(*) FROM v01.entries
             WHERE has_content <> 0 AND id NOT IN (SELECT entry_id FROM main.entry_contents)",
        )?
    } else {
        0
    };
    tx.execute_batch(
        "UPDATE entries SET has_content = 1 WHERE id IN (SELECT entry_id FROM entry_contents);
         INSERT INTO main.sqlite_sequence (name, seq)
         SELECT name, 0 FROM v01.sqlite_sequence
         WHERE name IN ('feeds', 'entries')
           AND name NOT IN (SELECT name FROM main.sqlite_sequence);
         UPDATE main.sqlite_sequence
         SET seq = MAX(seq, (SELECT old.seq FROM v01.sqlite_sequence AS old
                             WHERE old.name = main.sqlite_sequence.name))
         WHERE name IN (SELECT name FROM v01.sqlite_sequence WHERE name IN ('feeds', 'entries'));",
    )?;

    let documents = {
        let read = |key: &str| -> StoreResult<Option<(String, String)>> {
            Ok(tx
                .query_row(
                    "SELECT value, updated_at FROM v01.app_settings WHERE key = ?1",
                    [key],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?)
        };
        convert_documents(SourceDocuments {
            settings: read(USER_SETTINGS_KEY)?,
            app_state: read(APP_STATE_KEY)?,
            last_opened_feed: read(LEGACY_LAST_OPENED_FEED_KEY)?,
        })?
    };
    let now = super::codec::now();
    for (key, value, updated_at) in &documents.rows {
        tx.execute(
            "INSERT INTO app_settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
            params![key, value, updated_at.as_deref().unwrap_or(&now)],
        )?;
    }

    let outcome = ImportOutcome {
        layout,
        schema_version,
        feeds: count(tx, "SELECT COUNT(*) FROM main.feeds")?,
        entries: count(tx, "SELECT COUNT(*) FROM main.entries")?,
        bodies,
        orphan_bodies,
        empty_bodies,
        missing_bodies,
        misfiled_bodies,
        moved_legacy_keys: documents.moved_legacy_keys,
        dropped_app_state: documents.dropped_app_state,
    };
    let expected: BTreeMap<&str, u64> = [
        ("feeds", count(tx, "SELECT COUNT(*) FROM v01.feeds")?),
        ("entries", count(tx, "SELECT COUNT(*) FROM v01.entries")?),
    ]
    .into();
    let actual: BTreeMap<&str, u64> =
        [("feeds", outcome.feeds), ("entries", outcome.entries)].into();
    if expected != actual {
        return Err(unsupported(format!("复制后的行数不一致：{expected:?} → {actual:?}")));
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    /// A source locked longer than the busy timeout fails the snapshot instead of waiting forever.
    #[test]
    fn a_locked_source_fails_the_snapshot_within_the_busy_timeout() {
        let nonce =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("rssr-snapshot-busy-{nonce}"));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("rss-reader.db");
        let holder = Connection::open(&source).unwrap();
        holder
            .execute_batch(
                "CREATE TABLE t (x); INSERT INTO t VALUES (1);
                 BEGIN EXCLUSIVE; INSERT INTO t VALUES (2);",
            )
            .unwrap();

        // Run where a hang cannot hold the test: the answer must come within the bound.
        let (answer, receive) = std::sync::mpsc::channel();
        let (from, to) = (source.clone(), dir.join("copy"));
        std::thread::spawn(move || {
            let _ = answer.send(snapshot(&from, &to, Duration::from_millis(100)));
        });
        let started = Instant::now();
        let error = receive
            .recv_timeout(Duration::from_secs(10))
            .expect("the snapshot gives up within its busy timeout")
            .unwrap_err();
        assert!(error.to_string().contains("被其他程序占用"), "{error}");
        assert!(started.elapsed() < Duration::from_secs(5), "{:?}", started.elapsed());

        holder.execute_batch("ROLLBACK").unwrap();
        snapshot(&source, &dir.join("copy-2"), Duration::from_millis(100)).unwrap();
        let copied: i64 = Connection::open(dir.join("copy-2"))
            .unwrap()
            .query_row("SELECT COUNT(*) FROM t", [], |row| row.get(0))
            .unwrap();
        assert_eq!(copied, 1);
        drop(holder);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

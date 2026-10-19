//! Backups of a library file (phase C5): a consistent copy while another program writes, a target
//! never replaced, one winner among backups of one target, leftovers of a killed backup removed,
//! and nothing left behind by a backup cut off by its deadline.

#[path = "support/library.rs"]
mod library;

use std::path::{Path, PathBuf};
use std::time::Duration;

use rssr_infra::store::backup::{BackupOptions, backup_library};
use rssr_infra::store::{LibraryStore, StoreError};

use library::{Library, entry};

struct Dir(PathBuf);

impl Dir {
    fn new(label: &str) -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("rssr-backup-{label} 中文-{nonce}"));
        std::fs::create_dir_all(&path).expect("temp dir");
        Self(path)
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.0)
            .expect("read dir")
            .map(|entry| entry.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A library file with one subscription and `entries` entries.
async fn library(dir: &Dir, entries: usize) -> Library {
    let store = LibraryStore::open(dir.join("library.sqlite3")).await.expect("open library");
    let library = Library::over(store);
    let feed = library.add_feed("https://example.com/feed.xml").await;
    let entries = (0..entries).map(|index| entry(&format!("entry-{index}"))).collect();
    library.seed(feed.id, entries).await;
    library
}

/// The backup opened on its own, without the store (which would migrate it).
fn scalar(path: &Path, sql: &str) -> i64 {
    let connection =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .expect("open backup");
    connection.query_row(sql, [], |row| row.get(0)).expect("query backup")
}

fn integrity(path: &Path) -> String {
    let connection =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .expect("open backup");
    connection.query_row("PRAGMA integrity_check", [], |row| row.get(0)).expect("integrity")
}

fn message(error: &StoreError) -> String {
    error.to_string()
}

/// Another program (a second store on the file, as the server would be) commits two rows per
/// transaction while the backup runs: the copy is one snapshot, so it holds whole transactions.
#[tokio::test(flavor = "multi_thread")]
async fn a_backup_is_one_snapshot_while_another_program_writes() {
    let dir = Dir::new("snapshot");
    let source = library(&dir, 500).await;
    source.exec("CREATE TABLE pairs (value INTEGER NOT NULL)").await;
    let writer =
        Library::over(LibraryStore::open(dir.join("library.sqlite3")).await.expect("second store"));
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let writing = {
        let stop = std::sync::Arc::clone(&stop);
        tokio::spawn(async move {
            let mut commits = 0_u64;
            while !stop.load(std::sync::atomic::Ordering::SeqCst) {
                writer.exec("INSERT INTO pairs VALUES (1); INSERT INTO pairs VALUES (2);").await;
                commits += 1;
            }
            commits
        })
    };
    tokio::time::sleep(Duration::from_millis(50)).await;
    let target = dir.join("backup.sqlite3");
    let report =
        backup_library(&source.store, &target, BackupOptions::default()).await.expect("backup");
    stop.store(true, std::sync::atomic::Ordering::SeqCst);
    let commits = writing.await.expect("writer");

    assert_eq!(report.path, target);
    assert_eq!(report.bytes, std::fs::metadata(&target).expect("target").len());
    assert_eq!(integrity(&target), "ok");
    assert_eq!(scalar(&target, "SELECT count(*) FROM entries"), 500);
    let pairs = scalar(&target, "SELECT count(*) FROM pairs");
    assert_eq!(pairs % 2, 0, "whole transactions only ({pairs} rows)");
    assert!(commits > 0, "the other program kept writing");
    let names = dir.names();
    assert!(names.iter().all(|name| !name.contains(".partial-")), "{names:?}");
    assert!(names.iter().any(|name| name == "backup.sqlite3.lock"), "{names:?}");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&target).expect("target").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "a backup is owner-only, as the library is");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_existing_target_is_refused_and_left_alone() {
    let dir = Dir::new("exists");
    let source = library(&dir, 3).await;
    let target = dir.join("backup.sqlite3");
    std::fs::write(&target, b"keep me").expect("existing file");

    let error = backup_library(&source.store, &target, BackupOptions::default())
        .await
        .expect_err("an existing target is refused");
    assert!(message(&error).contains("已存在"), "{error}");
    assert_eq!(std::fs::read(&target).expect("target"), b"keep me");
    assert!(dir.names().iter().all(|name| !name.contains(".partial-")), "{:?}", dir.names());
}

/// Two backups of one target at once: they wait for each other, and the second finds the target
/// taken instead of replacing it.
#[tokio::test(flavor = "multi_thread")]
async fn two_backups_of_one_target_publish_once() {
    let dir = Dir::new("twice");
    let source = library(&dir, 200).await;
    let target = dir.join("backup.sqlite3");
    let (first, second) = tokio::join!(
        backup_library(&source.store, &target, BackupOptions::default()),
        backup_library(&source.store, &target, BackupOptions::default()),
    );
    let results = [first, second];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1, "{results:?}");
    let refused = results.iter().find_map(|result| result.as_ref().err()).expect("one refused");
    assert!(message(refused).contains("已存在"), "{refused}");
    assert_eq!(integrity(&target), "ok");
    assert_eq!(scalar(&target, "SELECT count(*) FROM entries"), 200);
    assert!(dir.names().iter().all(|name| !name.contains(".partial-")), "{:?}", dir.names());
}

/// A backup killed midway leaves its scratch files; the next backup of that target removes them,
/// and nothing else.
#[tokio::test(flavor = "multi_thread")]
async fn a_killed_backups_scratch_files_are_removed_by_the_next() {
    let dir = Dir::new("leftovers");
    let source = library(&dir, 3).await;
    let target = dir.join("backup.sqlite3");
    for name in ["backup.sqlite3.partial-dead", "backup.sqlite3.partial-dead-journal"] {
        std::fs::write(dir.join(name), b"half a backup").expect("leftover");
    }
    for name in ["backup.sqlite3.partialx", "other.sqlite3.partial-1"] {
        std::fs::write(dir.join(name), b"not ours").expect("unrelated file");
    }

    backup_library(&source.store, &target, BackupOptions::default()).await.expect("backup");
    let names = dir.names();
    assert!(!names.iter().any(|name| name.starts_with("backup.sqlite3.partial-")), "{names:?}");
    for name in ["backup.sqlite3", "backup.sqlite3.partialx", "other.sqlite3.partial-1"] {
        assert!(names.iter().any(|present| present == name), "{name} kept: {names:?}");
    }
    assert_eq!(integrity(&target), "ok");
}

/// A copy still running at its deadline is interrupted: no target, no scratch file.
#[tokio::test(flavor = "multi_thread")]
async fn a_backup_cut_off_by_its_deadline_leaves_nothing() {
    let dir = Dir::new("deadline");
    let source = library(&dir, 2_000).await;
    let target = dir.join("backup.sqlite3");
    let options = BackupOptions { deadline: Duration::ZERO, ..BackupOptions::default() };

    let error = backup_library(&source.store, &target, options)
        .await
        .expect_err("a copy past its deadline is interrupted");
    assert!(matches!(error, StoreError::Interrupted), "{error}");
    assert!(!target.exists());
    assert!(dir.names().iter().all(|name| !name.contains(".partial-")), "{:?}", dir.names());

    // The store is still usable, and a later backup of the same target succeeds.
    backup_library(&source.store, &target, BackupOptions::default()).await.expect("backup");
    assert_eq!(scalar(&target, "SELECT count(*) FROM entries"), 2_000);
}

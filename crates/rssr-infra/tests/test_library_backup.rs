//! Backups of a library file (phase C5): whole transactions while another store writes, a target
//! never replaced, one winner among backups of one target, only this protocol's scratch files
//! removed (including after a backup killed once it had published), reserved names refused, a
//! missing library never created, the lock file required to be a regular file, and nothing left
//! behind by a backup that was cancelled or ran past its deadline.

#[path = "support/library.rs"]
mod library;

use std::path::{Path, PathBuf};
use std::time::Duration;

use rssr_application::ParsedEntryData;
use rssr_infra::store::LibraryStore;
use rssr_infra::store::backup::{BackupCancel, BackupOptions, backup_library};

use library::{Library, entry};

const NONCE: &str = "0123456789abcdef0123456789abcdef";

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

    fn library(&self) -> PathBuf {
        self.join("library.sqlite3")
    }

    fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.0)
            .expect("read dir")
            .map(|entry| entry.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    fn scratch_names(&self) -> Vec<String> {
        self.names().into_iter().filter(|name| name.contains(".partial-")).collect()
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A library file with one subscription and `entries` entries of `body` bytes each.
async fn library(dir: &Dir, entries: usize, body: usize) -> Library {
    let store = LibraryStore::open(dir.library()).await.expect("open library");
    let library = Library::over(store);
    let feed = library.add_feed("https://example.com/feed.xml").await;
    let entries: Vec<ParsedEntryData> = (0..entries)
        .map(|index| ParsedEntryData {
            content_html: (body > 0).then(|| format!("<p>{}</p>", "x".repeat(body))),
            ..entry(&format!("entry-{index}"))
        })
        .collect();
    library.seed(feed.id, entries).await;
    library
}

fn options() -> BackupOptions {
    BackupOptions::default()
}

/// The backup opened on its own, read-only.
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

/// Another store on the file (as the server would be) commits two rows per transaction while the
/// backup runs: the copy is one snapshot, so it holds whole transactions. Whether a commit fell
/// inside the copy is not observed; SQLite's snapshot gives the guarantee, this checks its outcome.
#[tokio::test(flavor = "multi_thread")]
async fn a_backup_holds_whole_transactions_while_another_store_writes() {
    let dir = Dir::new("snapshot");
    let source = library(&dir, 500, 0).await;
    source.exec("CREATE TABLE pairs (value INTEGER NOT NULL)").await;
    let writer = Library::over(LibraryStore::open(dir.library()).await.expect("second store"));
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
    let report = backup_library(&dir.library(), &target, options(), &BackupCancel::new())
        .await
        .expect("backup");
    stop.store(true, std::sync::atomic::Ordering::SeqCst);
    let commits = writing.await.expect("writer");

    assert_eq!(report.path, target);
    assert_eq!(report.bytes, std::fs::metadata(&target).expect("target").len());
    assert_eq!(integrity(&target), "ok");
    assert_eq!(scalar(&target, "SELECT count(*) FROM entries"), 500);
    let pairs = scalar(&target, "SELECT count(*) FROM pairs");
    assert_eq!(pairs % 2, 0, "whole transactions only ({pairs} rows)");
    assert!(commits > 0, "the other store kept writing");
    assert!(dir.scratch_names().is_empty(), "{:?}", dir.names());
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
    let _source = library(&dir, 3, 0).await;
    let target = dir.join("backup.sqlite3");
    std::fs::write(&target, b"keep me").expect("existing file");

    let error = backup_library(&dir.library(), &target, options(), &BackupCancel::new())
        .await
        .expect_err("an existing target is refused");
    assert!(error.to_string().contains("已存在"), "{error}");
    assert_eq!(std::fs::read(&target).expect("target"), b"keep me");
    assert!(dir.scratch_names().is_empty(), "{:?}", dir.names());
}

/// Two backups of one target at once: they wait for each other, and the second finds the target
/// taken instead of replacing it.
#[tokio::test(flavor = "multi_thread")]
async fn two_backups_of_one_target_publish_once() {
    let dir = Dir::new("twice");
    let _source = library(&dir, 200, 0).await;
    let target = dir.join("backup.sqlite3");
    let library = dir.library();
    let (first_cancel, second_cancel) = (BackupCancel::new(), BackupCancel::new());
    let (first, second) = tokio::join!(
        backup_library(&library, &target, options(), &first_cancel),
        backup_library(&library, &target, options(), &second_cancel),
    );
    let results = [first, second];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1, "{results:?}");
    let refused = results.iter().find_map(|result| result.as_ref().err()).expect("one refused");
    assert!(refused.to_string().contains("已存在"), "{refused}");
    assert_eq!(integrity(&target), "ok");
    assert_eq!(scalar(&target, "SELECT count(*) FROM entries"), 200);
    assert!(dir.scratch_names().is_empty(), "{:?}", dir.names());
}

/// Only this protocol's scratch names for this target are removed: a backup named like a scratch
/// file of another kind (`backup.sqlite3.partial-kept`) is someone's backup and stays (review
/// C5-R1 B1).
#[tokio::test(flavor = "multi_thread")]
async fn only_this_targets_scratch_files_are_removed() {
    let dir = Dir::new("leftovers");
    let _source = library(&dir, 3, 0).await;
    let target = dir.join("backup.sqlite3");
    for name in [
        format!("backup.sqlite3.partial-{NONCE}"),
        format!("backup.sqlite3.partial-{NONCE}-journal"),
    ] {
        std::fs::write(dir.join(&name), b"half a backup").expect("leftover");
    }
    let kept = [
        "backup.sqlite3.partial-kept".to_owned(),
        "backup.sqlite3.partialx".to_owned(),
        format!("other.sqlite3.partial-{NONCE}"),
        "backup.sqlite3.partial-FEDCBA9876543210FEDCBA9876543210".to_owned(),
    ];
    for name in &kept {
        std::fs::write(dir.join(name), b"not ours").expect("unrelated file");
    }

    backup_library(&dir.library(), &target, options(), &BackupCancel::new()).await.expect("backup");
    let names = dir.names();
    for name in kept.iter().map(String::as_str).chain(["backup.sqlite3"]) {
        assert!(names.iter().any(|present| present == name), "{name} kept: {names:?}");
    }
    assert!(!names.contains(&format!("backup.sqlite3.partial-{NONCE}")), "{names:?}");
    assert!(!names.contains(&format!("backup.sqlite3.partial-{NONCE}-journal")), "{names:?}");
    assert_eq!(integrity(&target), "ok");
}

/// A backup killed after it published, before removing its scratch name, leaves two names for one
/// complete file. The next backup of that target removes the scratch name first, then refuses
/// the target, which it leaves as it is (review C5-R1 S3).
#[tokio::test(flavor = "multi_thread")]
async fn a_backup_killed_after_publishing_is_tidied_by_the_next() {
    let dir = Dir::new("published");
    let _source = library(&dir, 3, 0).await;
    let target = dir.join("backup.sqlite3");
    backup_library(&dir.library(), &target, options(), &BackupCancel::new())
        .await
        .expect("first backup");
    let scratch = dir.join(&format!("backup.sqlite3.partial-{NONCE}"));
    std::fs::hard_link(&target, &scratch).expect("second name, as a killed backup leaves it");

    let error = backup_library(&dir.library(), &target, options(), &BackupCancel::new())
        .await
        .expect_err("the target exists");
    assert!(error.to_string().contains("已存在"), "{error}");
    assert!(!scratch.exists(), "{:?}", dir.names());
    assert_eq!(integrity(&target), "ok");
}

#[tokio::test(flavor = "multi_thread")]
async fn names_reserved_for_the_protocol_are_refused() {
    let dir = Dir::new("reserved");
    let _source = library(&dir, 3, 0).await;
    let before = dir.names();
    for name in ["backup.lock".to_owned(), format!("backup.sqlite3.partial-{NONCE}")] {
        let error =
            backup_library(&dir.library(), &dir.join(&name), options(), &BackupCancel::new())
                .await
                .expect_err("a reserved name is refused");
        assert!(error.to_string().contains("留给备份自己的文件"), "{name}: {error}");
    }
    assert_eq!(dir.names(), before, "nothing created");
}

/// A data directory without a library: the backup fails and creates nothing there (review C5-R1
/// B3).
#[tokio::test(flavor = "multi_thread")]
async fn a_missing_library_is_never_created() {
    let dir = Dir::new("missing");
    let missing = dir.join("library.sqlite3");
    let target = dir.join("backup.sqlite3");
    backup_library(&missing, &target, options(), &BackupCancel::new())
        .await
        .expect_err("no library to copy");
    assert!(!missing.exists());
    assert!(!target.exists());
    assert!(dir.scratch_names().is_empty(), "{:?}", dir.names());
}

/// The lock file must be a regular file; anything else at its name is refused at once (on Unix a
/// FIFO would otherwise block the open forever, review C5-R1 S1).
#[tokio::test(flavor = "multi_thread")]
async fn a_lock_path_that_is_not_a_file_is_refused() {
    let dir = Dir::new("lock-kind");
    let _source = library(&dir, 3, 0).await;
    let target = dir.join("backup.sqlite3");
    std::fs::create_dir(dir.join("backup.sqlite3.lock")).expect("directory at the lock name");
    let error = backup_library(&dir.library(), &target, options(), &BackupCancel::new())
        .await
        .expect_err("a directory is no lock file");
    assert!(error.to_string().contains("不是普通文件"), "{error}");
    #[cfg(unix)]
    {
        let fifo_target = dir.join("fifo.sqlite3");
        let status = std::process::Command::new("mkfifo")
            .arg(dir.join("fifo.sqlite3.lock"))
            .status()
            .expect("mkfifo");
        assert!(status.success());
        let started = std::time::Instant::now();
        let error = backup_library(&dir.library(), &fifo_target, options(), &BackupCancel::new())
            .await
            .expect_err("a FIFO is no lock file");
        assert!(error.to_string().contains("不是普通文件"), "{error}");
        assert!(started.elapsed() < Duration::from_secs(5), "refused without blocking");
    }
    assert!(!target.exists());
}

/// Another holder of the lock: the backup waits its bounded time, then gives up; a cancel while it
/// waits stops it sooner.
#[tokio::test(flavor = "multi_thread")]
async fn a_held_lock_is_waited_for_a_bounded_time() {
    let dir = Dir::new("held");
    let _source = library(&dir, 3, 0).await;
    let target = dir.join("backup.sqlite3");
    let holder = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("backup.sqlite3.lock"))
        .expect("lock file");
    holder.lock().expect("hold the lock");

    let short = BackupOptions { lock_wait: Duration::from_millis(200), ..options() };
    let error = backup_library(&dir.library(), &target, short, &BackupCancel::new())
        .await
        .expect_err("the lock stays held");
    assert!(error.to_string().contains("另一个备份"), "{error}");

    let cancel = BackupCancel::new();
    let waiting = {
        let (library, target, cancel) = (dir.library(), target.clone(), cancel.clone());
        tokio::spawn(async move { backup_library(&library, &target, options(), &cancel).await })
    };
    tokio::time::sleep(Duration::from_millis(200)).await;
    cancel.cancel();
    let error = waiting.await.expect("task").expect_err("cancelled while waiting");
    assert!(error.to_string().contains("已取消"), "{error}");
    drop(holder);
    assert!(!target.exists());
    assert!(dir.scratch_names().is_empty(), "{:?}", dir.names());
}

/// A copy past its deadline is interrupted: no target, no scratch file; the target can be backed up
/// afterwards.
#[tokio::test(flavor = "multi_thread")]
async fn a_backup_cut_off_by_its_deadline_leaves_nothing() {
    let dir = Dir::new("deadline");
    let _source = library(&dir, 2_000, 0).await;
    let target = dir.join("backup.sqlite3");
    let zero = BackupOptions { deadline: Duration::ZERO, ..options() };

    let error = backup_library(&dir.library(), &target, zero, &BackupCancel::new())
        .await
        .expect_err("a copy past its deadline is interrupted");
    assert!(error.to_string().contains("超过时限"), "{error}");
    assert!(!target.exists());
    assert!(dir.scratch_names().is_empty(), "{:?}", dir.names());

    backup_library(&dir.library(), &target, options(), &BackupCancel::new()).await.expect("backup");
    assert_eq!(scalar(&target, "SELECT count(*) FROM entries"), 2_000);
}

/// Cancelled before it starts, and cancelled once its scratch file exists (the copy is about to run
/// or running): it reports the cancellation only after it stopped, and leaves nothing (review
/// C5-R1 B2).
#[tokio::test(flavor = "multi_thread")]
async fn a_cancelled_backup_stops_before_it_reports_and_leaves_nothing() {
    let dir = Dir::new("cancel");
    let _source = library(&dir, 5_000, 2_000).await;
    let target = dir.join("backup.sqlite3");

    let early = BackupCancel::new();
    early.cancel();
    let error = backup_library(&dir.library(), &target, options(), &early)
        .await
        .expect_err("cancelled before it started");
    assert!(error.to_string().contains("已取消"), "{error}");

    let cancel = BackupCancel::new();
    let running = {
        let (library, target, cancel) = (dir.library(), target.clone(), cancel.clone());
        tokio::spawn(async move { backup_library(&library, &target, options(), &cancel).await })
    };
    let started = std::time::Instant::now();
    while dir.scratch_names().is_empty() {
        assert!(started.elapsed() < Duration::from_secs(30), "the copy never started");
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    cancel.cancel();
    let error = running.await.expect("task").expect_err("cancelled while copying");
    assert!(error.to_string().contains("已取消"), "{error}");
    assert!(!target.exists());
    assert!(dir.scratch_names().is_empty(), "{:?}", dir.names());
}

//! Backups of a library file (phase C5): whole transactions while another store writes, a target
//! never replaced, one winner among backups of one target, only the scratch files a lock file
//! records removed (including after a backup killed once it had published, and under another
//! spelling only where the file system makes it the same lock file), names other than *.sqlite3
//! refused, a missing library never created, the lock file required to be a regular file and not
//! a link, a held lock waited for a bounded time, limits too long to represent taken as none, and
//! nothing left behind by a backup that was cancelled, dropped or ran past its deadline.

#[path = "support/library.rs"]
mod library;

use std::fs::TryLockError;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rssr_application::ParsedEntryData;
use rssr_infra::store::LibraryStore;
use rssr_infra::store::backup::{BackupCancel, BackupOptions, backup_library};

use library::{Library, entry};

const NONCE: &str = "0123456789abcdef0123456789abcdef";
const OTHER: &str = "fedcba9876543210fedcba9876543210";

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

    /// Whether a scratch file itself (not a companion file of SQLite's) has content: the copy is
    /// running.
    fn scratch_filling(&self) -> bool {
        self.scratch_names().iter().any(|name| {
            !["-journal", "-wal", "-shm"].iter().any(|suffix| name.ends_with(suffix))
                && std::fs::metadata(self.join(name)).is_ok_and(|metadata| metadata.len() > 0)
        })
    }

    /// What the lock file of `target` records (empty when nothing, or when it does not exist).
    fn record(&self, target: &str) -> String {
        std::fs::read_to_string(self.join(&format!("{target}.lock"))).unwrap_or_default()
    }

    /// Leaves what a backup that died leaves: the lock file `lock` recording `scratch`.
    fn record_scratch(&self, lock: &str, scratch: &str) {
        std::fs::write(self.join(lock), scratch).expect("record");
    }

    /// Whether the directory's file system resolves names without regard to case.
    fn case_insensitive(&self) -> bool {
        std::fs::write(self.join("probe-case"), b"").expect("probe");
        let insensitive = self.join("PROBE-CASE").exists();
        std::fs::remove_file(self.join("probe-case")).expect("remove probe");
        insensitive
    }

    /// Waits until nothing holds the lock of `target` (a backup's thread ends on its own).
    fn wait_for_lock(&self, target: &str) {
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.join(&format!("{target}.lock")))
            .expect("lock file");
        let started = Instant::now();
        loop {
            match lock.try_lock() {
                Ok(()) => return,
                Err(TryLockError::WouldBlock) => {
                    assert!(started.elapsed() < Duration::from_secs(30), "the lock stays held");
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(TryLockError::Error(error)) => panic!("lock: {error}"),
            }
        }
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
    assert_eq!(dir.record("backup.sqlite3"), "", "the record is cleared");
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

/// Only what the lock file records is removed: the scratch file a backup of this target that died
/// left, with SQLite's companion of it. No other name is guessed to be a leftover, scratch-like
/// or not, and the record is cleared (review C5-R1 B1, C5-R3-02).
#[tokio::test(flavor = "multi_thread")]
async fn only_the_recorded_scratch_files_are_removed() {
    let dir = Dir::new("leftovers");
    let _source = library(&dir, 3, 0).await;
    let target = dir.join("backup.sqlite3");
    let leftover = format!("backup.sqlite3.partial-{NONCE}");
    dir.record_scratch("backup.sqlite3.lock", &leftover);
    for name in [leftover.clone(), format!("{leftover}-journal")] {
        std::fs::write(dir.join(&name), b"half a backup").expect("leftover");
    }
    let kept = [
        "backup.sqlite3.partial-kept".to_owned(),
        format!("backup.sqlite3.partial-{OTHER}"),
        format!("other.sqlite3.partial-{NONCE}"),
    ];
    for name in &kept {
        std::fs::write(dir.join(name), b"not ours").expect("unrelated file");
    }

    backup_library(&dir.library(), &target, options(), &BackupCancel::new()).await.expect("backup");
    let names = dir.names();
    for name in kept.iter().map(String::as_str).chain(["backup.sqlite3"]) {
        assert!(names.iter().any(|present| present == name), "{name} kept: {names:?}");
    }
    assert!(!names.contains(&leftover), "{names:?}");
    assert!(!names.contains(&format!("{leftover}-journal")), "{names:?}");
    assert_eq!(dir.record("backup.sqlite3"), "");
    assert_eq!(integrity(&target), "ok");
}

/// A record names a scratch file that was never created (the backup died in between), or names
/// something that is not a scratch file: nothing is removed, the record is cleared and the backup
/// goes on.
#[tokio::test(flavor = "multi_thread")]
async fn a_record_is_acted_on_only_when_it_names_a_scratch_file() {
    let dir = Dir::new("records");
    let _source = library(&dir, 3, 0).await;
    dir.record_scratch("first.sqlite3.lock", &format!("first.sqlite3.partial-{NONCE}"));
    dir.record_scratch("second.sqlite3.lock", "library.sqlite3");
    for target in ["first.sqlite3", "second.sqlite3"] {
        backup_library(&dir.library(), &dir.join(target), options(), &BackupCancel::new())
            .await
            .expect("backup");
        assert_eq!(dir.record(target), "", "{target}");
    }
    assert_eq!(integrity(&dir.library()), "ok", "the library is still there");
    assert!(dir.scratch_names().is_empty(), "{:?}", dir.names());
}

/// A backup of `Backup.sqlite3` died and left its scratch file, recorded in `Backup.sqlite3.lock`.
/// The next backup, of `backup.sqlite3`, removes it where the file system makes the two names one
/// target (both reach that lock file and its record); on a case-sensitive one they are two
/// targets and it stays (review C5-R2-02, C5-R3-02).
#[tokio::test(flavor = "multi_thread")]
async fn a_leftover_under_another_spelling_is_removed_only_for_the_same_target() {
    let dir = Dir::new("spelling");
    let _source = library(&dir, 3, 0).await;
    let insensitive = dir.case_insensitive();
    eprintln!("file system case-insensitive: {insensitive}");
    let leftover = format!("Backup.sqlite3.partial-{NONCE}");
    dir.record_scratch("Backup.sqlite3.lock", &leftover);
    std::fs::write(dir.join(&leftover), b"half a backup").expect("leftover");

    backup_library(&dir.library(), &dir.join("backup.sqlite3"), options(), &BackupCancel::new())
        .await
        .expect("backup");
    let names = dir.names();
    assert_eq!(
        names.contains(&leftover),
        !insensitive,
        "case-insensitive: {insensitive}, {names:?}"
    );
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
    let scratch_name = format!("backup.sqlite3.partial-{NONCE}");
    let scratch = dir.join(&scratch_name);
    std::fs::hard_link(&target, &scratch).expect("second name, as a killed backup leaves it");
    dir.record_scratch("backup.sqlite3.lock", &scratch_name);

    let error = backup_library(&dir.library(), &target, options(), &BackupCancel::new())
        .await
        .expect_err("the target exists");
    assert!(error.to_string().contains("已存在"), "{error}");
    assert!(!scratch.exists(), "{:?}", dir.names());
    assert_eq!(dir.record("backup.sqlite3"), "");
    assert_eq!(integrity(&target), "ok");
}

/// A backup's name must end in `.sqlite3`. That keeps it off the names of a database's companion
/// files (a backup at one would be taken as the database's journal or shared memory and
/// overwritten), however the file system folds case (`\u{17f}`, long s, folds to `s` on a
/// casefolding ext4), and off the protocol's own names (review C5-R2-01, C5-R3-01). Nothing is
/// created for a refused name.
#[tokio::test(flavor = "multi_thread")]
async fn only_names_ending_in_sqlite3_are_accepted() {
    let dir = Dir::new("names");
    let _source = library(&dir, 3, 0).await;
    let before = dir.names();
    for name in [
        "backup.lock".to_owned(),
        "backup.sqlite3.lock".to_owned(),
        format!("backup.sqlite3.partial-{NONCE}"),
        "library.sqlite3-wal".to_owned(),
        "library.sqlite3-shm".to_owned(),
        "library.sqlite3-journal".to_owned(),
        "library.sqlite3-\u{17f}hm".to_owned(),
        "backup.SQLITE3".to_owned(),
        ".sqlite3".to_owned(),
        "backup".to_owned(),
        "backup.db".to_owned(),
    ] {
        let error =
            backup_library(&dir.library(), &dir.join(&name), options(), &BackupCancel::new())
                .await
                .expect_err("only *.sqlite3 is accepted");
        assert!(error.to_string().contains("须以 .sqlite3 结尾"), "{name}: {error}");
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

/// A link at the lock's name is refused, never followed (review C5-R2-04).
#[tokio::test(flavor = "multi_thread")]
async fn a_link_at_the_lock_name_is_refused() {
    let dir = Dir::new("lock-link");
    let _source = library(&dir, 3, 0).await;
    let target = dir.join("backup.sqlite3");
    std::fs::write(dir.join("elsewhere"), b"").expect("a regular file");
    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink(dir.join("elsewhere"), dir.join("backup.sqlite3.lock"));
    #[cfg(windows)]
    let linked =
        std::os::windows::fs::symlink_file(dir.join("elsewhere"), dir.join("backup.sqlite3.lock"));
    if let Err(error) = linked {
        // Windows lets only administrators or developer mode create links.
        eprintln!("SKIPPED a_link_at_the_lock_name_is_refused: cannot create a link: {error}");
        return;
    }
    let error = backup_library(&dir.library(), &target, options(), &BackupCancel::new())
        .await
        .expect_err("a link is no lock file");
    assert!(error.to_string().contains("不是普通文件"), "{error}");
    assert!(!target.exists());
    assert!(dir.scratch_names().is_empty(), "{:?}", dir.names());
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
    let started = Instant::now();
    let error = backup_library(&dir.library(), &target, short, &BackupCancel::new())
        .await
        .expect_err("the lock stays held");
    let waited = started.elapsed();
    assert!(error.to_string().contains("另一个备份"), "{error}");
    assert!(
        waited >= Duration::from_millis(200) && waited < Duration::from_secs(5),
        "waited {waited:?}"
    );

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

/// A copy past its deadline is interrupted: one past it from the start, and one whose deadline
/// passes while it copies about 10 MB. No target, no scratch file; the target can be backed up
/// afterwards.
#[tokio::test(flavor = "multi_thread")]
async fn a_backup_cut_off_by_its_deadline_leaves_nothing() {
    let dir = Dir::new("deadline");
    let _source = library(&dir, 5_000, 2_000).await;
    let target = dir.join("backup.sqlite3");

    for deadline in [Duration::ZERO, Duration::from_millis(1)] {
        let cut = BackupOptions { deadline, ..options() };
        let error = backup_library(&dir.library(), &target, cut, &BackupCancel::new())
            .await
            .expect_err("a copy past its deadline is interrupted");
        assert!(error.to_string().contains("超过时限"), "{deadline:?}: {error}");
        assert!(!target.exists());
        assert!(dir.scratch_names().is_empty(), "{:?}", dir.names());
    }

    backup_library(&dir.library(), &target, options(), &BackupCancel::new()).await.expect("backup");
    assert_eq!(scalar(&target, "SELECT count(*) FROM entries"), 5_000);
}

/// Limits too long to represent are none, not a panic (review C5-R2-07).
#[tokio::test(flavor = "multi_thread")]
async fn limits_too_long_to_represent_are_none() {
    let dir = Dir::new("unbounded");
    let _source = library(&dir, 3, 0).await;
    let target = dir.join("backup.sqlite3");
    let unbounded = BackupOptions { deadline: Duration::MAX, lock_wait: Duration::MAX };
    backup_library(&dir.library(), &target, unbounded, &BackupCancel::new()).await.expect("backup");
    assert_eq!(integrity(&target), "ok");
}

/// Dropping the future of a backup whose scratch file is being filled cancels it: its thread stops
/// on its own, removes its scratch file and clears its record, then releases the lock; nothing is
/// published (review C5-R2-06, C5-R3-04).
#[tokio::test(flavor = "multi_thread")]
async fn a_dropped_backup_stops_and_leaves_nothing() {
    let dir = Dir::new("dropped");
    let _source = library(&dir, 5_000, 2_000).await;
    let target = dir.join("backup.sqlite3");
    let library = dir.library();
    let cancel = BackupCancel::new();
    let started = async {
        let started = Instant::now();
        while !dir.scratch_filling() {
            assert!(started.elapsed() < Duration::from_secs(30), "the copy never started");
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    };
    tokio::select! {
        result = backup_library(&library, &target, options(), &cancel) => {
            panic!("the backup ended before it was dropped: {result:?}");
        }
        () = started => {}
    }
    dir.wait_for_lock("backup.sqlite3");
    assert!(!target.exists());
    assert!(dir.scratch_names().is_empty(), "{:?}", dir.names());
    assert_eq!(dir.record("backup.sqlite3"), "");
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
    assert_eq!(dir.record("backup.sqlite3"), "");
}

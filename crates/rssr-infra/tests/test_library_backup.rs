#![cfg(not(target_arch = "wasm32"))]
//! Backups of a library file (phase C5): whole transactions while another store writes, a target
//! never replaced, one winner among backups of one target, only scratch-named entries of the
//! target's scratch directory removed (including after a backup killed once it had published, and
//! under another spelling only where the file system makes it the same directory), a scratch
//! directory name that is not a directory refused, names other than *.sqlite3 refused, a missing
//! library never created, the lock file required to be a regular file and not a link (and never
//! written), a held lock waited for a bounded time, limits too long to represent taken as none,
//! and nothing left behind by a backup that was cancelled, dropped or ran past its deadline.

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

    /// The scratch directories (`<target>.partial`) and what they hold, as `dir/` and
    /// `dir/entry`. A directory that cannot be read fails the test rather than reading as empty.
    fn scratch_names(&self) -> Vec<String> {
        let mut found = Vec::new();
        for name in self.names().into_iter().filter(|name| name.ends_with(".partial")) {
            found.push(format!("{name}/"));
            for entry in std::fs::read_dir(self.join(&name)).expect("read scratch directory") {
                let entry = entry.expect("entry").file_name().to_string_lossy().into_owned();
                found.push(format!("{name}/{entry}"));
            }
        }
        found
    }

    /// Whether a scratch file itself (not a companion file of SQLite's) has content: writing began
    /// (whether the copy is still running is not observed).
    fn scratch_filling(&self) -> bool {
        self.scratch_names().iter().any(|name| {
            !name.ends_with('/')
                && !["-journal", "-wal", "-shm"].iter().any(|suffix| name.ends_with(suffix))
                && std::fs::metadata(self.join(name)).is_ok_and(|metadata| metadata.len() > 0)
        })
    }

    /// Leaves a file named `name` in the scratch directory of `target`, as a backup that died
    /// leaves its scratch file there.
    fn leave_scratch(&self, target: &str, name: &str) -> PathBuf {
        let dir = self.join(&format!("{target}.partial"));
        std::fs::create_dir_all(&dir).expect("scratch directory");
        let path = dir.join(name);
        std::fs::write(&path, b"half a backup").expect("leftover");
        path
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
    assert!(dir.scratch_names().is_empty(), "{:?}", dir.scratch_names());
    assert_eq!(
        std::fs::read(dir.join("backup.sqlite3.lock")).expect("lock file"),
        b"",
        "never written"
    );
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

/// Only scratch-named entries of this target's scratch directory are removed: the scratch file a
/// backup of this target that died left, with SQLite's companion of it. Other entries there stay
/// (and keep the directory), and nothing outside it is touched: another target's scratch
/// directory, or a file named the way scratch files used to be (review C5-R1 B1, C5-R3-02).
#[tokio::test(flavor = "multi_thread")]
async fn only_scratch_named_entries_of_the_scratch_directory_are_removed() {
    let dir = Dir::new("leftovers");
    let _source = library(&dir, 3, 0).await;
    let target = dir.join("backup.sqlite3");
    let leftovers = [
        dir.leave_scratch("backup.sqlite3", &format!("partial-{NONCE}")),
        dir.leave_scratch("backup.sqlite3", &format!("partial-{NONCE}-journal")),
    ];
    let kept = [
        dir.leave_scratch("backup.sqlite3", "partial-kept"),
        dir.leave_scratch("backup.sqlite3", "notes.txt"),
        dir.leave_scratch("other.sqlite3", &format!("partial-{OTHER}")),
    ];
    let old_style = dir.join(&format!("backup.sqlite3.partial-{NONCE}"));
    std::fs::write(&old_style, b"not ours").expect("unrelated file");

    backup_library(&dir.library(), &target, options(), &BackupCancel::new()).await.expect("backup");
    for path in &leftovers {
        assert!(!path.exists(), "{} removed: {:?}", path.display(), dir.scratch_names());
    }
    for path in kept.iter().chain([&old_style]) {
        assert!(path.exists(), "{} kept: {:?}", path.display(), dir.scratch_names());
    }
    assert_eq!(integrity(&target), "ok");
}

/// The scratch directory's name holding a file, or a link to a directory: refused before anything
/// is removed (only the lock file has been created). A link is not followed into what it points
/// to.
#[tokio::test(flavor = "multi_thread")]
async fn a_scratch_directory_name_that_is_not_a_directory_is_refused() {
    let dir = Dir::new("scratch-kind");
    let _source = library(&dir, 3, 0).await;
    std::fs::write(dir.join("file.sqlite3.partial"), b"someone's file").expect("a file");
    let error =
        backup_library(&dir.library(), &dir.join("file.sqlite3"), options(), &BackupCancel::new())
            .await
            .expect_err("a file is no scratch directory");
    assert!(error.to_string().contains("不是目录"), "{error}");
    assert_eq!(std::fs::read(dir.join("file.sqlite3.partial")).expect("file"), b"someone's file");
    assert!(!dir.join("file.sqlite3").exists());

    let elsewhere = dir.join("elsewhere");
    std::fs::create_dir(&elsewhere).expect("a directory");
    let named_like_scratch = elsewhere.join(format!("partial-{NONCE}"));
    std::fs::write(&named_like_scratch, b"not ours").expect("file there");
    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink(&elsewhere, dir.join("link.sqlite3.partial"));
    #[cfg(windows)]
    let linked = std::os::windows::fs::symlink_dir(&elsewhere, dir.join("link.sqlite3.partial"));
    match linked {
        Ok(()) => {
            let error = backup_library(
                &dir.library(),
                &dir.join("link.sqlite3"),
                options(),
                &BackupCancel::new(),
            )
            .await
            .expect_err("a link is no scratch directory");
            assert!(error.to_string().contains("不是目录"), "{error}");
            assert!(named_like_scratch.exists(), "nothing removed through the link");
        }
        // Windows lets only administrators or developer mode create links.
        Err(error) => eprintln!("SKIPPED the link half: cannot create a link: {error}"),
    }
}

/// A backup of `Backup.sqlite3` died and left its scratch file in `Backup.sqlite3.partial`. The
/// next backup, of `backup.sqlite3`, removes it where the file system makes the two names one
/// target (both reach that directory, as they reach one lock file); on a case-sensitive one they
/// are two targets and it stays (review C5-R2-02, C5-R3-02).
#[tokio::test(flavor = "multi_thread")]
async fn a_leftover_under_another_spelling_is_removed_only_for_the_same_target() {
    let dir = Dir::new("spelling");
    let _source = library(&dir, 3, 0).await;
    let insensitive = dir.case_insensitive();
    eprintln!("file system case-insensitive: {insensitive}");
    let leftover = dir.leave_scratch("Backup.sqlite3", &format!("partial-{NONCE}"));

    backup_library(&dir.library(), &dir.join("backup.sqlite3"), options(), &BackupCancel::new())
        .await
        .expect("backup");
    assert_eq!(
        leftover.exists(),
        !insensitive,
        "case-insensitive: {insensitive}, {:?}",
        dir.scratch_names()
    );
}

/// A lock file that is there already, with content, is locked and left as it is: the protocol
/// never writes to it.
#[tokio::test(flavor = "multi_thread")]
async fn an_existing_lock_file_is_left_as_it_is() {
    let dir = Dir::new("lock-kept");
    let _source = library(&dir, 3, 0).await;
    std::fs::write(dir.join("backup.sqlite3.lock"), b"someone's notes").expect("lock file");
    backup_library(&dir.library(), &dir.join("backup.sqlite3"), options(), &BackupCancel::new())
        .await
        .expect("backup");
    assert_eq!(std::fs::read(dir.join("backup.sqlite3.lock")).expect("lock"), b"someone's notes");
}

/// A lock file with another name (a hard link) is refused before it is opened: closing a second
/// descriptor of a database's file would drop the locks SQLite holds on it in this process
/// (review C5-R5-02). Unix only, where those locks are POSIX locks.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn a_lock_file_with_another_name_is_refused() {
    let dir = Dir::new("lock-linked");
    let _source = library(&dir, 3, 0).await;
    std::fs::hard_link(dir.library(), dir.join("backup.sqlite3.lock")).expect("hard link");
    let error = backup_library(
        &dir.library(),
        &dir.join("backup.sqlite3"),
        options(),
        &BackupCancel::new(),
    )
    .await
    .expect_err("a lock file with another name is refused");
    assert!(error.to_string().contains("硬链接"), "{error}");
    assert_eq!(integrity(&dir.library()), "ok");
    assert!(dir.scratch_names().is_empty(), "{:?}", dir.scratch_names());
}

/// A leftover that cannot be removed (here a directory under a scratch file's name) stops the
/// backup before it creates a scratch file of its own, so failed attempts do not pile up copies.
#[tokio::test(flavor = "multi_thread")]
async fn a_leftover_that_cannot_be_removed_stops_the_backup() {
    let dir = Dir::new("stuck");
    let _source = library(&dir, 3, 0).await;
    let stuck = dir.join(&format!("backup.sqlite3.partial/partial-{NONCE}"));
    std::fs::create_dir_all(&stuck).expect("a directory where a scratch file would be");
    backup_library(&dir.library(), &dir.join("backup.sqlite3"), options(), &BackupCancel::new())
        .await
        .expect_err("the leftover cannot be removed");
    assert!(!dir.join("backup.sqlite3").exists());
    assert_eq!(
        dir.scratch_names(),
        ["backup.sqlite3.partial/", &format!("backup.sqlite3.partial/partial-{NONCE}")]
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
    std::fs::create_dir(dir.join("backup.sqlite3.partial")).expect("scratch directory");
    let scratch = dir.join(&format!("backup.sqlite3.partial/partial-{NONCE}"));
    std::fs::hard_link(&target, &scratch).expect("second name, as a killed backup leaves it");

    let error = backup_library(&dir.library(), &target, options(), &BackupCancel::new())
        .await
        .expect_err("the target exists");
    assert!(error.to_string().contains("已存在"), "{error}");
    assert!(dir.scratch_names().is_empty(), "{:?}", dir.scratch_names());
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
        "backup.sqlite3.partial".to_owned(),
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

/// A copy past its deadline is interrupted: one past it from the start, and one with 1 ms, which
/// passes while it prepares or copies about 10 MB (which of the two is not observed). No target,
/// no scratch file; the target can be backed up afterwards.
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

/// Dropping the future of a backup once its scratch file has content (writing began; whether the
/// copy is still running then is not observed) cancels it: its thread stops on its own, removes
/// its scratch file and directory, then releases the lock; nothing is published (review C5-R2-06,
/// C5-R3-04).
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
    assert!(dir.scratch_names().is_empty(), "{:?}", dir.scratch_names());
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
    while !dir.scratch_names().iter().any(|name| !name.ends_with('/')) {
        assert!(started.elapsed() < Duration::from_secs(30), "the copy never started");
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    cancel.cancel();
    let error = running.await.expect("task").expect_err("cancelled while copying");
    assert!(error.to_string().contains("已取消"), "{error}");
    assert!(!target.exists());
    assert!(dir.scratch_names().is_empty(), "{:?}", dir.scratch_names());
}

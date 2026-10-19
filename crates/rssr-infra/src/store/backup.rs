//! Backups of the library file (`rssr-cli backup`, phase C5).
//!
//! A backup is a consistent copy made with `VACUUM INTO` (one read snapshot, so writers in other
//! processes are neither blocked nor seen half way) into a scratch file next to the target, and
//! published under the target's name only if that name is still free. One thread does all of it,
//! on a connection of its own, and owns the lock and the scratch file from start to end: nothing
//! is released or removed while SQLite may still write. In the target's directory the protocol
//! never changes the content of a file it did not create in this run: it creates files and
//! directories only if missing, hard-links and unlinks names, and never writes to or truncates an
//! existing file (review C5-R4-01, C5-R4-02). (The library itself is read through SQLite, which
//! may checkpoint its WAL when the backup's connection closes; see Limits.)
//!
//! Precondition (decided 2026-10-19 after review C5-R5): the target's directory is under the sole
//! control of the user running the backup, and holds no aliases made on purpose — no hard link,
//! symbolic link or mount (a bind mount included) that makes a backup's lock file or scratch
//! directory another name of some other file or directory. Within it, every name the protocol uses
//! is the plain name it looks like. What the protocol checks beyond that (a lock file that is not a
//! regular file or has other names, a scratch directory name that is not a directory) is refused
//! as a safeguard, not a defence: it does not detect a bind mount, nor names replaced while it
//! runs.
//!
//! 1. The library is opened only if it exists and is a library: no creation, no v0.1 import.
//! 2. Backups of one target exclude each other through an advisory lock on `<target>.lock`
//!    (`flock` / `LockFileEx`), waited for a bounded time. The lock file must be a regular file,
//!    on Unix with no other name (checked before it is opened: closing a second descriptor of a
//!    database's file would drop the POSIX locks SQLite holds on it in this process, review
//!    C5-R5-02), opened without following a link (and on Unix without blocking on a FIFO), and is
//!    never written; it stays in place, since removing it would let a waiter lock a file the next
//!    backup no longer sees.
//! 3. Scratch files live in the directory `<target>.partial`, created when missing (owner-only on
//!    Unix) and refused when that name is anything but a directory (a link included). Holding the
//!    lock, the backup first removes the entries there named like scratch files: only a backup of
//!    this target that died can have left them (a running one holds the lock), including one
//!    killed after it published. Other entries stay; the scan reads every entry and checks for
//!    cancellation between them. Then a target that exists is refused.
//! 4. The scratch file `partial-<32 hex digits>` is created there only if missing (`VACUUM INTO`
//!    accepts an empty file), owner-only on Unix as the library is, filled, synced and checked to
//!    be a library. SQLite's `-journal`, `-wal` and `-shm` files of it count as scratch files.
//! 5. Cancellation is checked last before publication; past it the backup is published by a hard
//!    link to the target's name, which fails atomically if the name was taken meanwhile. A file
//!    system without hard links is reported, never answered with a rename that could replace a
//!    file. The scratch name is then removed, the scratch directory too if it is empty, and the
//!    target's directory synced; a failure of those is logged (a scratch directory that is not
//!    empty is expected and kept quietly), not reported, because the backup is already in place.
//!
//! Names. A backup's file name must end in `.sqlite3` (see [`is_target_name`]). That keeps it off
//! every name SQLite gives a database's companion files, where a backup would be taken as the
//! database's journal or shared memory and overwritten, and off the protocol's own names. Under
//! the precondition, two spellings of a target that the file system takes for one name (a
//! case-insensitive file system) take `<target>.lock` and `<target>.partial` for one name too: the
//! names share a directory and a stem and end in the same ASCII suffix, which case folding and
//! normalization leave as it is. So the lock held and the scratch directory cleaned belong to one
//! target (review C5-R2-02, C5-R3-02).
//!
//! Limits. The deadline applies to the copy statement only: SQLite's progress handler interrupts it
//! at the next check once the deadline has passed or the backup was cancelled. The checks come
//! every [`PROGRESS_OPS`] virtual machine instructions, so the deadline is cooperative: one
//! instruction copying a large record runs to its end first. The lock wait polls every 50 ms up to
//! its limit and checks for cancellation as it polls; opening the lock file and scheduling come on
//! top. A deadline or lock wait too long to represent is none, and neither bounds the whole
//! backup. Not interrupted by the deadline or by cancellation: SQLite sleeping while another
//! connection holds a lock (its busy timeout is a budget of sleep, not of elapsed time; that sleep
//! counts toward the deadline), closing the connection (which checkpoints the library's WAL when it
//! is the last connection to it, in any process), and the file operations around publication;
//! cancellation takes effect at the next check after them, the deadline not at all once the
//! statement has ended. While the copy reads its snapshot, other connections' writes collect in the
//! library's `-wal` file, which can be recycled only after the copy ends.
//!
//! Anything put into the scratch directory under a scratch file's name is taken for a leftover.
//! Each call runs on a thread of its own; the caller bounds how many run at once (the CLI starts
//! one).

use std::fs::{File, OpenOptions, TryLockError};
use std::hash::{BuildHasher, Hasher, RandomState};
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use rusqlite::{Connection, OpenFlags};
use tokio::sync::oneshot;

use super::connection::{self, Role};
use super::error::{StoreError, StoreResult};
use super::files;

/// How long a backup may copy, by default.
pub const DEFAULT_BACKUP_DEADLINE: Duration = Duration::from_secs(10 * 60);
/// How long a backup waits for another backup of the same target, by default.
pub const DEFAULT_BACKUP_LOCK_WAIT: Duration = Duration::from_secs(10);
/// SQLite calls the progress handler every this many virtual machine instructions.
pub const PROGRESS_OPS: i32 = 1_000;

#[derive(Debug, Clone, Copy)]
pub struct BackupOptions {
    /// Limit on the copy statement (cooperative, see the module documentation).
    pub deadline: Duration,
    /// Limit on waiting for another backup of the same target.
    pub lock_wait: Duration,
}

impl Default for BackupOptions {
    fn default() -> Self {
        Self { deadline: DEFAULT_BACKUP_DEADLINE, lock_wait: DEFAULT_BACKUP_LOCK_WAIT }
    }
}

/// Asks a running backup to stop. It stops at its next check and still reports what it did: a
/// backup cancelled after its last check is published and reported as such.
#[derive(Debug, Clone, Default)]
pub struct BackupCancel(Arc<AtomicBool>);

impl BackupCancel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// A finished backup.
#[derive(Debug, Clone)]
pub struct BackupReport {
    pub path: PathBuf,
    pub bytes: u64,
}

/// Writes a consistent copy of the library file `library` to `target`, which must not exist.
/// Dropping the returned future cancels the backup; its thread still cleans up after itself, and
/// the result is lost.
pub async fn backup_library(
    library: &Path,
    target: &Path,
    options: BackupOptions,
    cancel: &BackupCancel,
) -> StoreResult<BackupReport> {
    struct CancelOnDrop(Option<BackupCancel>);
    impl Drop for CancelOnDrop {
        fn drop(&mut self) {
            if let Some(cancel) = self.0.take() {
                cancel.cancel();
            }
        }
    }
    let library = std::path::absolute(library)?;
    let target = std::path::absolute(target)?;
    let (reply, receive) = oneshot::channel();
    let worker = cancel.clone();
    std::thread::Builder::new().name("rssr-backup".to_owned()).spawn(move || {
        let _ = reply.send(run(&library, &target, options, &worker));
    })?;
    let mut guard = CancelOnDrop(Some(cancel.clone()));
    let result = receive.await.map_err(|_| StoreError::TaskFailed)?;
    guard.0 = None;
    result
}

fn run(
    library: &Path,
    target: &Path,
    options: BackupOptions,
    cancel: &BackupCancel,
) -> StoreResult<BackupReport> {
    let parent = check_target(target)?;
    let lock = lock(&sibling(target, ".lock"), options.lock_wait, cancel, target)?;
    let scratch_dir = ScratchDir::open(sibling(target, ".partial"))?;
    scratch_dir.remove_leftovers(cancel, target)?;
    if target.symlink_metadata().is_ok() {
        return Err(exists(target));
    }
    stop_if_cancelled(cancel, target)?;
    let scratch = Scratch::create(scratch_dir.path.join(format!("partial-{}", nonce())))?;
    copy(library, &scratch.path, options.deadline, cancel, target)?;
    stop_if_cancelled(cancel, target)?;
    // Windows' `FlushFileBuffers` needs a handle that can write.
    let written = OpenOptions::new().write(true).open(&scratch.path)?;
    written.sync_all()?;
    let bytes = written.metadata()?.len();
    drop(written);
    files::check_library(&files::probe_uri(&scratch.path)?)?;
    stop_if_cancelled(cancel, target)?;
    match std::fs::hard_link(&scratch.path, target) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => return Err(exists(target)),
        Err(error) => {
            return Err(StoreError::Io(Error::new(
                error.kind(),
                format!(
                    "无法以硬链接发布备份（文件系统可能不支持硬链接），没有写入 {}：{error}",
                    target.display()
                ),
            )));
        }
    }
    // Published: nothing below can fail the backup, it only tidies up.
    drop(scratch);
    drop(scratch_dir);
    if let Err(error) = sync_dir(parent) {
        tracing::warn!(%error, dir = %parent.display(), "备份已写入，同步所在目录失败");
    }
    drop(lock);
    Ok(BackupReport { path: target.to_path_buf(), bytes })
}

/// The ending every backup's file name has.
const TARGET_SUFFIX: &str = ".sqlite3";
/// The suffixes SQLite gives a database's companion files.
const SQLITE_COMPANIONS: [&str; 3] = ["-journal", "-wal", "-shm"];

/// The target's directory; the target's file name must be a backup's name ([`is_target_name`]).
fn check_target(target: &Path) -> StoreResult<&Path> {
    let parent = target
        .parent()
        .ok_or_else(|| invalid(format!("备份目标没有所在目录：{}", target.display())))?;
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid(format!("备份文件名须是 UTF-8 文件名：{}", target.display())))?;
    if !is_target_name(name) {
        return Err(invalid(format!(
            "备份文件名须以 .sqlite3 结尾（例如 library-2026-10-19.sqlite3），其他名字留给备份自己的文件与 SQLite 的附属文件：{name}"
        )));
    }
    Ok(parent)
}

/// Whether `name` may name a backup: one plain file name (on Windows without `:`, which would name
/// a stream of another file) that ends in exactly `.sqlite3` after something else.
///
/// This one positive rule keeps a backup off every name that others claim, however the file
/// system folds case or normalizes names (review C5-R3-01). SQLite names a database's companion
/// files by appending `-journal`, `-wal`, `-shm` or `-mj` and nine hex digits; this protocol names
/// its own by appending `.lock` or `.partial`, and its scratch files in the latter `partial-` and
/// 32 hex digits, with SQLite's companion suffixes. Those endings are ASCII and each differs from
/// `.sqlite3` at a position counted from the end where the two characters are not one letter in
/// two cases: the last (`3` against `l`, `m` or `k`), or the eighth from the end (`.` against a
/// hex digit). Folding and normalization map each ASCII character to one character and equate
/// two ASCII characters only when they are one letter in two cases, so no name ending in
/// `.sqlite3` is equivalent to one of theirs.
fn is_target_name(name: &str) -> bool {
    let plain = !name.chars().any(|c| std::path::is_separator(c) || (cfg!(windows) && c == ':'));
    plain && name.len() > TARGET_SUFFIX.len() && name.ends_with(TARGET_SUFFIX)
}

/// Whether `name` is a scratch file's name: `partial-<32 lowercase hex digits>`, or that with one
/// of SQLite's companion suffixes.
fn is_scratch_name(name: &str) -> bool {
    let base =
        SQLITE_COMPANIONS.iter().find_map(|suffix| name.strip_suffix(suffix)).unwrap_or(name);
    base.strip_prefix("partial-").is_some_and(|nonce| {
        nonce.len() == 32 && nonce.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    })
}

/// Waits up to `wait` for the advisory lock on `path`, which must be a regular file.
fn lock(path: &Path, wait: Duration, cancel: &BackupCancel, target: &Path) -> StoreResult<File> {
    if let Ok(metadata) = path.symlink_metadata() {
        if !metadata.is_file() {
            return Err(not_regular(path));
        }
        #[cfg(unix)]
        if std::os::unix::fs::MetadataExt::nlink(&metadata) > 1 {
            return Err(invalid(format!("备份锁还有别的名字（硬链接）：{}", path.display())));
        }
    }
    let mut options = OpenOptions::new();
    options.create(true).truncate(false).write(true);
    // Whatever is at the name now is opened itself, not what a link there points to: a link put
    // in place after the check above is refused below (review C5-R2-04).
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::custom_flags(
        &mut options,
        libc::O_NONBLOCK | libc::O_NOFOLLOW,
    );
    #[cfg(windows)]
    std::os::windows::fs::OpenOptionsExt::custom_flags(&mut options, FILE_FLAG_OPEN_REPARSE_POINT);
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(not_regular(path));
    }
    let until = Instant::now().checked_add(wait);
    loop {
        stop_if_cancelled(cancel, target)?;
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(TryLockError::WouldBlock) if until.is_none_or(|until| Instant::now() < until) => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(TryLockError::WouldBlock) => {
                return Err(StoreError::Io(Error::new(
                    ErrorKind::TimedOut,
                    format!("另一个备份仍在写入同一目标：{}", target.display()),
                )));
            }
            Err(TryLockError::Error(error)) => return Err(error.into()),
        }
    }
}

/// The target's scratch directory, removed when dropped if it is empty (never with contents).
struct ScratchDir {
    path: PathBuf,
}

impl ScratchDir {
    /// Creates the directory `path` if missing (owner-only on Unix); a name that exists must be a
    /// directory, not a link to one.
    fn open(path: PathBuf) -> StoreResult<Self> {
        let mut builder = std::fs::DirBuilder::new();
        // One level only: an existing name is checked below, never created through.
        builder.recursive(false);
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        match builder.create(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                if !path.symlink_metadata()?.file_type().is_dir() {
                    return Err(invalid(format!("备份暂存目录不是目录：{}", path.display())));
                }
            }
            Err(error) => return Err(error.into()),
        }
        Ok(Self { path })
    }

    /// Removes the entries named like scratch files: only a backup of this target that died left
    /// them, since a running one holds the lock. Other entries stay.
    fn remove_leftovers(&self, cancel: &BackupCancel, target: &Path) -> StoreResult<()> {
        for entry in std::fs::read_dir(&self.path)? {
            stop_if_cancelled(cancel, target)?;
            let entry = entry?;
            if !entry.file_name().to_str().is_some_and(is_scratch_name) {
                continue;
            }
            match std::fs::remove_file(entry.path()) {
                Err(error) if error.kind() != ErrorKind::NotFound => return Err(error.into()),
                _ => {}
            }
        }
        Ok(())
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        // Fails while anything is left in it, which then stays for the next backup to remove.
        match std::fs::remove_dir(&self.path) {
            Err(error)
                if !matches!(error.kind(), ErrorKind::DirectoryNotEmpty | ErrorKind::NotFound) =>
            {
                tracing::warn!(%error, path = %self.path.display(), "无法删除备份暂存目录");
            }
            _ => {}
        }
    }
}

/// `VACUUM INTO` from the existing library file into `into`, on a connection of its own that
/// never creates the library.
fn copy(
    library: &Path,
    into: &Path,
    deadline: Duration,
    cancel: &BackupCancel,
    target: &Path,
) -> StoreResult<()> {
    files::check_library(&files::probe_uri(library)?)?;
    let into =
        into.to_str().ok_or_else(|| invalid(format!("备份路径须是 UTF-8：{}", into.display())))?;
    let connection = Connection::open_with_flags(
        library,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    connection::configure(&connection, Role::Writer)?;
    let until = Instant::now().checked_add(deadline);
    let stop = cancel.clone();
    connection.progress_handler(
        PROGRESS_OPS,
        Some(move || stop.is_cancelled() || until.is_some_and(|until| Instant::now() >= until)),
    )?;
    match connection.execute("VACUUM INTO ?1", [into]) {
        Ok(_) => Ok(()),
        Err(error) => {
            let error = StoreError::from(error);
            stop_if_cancelled(cancel, target)?;
            match error {
                StoreError::Interrupted => Err(StoreError::Io(Error::new(
                    ErrorKind::TimedOut,
                    format!("备份超过时限被中断，没有写入 {}", target.display()),
                ))),
                error => Err(error),
            }
        }
    }
}

fn stop_if_cancelled(cancel: &BackupCancel, target: &Path) -> StoreResult<()> {
    if cancel.is_cancelled() {
        return Err(StoreError::Io(Error::new(
            ErrorKind::Interrupted,
            format!("已取消备份，没有写入 {}", target.display()),
        )));
    }
    Ok(())
}

/// A scratch file of this backup, removed when dropped with SQLite's companion files of it (after
/// a publish its other name remains).
struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn create(path: PathBuf) -> std::io::Result<Self> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        options.open(&path)?;
        Ok(Self { path })
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let companions = SQLITE_COMPANIONS.iter().map(|suffix| sibling(&self.path, suffix));
        for path in std::iter::once(self.path.clone()).chain(companions) {
            match std::fs::remove_file(&path) {
                Err(error) if error.kind() != ErrorKind::NotFound => {
                    // Left in the scratch directory: the next backup of the target removes it.
                    tracing::warn!(%error, path = %path.display(), "无法删除备份暂存文件");
                }
                _ => {}
            }
        }
    }
}

/// `CreateFileW` opens a reparse point (a link) itself instead of its target.
#[cfg(windows)]
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

#[cfg(unix)]
fn sync_dir(dir: &Path) -> std::io::Result<()> {
    File::open(dir)?.sync_all()
}

/// Windows has no directory handle to sync; the link is as durable as the file system makes it.
#[cfg(not(unix))]
fn sync_dir(_dir: &Path) -> std::io::Result<()> {
    Ok(())
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

/// 32 lowercase hex digits from two SipHash outputs under std's random hasher keys (drawn from
/// the system's random source once per thread, then varied for each `RandomState`; a backup runs
/// on a new thread). Not a fresh 128-bit draw, but unpredictable; a collision fails `create_new`.
fn nonce() -> String {
    let half = || RandomState::new().build_hasher().finish();
    format!("{:016x}{:016x}", half(), half())
}

fn exists(target: &Path) -> StoreError {
    StoreError::Io(Error::new(
        ErrorKind::AlreadyExists,
        format!("备份目标已存在：{}", target.display()),
    ))
}

fn not_regular(path: &Path) -> StoreError {
    invalid(format!("备份锁不是普通文件：{}", path.display()))
}

fn invalid(message: String) -> StoreError {
    StoreError::Io(Error::new(ErrorKind::InvalidInput, message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backups_are_named_sqlite3_and_scratch_files_partial_hex() {
        for name in ["backup.sqlite3", "library-2026-10-19.sqlite3", "备份.sqlite3", "a.b.sqlite3"]
        {
            assert!(is_target_name(name), "{name}");
        }
        for name in [
            ".sqlite3",
            "backup.SQLITE3",
            "backup.sqlite3-wal",
            "library.sqlite3-\u{17f}hm",
            "backup.lock",
            "backup.sqlite3.partial",
            "backup.db",
            "a/b.sqlite3",
        ] {
            assert!(!is_target_name(name), "{name}");
        }
        assert_eq!(is_target_name("a:b.sqlite3"), !cfg!(windows));

        let hex = "0123456789abcdef0123456789abcdef";
        for suffix in ["", "-journal", "-wal", "-shm"] {
            assert!(is_scratch_name(&format!("partial-{hex}{suffix}")), "{suffix}");
        }
        for name in [
            "partial-kept".to_owned(),
            "partial-0123456789ABCDEF0123456789ABCDEF".to_owned(),
            format!("partial-{hex}-other"),
            format!("backup.sqlite3.partial-{hex}"),
            format!("x-partial-{hex}"),
            hex.to_owned(),
        ] {
            assert!(!is_scratch_name(&name), "{name}");
        }
        assert_eq!(nonce().len(), 32);
        assert!(is_scratch_name(&format!("partial-{}", nonce())));
    }
}

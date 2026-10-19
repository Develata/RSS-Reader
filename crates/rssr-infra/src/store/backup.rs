//! Backups of the library file (`rssr-cli backup`, phase C5).
//!
//! A backup is a consistent copy made with `VACUUM INTO` (one read snapshot, so writers in other
//! processes are neither blocked nor seen half way) into a scratch file next to the target, and
//! published under the target's name only if that name is still free. One thread does all of it,
//! on a connection of its own, and owns the lock and the scratch file from start to end: nothing
//! is released or removed while SQLite may still write.
//!
//! 1. The library is opened only if it exists and is a library: no creation, no v0.1 import.
//! 2. Backups of one target exclude each other through an advisory lock on `<target>.lock`
//!    (`flock` / `LockFileEx`), waited for a bounded time. The lock file must be a regular file,
//!    opened without following a link (and on Unix without blocking on a FIFO); it stays in
//!    place, since removing it would let a waiter lock a file the next backup no longer sees.
//! 3. Holding the lock, the scratch files of this target are removed first: only a backup that
//!    died can have left them (a running one holds the lock), including one killed after it
//!    published. Their names are `<owner>.partial-<32 hex digits>`, with SQLite's `-journal`,
//!    `-wal` or `-shm` after it, where `<owner>.lock` is the lock file this backup holds: the
//!    target's own name, or another spelling of it that the file system resolves to the same file
//!    (a case-insensitive one). Then a target that exists is refused.
//! 4. The scratch file is created only if missing (`VACUUM INTO` accepts an empty file), owner-only
//!    on Unix as the library is, filled, synced and checked to be a library.
//! 5. Cancellation is checked last before publication; past it the backup is published by a hard
//!    link to the target's name, which fails atomically if the name was taken meanwhile. A file
//!    system without hard links is reported, never answered with a rename that could replace a
//!    file. The scratch name is then removed and the directory synced; a failure of those two is
//!    logged, not reported, because the backup is already in place.
//!
//! Target names that a lock, a scratch file or SQLite would claim are refused, without regard to
//! case: names ending in `.lock`, `-journal`, `-wal` or `-shm`, and the scratch form. A backup
//! named like a database's companion file would be taken by that database as its journal or
//! shared memory and overwritten the next time it is opened.
//!
//! Limits. The copy statement is interrupted at SQLite's next progress check once the deadline
//! has passed or the backup was cancelled. The checks come every [`PROGRESS_OPS`] virtual machine
//! instructions, so the deadline is cooperative: one instruction copying a large record runs to its
//! end first. A deadline or lock wait too long to represent is none. Outside the deadline and
//! cancellation: the lock wait (bounded on its own), the scan of the target's directory for
//! leftovers (it checks for cancellation at each entry), SQLite waiting for another connection's
//! lock (up to its busy timeout), closing the connection (which checkpoints the library's WAL when
//! no other process has the library open), and the file operations around publication. While the
//! copy reads its snapshot, another process's writes collect in the library's `-wal` file, which
//! can be recycled only after the copy ends.
//!
//! The protocol assumes that only the user running the backup can change the target's directory:
//! it does not defend against another writer replacing names there while it runs. Each call runs
//! on a thread of its own; the caller bounds how many run at once (the CLI starts one).

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
    let (parent, name) = check_target(target)?;
    let lock = lock(&sibling(target, ".lock"), options.lock_wait, cancel, target)?;
    remove_leftovers(parent, name, &lock, cancel, target)?;
    if target.symlink_metadata().is_ok() {
        return Err(exists(target));
    }
    stop_if_cancelled(cancel, target)?;
    let scratch = Scratch::create(sibling(target, &format!(".partial-{}", nonce())))?;
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
    if let Err(error) = sync_dir(parent) {
        tracing::warn!(%error, dir = %parent.display(), "备份已写入，同步所在目录失败");
    }
    drop(lock);
    Ok(BackupReport { path: target.to_path_buf(), bytes })
}

/// The suffixes SQLite gives a database's companion files.
const SQLITE_COMPANIONS: [&str; 3] = ["-journal", "-wal", "-shm"];

/// The target's directory and file name; a name the protocol or SQLite claims is refused,
/// without regard to case (a case-insensitive file system resolves `X.LOCK` to `x.lock`).
fn check_target(target: &Path) -> StoreResult<(&Path, &str)> {
    let parent = target
        .parent()
        .ok_or_else(|| invalid(format!("备份目标没有所在目录：{}", target.display())))?;
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid(format!("备份文件名须是 UTF-8 文件名：{}", target.display())))?;
    let folded = name.to_lowercase();
    if [".lock"].iter().chain(&SQLITE_COMPANIONS).any(|suffix| folded.ends_with(suffix))
        || scratch_owner(&folded).is_some()
    {
        return Err(invalid(format!(
            "备份文件名不能以 .lock、-journal、-wal 或 -shm 结尾，也不能是 <名字>.partial-<32 位十六进制> 的形式，不分大小写（留给备份自己的文件与 SQLite 的附属文件）：{name}"
        )));
    }
    Ok((parent, name))
}

/// For a scratch file name of this protocol (`<owner>.partial-<32 hex>`, optionally followed by
/// `-journal`, `-wal` or `-shm`), the owner's name.
fn scratch_owner(name: &str) -> Option<&str> {
    let base =
        SQLITE_COMPANIONS.iter().find_map(|suffix| name.strip_suffix(suffix)).unwrap_or(name);
    let (owner, nonce) = base.rsplit_once(".partial-")?;
    let hex =
        nonce.len() == 32 && nonce.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'));
    (hex && !owner.is_empty()).then_some(owner)
}

/// Waits up to `wait` for the advisory lock on `path`, which must be a regular file.
fn lock(path: &Path, wait: Duration, cancel: &BackupCancel, target: &Path) -> StoreResult<File> {
    if let Ok(metadata) = path.symlink_metadata()
        && !metadata.is_file()
    {
        return Err(not_regular(path));
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

/// Removes the scratch files a backup which died left in `dir` for the target `name`: those
/// whose owner's lock file is `lock`, the one held. Another spelling of the name is the same
/// target only where the file system resolves both to one file; a case-sensitive one keeps the
/// scratch files of a target that differs only in case (review C5-R2-02).
fn remove_leftovers(
    dir: &Path,
    name: &str,
    lock: &File,
    cancel: &BackupCancel,
    target: &Path,
) -> StoreResult<()> {
    for entry in std::fs::read_dir(dir)? {
        stop_if_cancelled(cancel, target)?;
        let entry = entry?;
        let file_name = entry.file_name();
        let Some(owner) = file_name.to_str().and_then(scratch_owner) else {
            continue;
        };
        if owner != name && !is_held_lock(&dir.join(format!("{owner}.lock")), lock) {
            continue;
        }
        match std::fs::remove_file(entry.path()) {
            Err(error) if error.kind() != ErrorKind::NotFound => return Err(error.into()),
            _ => {}
        }
    }
    Ok(())
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

/// A scratch file of this backup, removed when dropped (after a publish its other name remains).
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
        match std::fs::remove_file(&self.path) {
            Err(error) if error.kind() != ErrorKind::NotFound => {
                // Removed by the next backup of the target, under its lock.
                tracing::warn!(%error, path = %self.path.display(), "无法删除备份暂存文件");
            }
            _ => {}
        }
    }
}

/// Whether `path` names the regular file `held` is open on. On Unix it is read from the directory
/// entry without opening it, so a FIFO cannot block; a link is never the lock.
fn is_held_lock(path: &Path, held: &File) -> bool {
    let Ok(metadata) = path.symlink_metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        held.metadata()
            .is_ok_and(|held| held.dev() == metadata.dev() && held.ino() == metadata.ino())
    }
    // std's file index is unstable on Windows; `same-file` reads it.
    #[cfg(windows)]
    {
        let held = held.try_clone().and_then(same_file::Handle::from_file);
        matches!((same_file::Handle::from_path(path), held), (Ok(other), Ok(held)) if other == held)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = held;
        false
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

fn sibling(target: &Path, suffix: &str) -> PathBuf {
    let mut name = target.as_os_str().to_os_string();
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
    fn only_this_protocols_scratch_names_have_an_owner() {
        let hex = "0123456789abcdef0123456789abcdef";
        for suffix in ["", "-journal", "-wal", "-shm"] {
            assert_eq!(
                scratch_owner(&format!("backup.db.partial-{hex}{suffix}")),
                Some("backup.db")
            );
        }
        for name in [
            "backup.db.partial-kept",
            "backup.db.partial-0123456789ABCDEF0123456789ABCDEF",
            "backup.db.partial-0123456789abcdef0123456789abcde",
            &format!(".partial-{hex}"),
            &format!("backup.db.partial-{hex}-other"),
            "backup.db",
        ] {
            assert_eq!(scratch_owner(name), None, "{name}");
        }
        assert_eq!(nonce().len(), 32);
        assert!(scratch_owner(&format!("x.partial-{}", nonce())).is_some());
    }
}

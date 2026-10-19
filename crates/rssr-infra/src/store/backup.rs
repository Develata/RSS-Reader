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
//!    (`flock` / `LockFileEx`), waited for a bounded time. The lock file must be a regular file
//!    (on Unix it is opened without following a link and without blocking on a FIFO); it stays in
//!    place, since removing it would let a waiter lock a file the next backup no longer sees.
//! 3. Holding the lock, the scratch files this protocol names are removed first: only a backup
//!    that died can have left them (a running one holds the lock), including one killed after it
//!    published. Their names are `<target>.partial-<32 hex digits>`, with SQLite's `-journal`,
//!    `-wal` or `-shm` after it; a target whose name has that form or ends in `.lock` is refused,
//!    so the removal never touches a backup. Then a target that exists is refused.
//! 4. The scratch file is created only if missing (`VACUUM INTO` accepts an empty file), owner-only
//!    on Unix as the library is, filled, synced and checked to be a library.
//! 5. Cancellation is checked last before publication; past it the backup is published by a hard
//!    link to the target's name, which fails atomically if the name was taken meanwhile. A file
//!    system without hard links is reported, never answered with a rename that could replace a
//!    file. The scratch name is then removed and the directory synced; a failure of those two is
//!    logged, not reported, because the backup is already in place.
//!
//! The copy statement is interrupted at SQLite's next progress check once the deadline has passed
//! or the backup was cancelled. The checks come every [`PROGRESS_OPS`] virtual machine
//! instructions, so the deadline is cooperative: one instruction copying a large record runs to its
//! end first. The lock wait has its own bound, and preparing and publishing are short file
//! operations that check for cancellation between steps but are outside the deadline.

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
/// Dropping the returned future cancels the backup; its thread still cleans up after itself.
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
    let _lock = lock(&sibling(target, ".lock"), options.lock_wait, cancel, target)?;
    remove_leftovers(parent, name, cancel, target)?;
    if target.symlink_metadata().is_ok() {
        return Err(exists(target));
    }
    stop_if_cancelled(cancel, target)?;
    let scratch = Scratch::create(sibling(target, &format!(".partial-{}", nonce())))?;
    copy(library, &scratch.path, options.deadline, cancel, target)?;
    OpenOptions::new().write(true).open(&scratch.path)?.sync_all()?;
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
    // Published: what follows only tidies up.
    drop(scratch);
    if let Err(error) = sync_dir(parent) {
        tracing::warn!(%error, dir = %parent.display(), "备份已写入，同步所在目录失败");
    }
    let bytes = std::fs::metadata(target)?.len();
    Ok(BackupReport { path: target.to_path_buf(), bytes })
}

/// The target's directory and file name; a name this protocol uses for its own files is refused.
fn check_target(target: &Path) -> StoreResult<(&Path, &str)> {
    let parent = target
        .parent()
        .ok_or_else(|| invalid(format!("备份目标没有所在目录：{}", target.display())))?;
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid(format!("备份文件名须是 UTF-8 文件名：{}", target.display())))?;
    if name.ends_with(".lock") || scratch_owner(name).is_some() {
        return Err(invalid(format!(
            "备份文件名不能以 .lock 结尾，也不能是 <名字>.partial-<32 位十六进制> 的形式（留给备份自己的文件）：{name}"
        )));
    }
    Ok((parent, name))
}

/// For a scratch file name of this protocol (`<owner>.partial-<32 hex>`, optionally followed by
/// `-journal`, `-wal` or `-shm`), the owner's name.
fn scratch_owner(name: &str) -> Option<&str> {
    let base = ["-journal", "-wal", "-shm"]
        .iter()
        .find_map(|suffix| name.strip_suffix(suffix))
        .unwrap_or(name);
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
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::custom_flags(
        &mut options,
        libc::O_NONBLOCK | libc::O_NOFOLLOW,
    );
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(not_regular(path));
    }
    let until = Instant::now() + wait;
    loop {
        stop_if_cancelled(cancel, target)?;
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(TryLockError::WouldBlock) if Instant::now() < until => {
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

/// Removes the scratch files of `name` that a backup which died left in `dir`.
fn remove_leftovers(
    dir: &Path,
    name: &str,
    cancel: &BackupCancel,
    target: &Path,
) -> StoreResult<()> {
    for entry in std::fs::read_dir(dir)? {
        stop_if_cancelled(cancel, target)?;
        let entry = entry?;
        let file_name = entry.file_name();
        if file_name.to_str().and_then(scratch_owner) != Some(name) {
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
    let until = Instant::now() + deadline;
    let stop = cancel.clone();
    connection.progress_handler(
        PROGRESS_OPS,
        Some(move || stop.is_cancelled() || Instant::now() >= until),
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

/// 128 bits from two randomly keyed hashers (std seeds each `RandomState` from the system's
/// random source), as 32 lowercase hex digits.
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

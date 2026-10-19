//! Backups of the library file (`rssr-cli backup`, phase C5).
//!
//! A backup is a consistent copy made with `VACUUM INTO` (one read snapshot, so writers in other
//! processes are neither blocked nor seen half way) into a scratch file next to the target, and
//! published under the target's name only if that name is still free:
//!
//! 1. Backups of one target exclude each other through an advisory lock on `<target>.lock`
//!    (`flock` / `LockFileEx`), waited for a bounded time. The lock file stays in place: removing
//!    it would let a waiter lock a file the next backup no longer sees.
//! 2. Holding the lock, a target that exists is refused, and scratch files `<target>.partial-*`
//!    are removed: only a backup that died can have left them, a running one holds the lock.
//! 3. The scratch file is created only if missing (`VACUUM INTO` accepts an empty file), owner-only
//!    on Unix as the library is, filled, synced and checked to be a library.
//! 4. It is published by a hard link to the target's name, which fails atomically if the name was
//!    taken meanwhile; a file system without hard links is reported, never answered with a rename
//!    that could replace a file. The scratch name is then removed and the directory synced.
//!
//! A backup that fails or is cancelled (its future dropped) removes its scratch file; one that
//! cannot (Windows keeps a file SQLite still has open) leaves it to the next backup of the target.
//! The copy runs on the store's writer without a transaction, with its own deadline
//! ([`LibraryStore::maintain`]).

use std::fs::{File, OpenOptions, TryLockError};
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::error::{StoreError, StoreResult};
use super::executor::{LibraryStore, on_thread};
use super::files;

/// How long a backup may run once it starts copying, by default.
pub const DEFAULT_BACKUP_DEADLINE: Duration = Duration::from_secs(10 * 60);
/// How long a backup waits for another backup of the same target, by default.
pub const DEFAULT_BACKUP_LOCK_WAIT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy)]
pub struct BackupOptions {
    /// Limit on the copy itself; a statement still running then is interrupted.
    pub deadline: Duration,
    /// Limit on waiting for another backup of the same target.
    pub lock_wait: Duration,
}

impl Default for BackupOptions {
    fn default() -> Self {
        Self { deadline: DEFAULT_BACKUP_DEADLINE, lock_wait: DEFAULT_BACKUP_LOCK_WAIT }
    }
}

/// A finished backup.
#[derive(Debug, Clone)]
pub struct BackupReport {
    pub path: PathBuf,
    pub bytes: u64,
}

/// Writes a consistent copy of the library behind `store` to `target`, which must not exist.
pub async fn backup_library(
    store: &LibraryStore,
    target: &Path,
    options: BackupOptions,
) -> StoreResult<BackupReport> {
    let target = std::path::absolute(target)?;
    let lock_wait = options.lock_wait;
    let prepared = on_thread(move |gave_up| Prepared::new(target, lock_wait, gave_up)).await?;
    let into = prepared
        .scratch
        .path
        .to_str()
        .ok_or_else(|| invalid(format!("备份路径不是 UTF-8：{}", prepared.scratch.path.display())))?
        .to_owned();
    store
        .maintain(options.deadline, move |connection| {
            connection.execute("VACUUM INTO ?1", [into])?;
            Ok(())
        })
        .await?;
    on_thread(move |_| prepared.publish()).await
}

/// The lock of a target, and the scratch file being filled for it.
struct Prepared {
    target: PathBuf,
    scratch: Scratch,
    _lock: File,
}

impl Prepared {
    fn new(target: PathBuf, lock_wait: Duration, gave_up: &AtomicBool) -> StoreResult<Self> {
        let parent = target
            .parent()
            .ok_or_else(|| invalid(format!("备份目标没有所在目录：{}", target.display())))?;
        let name = target
            .file_name()
            .ok_or_else(|| invalid(format!("备份目标不是文件名：{}", target.display())))?;
        let lock = lock(&sibling(&target, ".lock"), lock_wait, gave_up)?;
        if target.symlink_metadata().is_ok() {
            return Err(exists(&target));
        }
        let mut prefix = name.to_os_string();
        prefix.push(".partial-");
        for entry in std::fs::read_dir(parent)? {
            let entry = entry?;
            if entry.file_name().as_encoded_bytes().starts_with(prefix.as_encoded_bytes()) {
                match std::fs::remove_file(entry.path()) {
                    Err(error) if error.kind() != ErrorKind::NotFound => return Err(error.into()),
                    _ => {}
                }
            }
        }
        let path = sibling(&target, &format!(".partial-{}", nonce()));
        create_new_private(&path)?;
        Ok(Self { target, scratch: Scratch { path }, _lock: lock })
    }

    fn publish(self) -> StoreResult<BackupReport> {
        let scratch = &self.scratch.path;
        // Opened for writing: Windows flushes only a file handle that may write.
        OpenOptions::new().write(true).open(scratch)?.sync_all()?;
        files::check_library(&files::probe_uri(scratch)?)?;
        match std::fs::hard_link(scratch, &self.target) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                return Err(exists(&self.target));
            }
            Err(error) => {
                return Err(StoreError::Io(Error::new(
                    error.kind(),
                    format!(
                        "无法以硬链接发布备份（文件系统可能不支持硬链接，备份未写入 {}）：{error}",
                        self.target.display()
                    ),
                )));
            }
        }
        std::fs::remove_file(scratch)?;
        sync_dir(self.target.parent().unwrap_or(Path::new(".")))?;
        let bytes = std::fs::metadata(&self.target)?.len();
        Ok(BackupReport { path: self.target.clone(), bytes })
    }
}

/// A scratch file, removed when dropped (after a publish only its name is gone already).
struct Scratch {
    path: PathBuf,
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

/// Waits up to `wait` for the advisory lock on `path`, and stops waiting once `gave_up` is set.
fn lock(path: &Path, wait: Duration, gave_up: &AtomicBool) -> StoreResult<File> {
    let file = OpenOptions::new().create(true).truncate(false).write(true).open(path)?;
    let until = Instant::now() + wait;
    loop {
        if gave_up.load(Ordering::SeqCst) {
            return Err(StoreError::Interrupted);
        }
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(TryLockError::WouldBlock) if Instant::now() < until => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(TryLockError::WouldBlock) => {
                return Err(StoreError::Io(Error::new(
                    ErrorKind::TimedOut,
                    format!("另一个备份仍在写入同一目标：{}", path.display()),
                )));
            }
            Err(TryLockError::Error(error)) => return Err(error.into()),
        }
    }
}

fn create_new_private(path: &Path) -> std::io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path).map(drop)
}

#[cfg(unix)]
fn sync_dir(dir: &Path) -> std::io::Result<()> {
    File::open(dir)?.sync_all()
}

/// Windows has no directory handle to sync; the file system journals the link and the removal.
#[cfg(not(unix))]
fn sync_dir(_dir: &Path) -> std::io::Result<()> {
    Ok(())
}

fn sibling(target: &Path, suffix: &str) -> PathBuf {
    let mut name = target.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

fn nonce() -> String {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_nanos());
    format!("{}-{nanos:x}", std::process::id())
}

fn exists(target: &Path) -> StoreError {
    StoreError::Io(Error::new(
        ErrorKind::AlreadyExists,
        format!("备份目标已存在：{}", target.display()),
    ))
}

fn invalid(message: String) -> StoreError {
    StoreError::Io(Error::new(ErrorKind::InvalidInput, message))
}

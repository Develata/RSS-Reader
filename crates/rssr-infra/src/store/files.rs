//! File-level helpers for library files: identity probing and creating a new file.

use std::ffi::OsStr;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use std::time::{Duration, Instant};

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use rusqlite::{Connection, ErrorCode, OpenFlags};

use super::error::{StoreError, StoreResult};
use super::migrate;

/// Bytes kept as-is in a `file:` URI path; everything else is percent-encoded.
const URI_PATH: &AsciiSet =
    &NON_ALPHANUMERIC.remove(b'-').remove(b'.').remove(b'_').remove(b'~').remove(b'/').remove(b':');

/// The `file:` URI of `path` with `query`, so that SQLite and the file system agree on which file
/// it is (`?`, `#`, `%` and non-ASCII characters in the path are encoded).
pub fn sqlite_uri(path: &Path, query: &str) -> StoreResult<String> {
    let absolute = std::path::absolute(path)?;
    let text = absolute.to_str().ok_or_else(|| {
        StoreError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("路径不是有效的 UTF-8：{}", absolute.display()),
        ))
    })?;
    // Only Windows separates with backslashes; elsewhere a backslash is part of a name.
    let mut normalized = if cfg!(windows) { text.replace('\\', "/") } else { text.to_owned() };
    if !normalized.starts_with('/') {
        // `C:/x` becomes `/C:/x`, giving `file:///C:/x`.
        normalized.insert(0, '/');
    }
    Ok(format!("file://{}?{query}", utf8_percent_encode(&normalized, URI_PATH)))
}

/// The URI [`check_library`] reads the library at `path` through: read-only and `immutable=1`.
/// Fails for a path SQLite cannot be given as a URI (not UTF-8).
pub fn probe_uri(path: &Path) -> StoreResult<String> {
    sqlite_uri(path, "mode=ro&immutable=1")
}

/// How long a probe that looks corrupt is retried: a checkpoint by another process can be caught
/// midway, with page 1 already counting pages the file does not have yet.
const SETTLE: Duration = Duration::from_secs(2);

/// Whether the existing, non-empty file behind `probe` (from [`probe_uri`]) is a library.
///
/// The application id is read through SQLite with `immutable=1`: nothing is written, no `-wal` or
/// `-shm` file is created or changed, and SQLite's own file handling keeps the locks other
/// connections of this process hold on the file (closing a separate file descriptor would drop
/// them on POSIX systems). A library is published only once complete, header included (see
/// [`CreationLock::publish`]), and its application id never changes, so the main file alone
/// decides; anything else is not a library.
///
/// Without locks the read can meet a checkpoint in progress, which SQLite reports as corruption;
/// that is retried for [`SETTLE`] (a mitigation: nothing bounds a checkpoint's duration), then
/// reported as corruption, which refuses the open without writing. A file whose format is not
/// SQLite's is refused at once as not a library.
pub fn check_library(probe: &str) -> StoreResult<()> {
    let until = Instant::now() + SETTLE;
    loop {
        match read_application_id(probe) {
            Ok(application_id) if application_id == migrate::APPLICATION_ID => return Ok(()),
            Ok(_) => return Err(StoreError::NotALibrary),
            Err(error) => match error.sqlite_error_code() {
                Some(ErrorCode::DatabaseCorrupt) if Instant::now() < until => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Some(ErrorCode::NotADatabase) => return Err(StoreError::NotALibrary),
                _ => return Err(error.into()),
            },
        }
    }
}

fn read_application_id(probe: &str) -> rusqlite::Result<i32> {
    Connection::open_with_flags(
        probe,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?
    .query_row("PRAGMA application_id", [], |row| row.get(0))
}

/// `<name>.<suffix>` next to `path`.
fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().map(OsStr::to_os_string).unwrap_or_default();
    name.push(".");
    name.push(suffix);
    path.with_file_name(name)
}

/// Removes `path` and the side files SQLite may have left next to it; a file that is not there
/// is fine, any other failure is reported.
fn remove_with_side_files(path: &Path) -> std::io::Result<()> {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut name = path.as_os_str().to_os_string();
        name.push(suffix);
        match std::fs::remove_file(name) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
    }
    Ok(())
}

/// Scratch files a creator uses (`<name>.<label>`); see [`CreationLock::scratch`].
pub const SCRATCH_LABELS: [&str; 4] = ["creating", "importing", "import-index", "import-content"];

/// How long a creator waits for another creator of the same library.
const CREATION_WAIT: Duration = Duration::from_secs(120);

/// The exclusive right to create the library at a path: an advisory lock on `<name>.create-lock`
/// next to it (`flock` / `LockFileEx`), released when dropped or when the process ends.
///
/// Everything that creates a library (a new empty one, an import) holds it from checking that the
/// library is missing until it is published, so concurrent creators wait for each other instead of
/// racing, and the scratch files beside the target belong to the holder alone. The lock file stays
/// in place: removing it would let a waiter lock a file that later creators no longer see.
///
/// The guarantee holds among the programs that take this lock; a file put at the target by
/// anything else between the check and the rename is not protected (`rename` replaces it on most
/// platforms).
pub struct CreationLock {
    target: PathBuf,
    _file: std::fs::File,
}

impl CreationLock {
    /// Waits for the lock of `target` up to [`CREATION_WAIT`] (an import of a large library takes
    /// a while; a holder that hangs makes the others fail instead of waiting forever), and stops
    /// waiting once `gave_up` is set (whoever asked for the library went away).
    pub fn acquire(target: &Path, gave_up: &AtomicBool) -> StoreResult<Self> {
        Self::acquire_within(target, CREATION_WAIT, gave_up)
    }

    fn acquire_within(target: &Path, wait: Duration, gave_up: &AtomicBool) -> StoreResult<Self> {
        let file = Self::open_lock_file(target)?;
        let until = Instant::now() + wait;
        loop {
            if gave_up.load(Ordering::SeqCst) {
                return Err(StoreError::Interrupted);
            }
            match file.try_lock() {
                Ok(()) => return Ok(Self { target: target.to_path_buf(), _file: file }),
                Err(std::fs::TryLockError::WouldBlock) if Instant::now() < until => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(std::fs::TryLockError::WouldBlock) => {
                    return Err(StoreError::Io(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        format!("另一个程序仍在建立或导入资料库：{}", target.display()),
                    )));
                }
                Err(std::fs::TryLockError::Error(error)) => return Err(error.into()),
            }
        }
    }

    /// The lock of `target` if nobody holds it now.
    pub fn try_acquire(target: &Path) -> StoreResult<Option<Self>> {
        let file = Self::open_lock_file(target)?;
        match file.try_lock() {
            Ok(()) => Ok(Some(Self { target: target.to_path_buf(), _file: file })),
            Err(std::fs::TryLockError::WouldBlock) => Ok(None),
            Err(std::fs::TryLockError::Error(error)) => Err(error.into()),
        }
    }

    fn open_lock_file(target: &Path) -> std::io::Result<std::fs::File> {
        std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(sibling(target, "create-lock"))
    }

    /// The scratch file `<name>.<label>` next to the target, cleared of anything a process that
    /// died holding the lock left there. It borrows the lock, so it is removed before the lock is
    /// released.
    pub fn scratch(&self, label: &str) -> StoreResult<ScratchFile<'_>> {
        let path = sibling(&self.target, label);
        remove_with_side_files(&path)?;
        Ok(ScratchFile { path, _lock: PhantomData })
    }

    /// Removes every scratch file a creator that died after publishing (or before) left behind.
    pub fn remove_leftovers(&self) -> StoreResult<()> {
        for label in SCRATCH_LABELS {
            remove_with_side_files(&sibling(&self.target, label))?;
        }
        Ok(())
    }

    /// Publishes the finished, closed `scratch` file as the target, which must still be missing
    /// (holders of this lock are the only creators). The rename is atomic: other processes see no
    /// library or the complete one, never a partial file.
    pub fn publish(&self, scratch: ScratchFile<'_>) -> StoreResult<()> {
        if self.target.exists() {
            return Err(StoreError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("资料库已存在：{}", self.target.display()),
            )));
        }
        std::fs::rename(&scratch.path, &self.target)?;
        Ok(())
    }
}

/// A file next to a library being created, owned by the holder of its [`CreationLock`]; removed
/// with its SQLite side files when dropped (after a publish only the side files remain to remove).
pub struct ScratchFile<'lock> {
    path: PathBuf,
    _lock: PhantomData<&'lock CreationLock>,
}

impl ScratchFile<'_> {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ScratchFile<'_> {
    fn drop(&mut self) {
        if let Err(error) = remove_with_side_files(&self.path) {
            // Removed by the next creator holding the lock (`remove_leftovers`, `scratch`).
            tracing::warn!(%error, path = %self.path.display(), "无法删除资料库暂存文件");
        }
    }
}

/// Removes scratch files left next to an existing library by a creator that died, when no creator
/// is at work (best effort: a library on read-only media keeps whatever is there).
pub fn remove_stale_scratch(target: &Path) {
    let removed = CreationLock::try_acquire(target)
        .and_then(|lock| lock.map_or(Ok(()), |lock| lock.remove_leftovers()));
    if let Err(error) = removed {
        tracing::debug!(%error, "未能清理资料库旁的暂存文件");
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use super::*;

    fn temp_dir(label: &str) -> PathBuf {
        let nonce =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("rssr-files-{label} 中文-{nonce}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A library caught with page 1 counting more pages than the file has (simulated by cutting
    /// off the last page, as a checkpoint growing the file would show it) is accepted once the
    /// file catches up, and reported as corrupt if it never does.
    #[test]
    fn a_probe_that_meets_a_checkpoint_waits_for_it() {
        let dir = temp_dir("settle");
        let target = dir.join("library.sqlite3");
        {
            let mut conn = Connection::open(&target).unwrap();
            migrate::migrate(&mut conn, "2026-01-01T00:00:00Z").unwrap();
        }
        let probe = probe_uri(&target).unwrap();
        check_library(&probe).unwrap();
        let complete = std::fs::read(&target).unwrap();
        let page = 4096;
        assert!(complete.len() > page);
        std::fs::write(&target, &complete[..complete.len() - page]).unwrap();

        let finisher = {
            let (target, complete) = (target.clone(), complete.clone());
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(300));
                std::fs::write(&target, &complete).unwrap();
            })
        };
        check_library(&probe).expect("accepted once the file is complete");
        finisher.join().unwrap();

        std::fs::write(&target, &complete[..complete.len() - page]).unwrap();
        let started = Instant::now();
        assert!(matches!(check_library(&probe), Err(StoreError::Corrupt(_))));
        assert!(started.elapsed() >= SETTLE);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_backslash_is_part_of_a_unix_file_name() {
        let uri = sqlite_uri(Path::new("/data/a\\b.sqlite3"), "mode=ro").unwrap();
        assert_eq!(uri, "file:///data/a%5Cb.sqlite3?mode=ro");
    }

    #[test]
    fn creators_of_one_library_take_turns() {
        let dir = temp_dir("lock");
        let target = dir.join("library.sqlite3");
        let never = AtomicBool::new(false);
        let first = CreationLock::acquire(&target, &never).unwrap();
        assert!(CreationLock::try_acquire(&target).unwrap().is_none(), "held");
        let (acquired, receive) = std::sync::mpsc::channel();
        let (asking, wait_asking) = std::sync::mpsc::channel();
        let waiter = {
            let target = target.clone();
            std::thread::spawn(move || {
                asking.send(()).unwrap();
                let _second = CreationLock::acquire(&target, &AtomicBool::new(false)).unwrap();
                acquired.send(()).unwrap();
            })
        };
        wait_asking.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(receive.recv_timeout(Duration::from_millis(300)).is_err(), "second one waits");
        drop(first);
        receive.recv_timeout(Duration::from_secs(10)).expect("second one proceeds");
        waiter.join().unwrap();

        // A holder that never lets go makes the next creator fail instead of waiting forever, and
        // a creator whose caller gave up stops waiting. Both run where a hang cannot hold the
        // test.
        let holder = CreationLock::acquire(&target, &never).unwrap();
        let attempt = |wait: Duration, gave_up: Arc<AtomicBool>| {
            let target = target.clone();
            let (answer, receive) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let started = Instant::now();
                let result = CreationLock::acquire_within(&target, wait, &gave_up).map(drop);
                let _ = answer.send((result, started.elapsed()));
            });
            receive.recv_timeout(Duration::from_secs(10)).expect("the wait ends on its own")
        };
        let (refused, waited) =
            attempt(Duration::from_millis(200), Arc::new(AtomicBool::new(false)));
        assert!(
            matches!(refused, Err(StoreError::Io(ref error)) if error.kind() == std::io::ErrorKind::TimedOut)
        );
        assert!(waited >= Duration::from_millis(200));
        let (stopped, waited) = attempt(Duration::from_secs(60), Arc::new(AtomicBool::new(true)));
        assert!(matches!(stopped, Err(StoreError::Interrupted)));
        assert!(waited < Duration::from_secs(5));
        drop(holder);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn publishing_never_replaces_a_file_and_scratch_files_start_empty() {
        let dir = temp_dir("publish");
        let target = dir.join("library.sqlite3");
        let lock = CreationLock::acquire(&target, &AtomicBool::new(false)).unwrap();
        // Left behind by a holder that died.
        let stale = sibling(&target, "creating");
        std::fs::write(&stale, b"stale").unwrap();
        std::fs::write(dir.join("library.sqlite3.creating-journal"), b"stale").unwrap();

        let scratch = lock.scratch("creating").unwrap();
        assert!(!scratch.path().exists());
        assert!(!dir.join("library.sqlite3.creating-journal").exists());
        std::fs::write(scratch.path(), b"new").unwrap();
        std::fs::write(&target, b"other").unwrap();
        assert!(lock.publish(scratch).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"other");
        assert!(!stale.exists(), "a refused scratch file is removed");

        std::fs::remove_file(&target).unwrap();
        let scratch = lock.scratch("creating").unwrap();
        std::fs::write(scratch.path(), b"new").unwrap();
        lock.publish(scratch).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        assert!(!stale.exists());

        // Left behind by a creator that died after publishing: removed once nobody creates.
        for label in SCRATCH_LABELS {
            std::fs::write(sibling(&target, label), b"left").unwrap();
        }
        remove_stale_scratch(&target);
        assert!(sibling(&target, "importing").exists(), "kept while a creator holds the lock");
        drop(lock);
        remove_stale_scratch(&target);
        for label in SCRATCH_LABELS {
            assert!(!sibling(&target, label).exists(), "{label}");
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}

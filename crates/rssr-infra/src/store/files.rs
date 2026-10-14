//! File-level helpers for library files: identity probing and publishing a new file.

use std::path::Path;

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use rusqlite::{Connection, OpenFlags};

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
    let mut normalized = text.replace('\\', "/");
    if !normalized.starts_with('/') {
        // `C:/x` becomes `/C:/x`, giving `file:///C:/x`.
        normalized.insert(0, '/');
    }
    Ok(format!("file://{}?{query}", utf8_percent_encode(&normalized, URI_PATH)))
}

/// Whether the existing, non-empty file at `path` is a library.
///
/// The application id is read through SQLite with `immutable=1`: nothing is written, no `-wal` or
/// `-shm` file is created or changed, and SQLite's own file handling keeps the locks other
/// connections of this process hold on the file (closing a separate file descriptor would drop
/// them on POSIX systems). A library is published only once its header carries the id (see
/// [`publish_new`]), so the main file alone decides; anything else is not a library.
pub fn check_library(path: &Path) -> StoreResult<()> {
    let probe = Connection::open_with_flags(
        sqlite_uri(path, "mode=ro&immutable=1")?,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let application_id: i32 = probe
        .query_row("PRAGMA application_id", [], |row| row.get(0))
        .map_err(|error| match StoreError::from(error) {
            StoreError::Corrupt(_) => StoreError::NotALibrary,
            other => other,
        })?;
    if application_id == migrate::APPLICATION_ID { Ok(()) } else { Err(StoreError::NotALibrary) }
}

/// Publishes the finished file `from` as `to` unless `to` already exists; returns whether it was
/// published. `from` is removed either way.
///
/// A hard link never replaces an existing file, so two processes creating the same library (or
/// importing into it) cannot overwrite each other's result. On file systems without hard links
/// the file is renamed after checking that `to` is missing, which leaves a small race there.
pub fn publish_new(from: &Path, to: &Path) -> StoreResult<bool> {
    let published = match std::fs::hard_link(from, to) {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => false,
        Err(_) if !to.exists() => {
            std::fs::rename(from, to)?;
            return Ok(true);
        }
        Err(_) => false,
    };
    std::fs::remove_file(from)?;
    Ok(published)
}

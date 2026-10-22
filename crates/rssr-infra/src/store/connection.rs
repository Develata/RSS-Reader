//! Per-connection setup shared by every executor. Target-neutral: no threads, no clocks.

use std::time::Duration;

use rusqlite::Connection;
use rusqlite::functions::FunctionFlags;

use super::error::StoreResult;

/// How long a writer waits for another connection's (or another process's) write lock.
pub const BUSY_TIMEOUT: Duration = Duration::from_secs(30);

/// Role of a connection: readers cannot write, which keeps every write on the writer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Writer,
    Reader,
}

/// Applies the settings every library connection needs, before any statement runs.
///
/// - `foreign_keys`: bodies are removed with their entries (`ON DELETE CASCADE`).
/// - `busy_timeout`: the GUI, the CLI and the server may share one file.
/// - `fold_for_search`: the SQL spelling of [`rssr_domain::fold_for_search`].
/// - `listed_title`: [`rssr_domain::listed_title`] itself, on the stored text without copying it
///   whole. Not SQLite's `length`/`substr`: they stop at the first NUL, so a title with one could
///   pass uncut (review C3-R2 R2-01).
pub fn configure(connection: &Connection, role: Role) -> StoreResult<()> {
    connection.busy_timeout(BUSY_TIMEOUT)?;
    connection.pragma_update(None, "foreign_keys", true)?;
    if role == Role::Reader {
        connection.pragma_update(None, "query_only", true)?;
    }
    connection.create_scalar_function(
        "fold_for_search",
        1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |context| {
            let text: Option<String> = context.get(0)?;
            Ok(text.map(|text| rssr_domain::fold_for_search(&text)))
        },
    )?;
    connection.create_scalar_function(
        "listed_title",
        1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |context| {
            // The first `MAX + 1` characters decide the result and fit in `4 * (MAX + 1)` bytes:
            // only that prefix is decoded (review C3-R3 R3-04). Within it at least `MAX` whole
            // characters and something after them: a cut title keeps exactly the characters a
            // decoding of the whole text would.
            let window = 4 * (rssr_domain::MAX_LISTED_TITLE_CHARS + 1);
            Ok(match context.get_raw(0) {
                rusqlite::types::ValueRef::Text(bytes) | rusqlite::types::ValueRef::Blob(bytes) => {
                    let prefix = &bytes[..bytes.len().min(window)];
                    Some(rssr_domain::listed_title(&String::from_utf8_lossy(prefix)))
                }
                _ => None,
            })
        },
    )?;
    Ok(())
}

/// Switches a file database to WAL and returns the journal mode that is actually in effect:
/// SQLite reports the mode it kept instead of failing, for example on file systems without
/// shared memory (network shares, some sync folders).
#[cfg(not(target_arch = "wasm32"))]
pub fn enable_wal(connection: &Connection) -> StoreResult<String> {
    let mode: String = connection.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    Ok(mode.to_ascii_lowercase())
}

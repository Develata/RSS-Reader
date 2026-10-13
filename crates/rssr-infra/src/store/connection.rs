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
    Ok(())
}

/// Switches a file database to WAL and returns the journal mode that is actually in effect:
/// SQLite reports the mode it kept instead of failing, for example on file systems without
/// shared memory (network shares, some sync folders).
pub fn enable_wal(connection: &Connection) -> StoreResult<String> {
    let mode: String = connection.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    Ok(mode.to_ascii_lowercase())
}

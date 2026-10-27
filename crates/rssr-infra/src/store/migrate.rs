//! Library schema migrations. Target-neutral.
//!
//! Invariants:
//! - every library file carries [`APPLICATION_ID`]; any other non-empty file is refused, so a
//!   v0.1.x database is never mistaken for a library (it goes through the importer instead);
//! - the applied migrations are a prefix of [`MIGRATIONS`], each recorded with the checksum of
//!   its SQL after converting CRLF to LF, so Windows and Unix checkouts agree;
//! - all pending migrations and their history rows commit in one `BEGIN IMMEDIATE`
//!   transaction: a crash leaves the previous schema, and a second process waits for the write
//!   lock, then finds nothing left to do.

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use sha2::{Digest, Sha384};

use super::error::{StoreError, StoreResult};

/// `PRAGMA application_id` of a library file ("RSSR").
pub const APPLICATION_ID: i32 = 0x5253_5352;

pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

/// Ordered by version, starting at 1 without gaps. Migrations must not contain statements that
/// cannot run inside a transaction (`PRAGMA`, `VACUUM`, `ATTACH`, `BEGIN`, `COMMIT`).
pub const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "baseline",
    sql: include_str!("migrations/0001_baseline.sql"),
}];

/// SHA-384 of the SQL with CRLF converted to LF.
pub fn checksum(sql: &str) -> Vec<u8> {
    Sha384::digest(sql.replace("\r\n", "\n").as_bytes()).to_vec()
}

/// Brings the library to the latest schema. `now` is the RFC 3339 time recorded in the history.
pub fn migrate(connection: &mut Connection, now: &str) -> StoreResult<()> {
    migrate_with(connection, MIGRATIONS, now)
}

fn migrate_with(
    connection: &mut Connection,
    migrations: &[Migration],
    now: &str,
) -> StoreResult<()> {
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    claim_file(&tx)?;
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            checksum BLOB NOT NULL,
            applied_at TEXT NOT NULL
        )",
    )?;
    let applied: Vec<(i64, Vec<u8>)> = {
        let mut statement =
            tx.prepare("SELECT version, checksum FROM schema_migrations ORDER BY version")?;
        statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?.collect::<Result<_, _>>()?
    };
    for (index, (version, recorded)) in applied.iter().enumerate() {
        let Some(migration) = migrations.get(index) else {
            return Err(StoreError::NewerLibrary(*version));
        };
        if migration.version != *version {
            return Err(if *version > migration.version {
                StoreError::MigrationGap
            } else {
                StoreError::MigrationModified(*version)
            });
        }
        if checksum(migration.sql) != *recorded {
            return Err(StoreError::MigrationModified(*version));
        }
    }
    for migration in &migrations[applied.len()..] {
        tx.execute_batch(migration.sql)?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, checksum, applied_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![migration.version, migration.name, checksum(migration.sql), now],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// What an opened SQLite file is, as far as the library is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Identity {
    /// Carries [`APPLICATION_ID`].
    Library,
    /// No schema and no application id: a new file the library may claim.
    Empty,
}

/// Reads the file's identity without writing; any other SQLite file is [`StoreError::NotALibrary`].
pub fn identity(connection: &Connection) -> StoreResult<Identity> {
    let application_id: i32 =
        connection.query_row("PRAGMA application_id", [], |row| row.get(0))?;
    if application_id == APPLICATION_ID {
        return Ok(Identity::Library);
    }
    let has_schema = connection
        .query_row("SELECT 1 FROM sqlite_schema LIMIT 1", [], |_| Ok(()))
        .optional()?
        .is_some();
    if application_id != 0 || has_schema {
        return Err(StoreError::NotALibrary);
    }
    Ok(Identity::Empty)
}

/// Marks an empty file as a library, accepts a library, refuses anything else.
fn claim_file(connection: &Connection) -> StoreResult<()> {
    if identity(connection)? == Identity::Empty {
        connection.pragma_update(None, "application_id", APPLICATION_ID)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: &str = "2026-10-13T00:00:00Z";

    fn connection() -> Connection {
        let connection = Connection::open_in_memory().expect("open");
        super::super::connection::configure(&connection, super::super::connection::Role::Writer)
            .expect("configure");
        connection
    }

    fn applied(connection: &Connection) -> Vec<i64> {
        let mut statement =
            connection.prepare("SELECT version FROM schema_migrations ORDER BY version").unwrap();
        statement.query_map([], |row| row.get(0)).unwrap().collect::<Result<_, _>>().unwrap()
    }

    #[test]
    fn versions_start_at_one_without_gaps() {
        for (index, migration) in MIGRATIONS.iter().enumerate() {
            assert_eq!(migration.version, index as i64 + 1, "{}", migration.name);
        }
    }

    #[test]
    fn migrations_only_hold_transactional_statements() {
        for migration in MIGRATIONS {
            let code: String = migration
                .sql
                .lines()
                .map(|line| line.split("--").next().unwrap_or_default())
                .collect::<Vec<_>>()
                .join("\n")
                .to_ascii_uppercase();
            for statement in code.split(';') {
                let first = statement.split_whitespace().next().unwrap_or_default();
                assert!(
                    !matches!(
                        first,
                        "PRAGMA" | "VACUUM" | "ATTACH" | "DETACH" | "BEGIN" | "COMMIT"
                    ),
                    "{} starts a statement with {first}",
                    migration.name
                );
            }
        }
    }

    #[test]
    fn empty_file_becomes_a_library_and_reopening_is_a_no_op() {
        let mut connection = connection();
        migrate(&mut connection, NOW).unwrap();
        let application_id: i32 =
            connection.query_row("PRAGMA application_id", [], |row| row.get(0)).unwrap();
        assert_eq!(application_id, APPLICATION_ID);
        assert_eq!(applied(&connection), [1]);
        migrate(&mut connection, "2026-10-14T00:00:00Z").unwrap();
        let recorded_at: String = connection
            .query_row("SELECT applied_at FROM schema_migrations WHERE version = 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(recorded_at, NOW);
    }

    #[test]
    fn checksum_ignores_line_endings_but_not_content() {
        let sql = MIGRATIONS[0].sql;
        assert_eq!(checksum(sql), checksum(&sql.replace("\r\n", "\n").replace('\n', "\r\n")));
        assert_ne!(checksum(sql), checksum(&format!("{sql}\n-- edited")));
    }

    #[test]
    fn a_changed_recorded_migration_is_refused() {
        let mut connection = connection();
        migrate(&mut connection, NOW).unwrap();
        connection
            .execute("UPDATE schema_migrations SET checksum = x'00' WHERE version = 1", [])
            .unwrap();
        assert!(matches!(migrate(&mut connection, NOW), Err(StoreError::MigrationModified(1))));
    }

    #[test]
    fn a_library_from_a_newer_program_is_refused() {
        let mut connection = connection();
        migrate(&mut connection, NOW).unwrap();
        let next = MIGRATIONS.len() as i64 + 1;
        connection
            .execute(
                "INSERT INTO schema_migrations (version, name, checksum, applied_at)
                 VALUES (?1, 'future', x'00', ?2)",
                params![next, NOW],
            )
            .unwrap();
        assert!(
            matches!(migrate(&mut connection, NOW), Err(StoreError::NewerLibrary(v)) if v == next)
        );
    }

    /// A v0.1.x database (or any other SQLite file) has tables but not the library's id.
    #[test]
    fn a_foreign_database_is_refused_and_left_unchanged() {
        let mut connection = connection();
        connection.execute_batch("CREATE TABLE feeds (id INTEGER PRIMARY KEY)").unwrap();
        assert!(matches!(migrate(&mut connection, NOW), Err(StoreError::NotALibrary)));
        let application_id: i32 =
            connection.query_row("PRAGMA application_id", [], |row| row.get(0)).unwrap();
        assert_eq!(application_id, 0);
        let tables: i64 = connection
            .query_row("SELECT count(*) FROM sqlite_schema WHERE type = 'table'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(tables, 1, "no history table was added");
    }

    /// A failing migration leaves neither the earlier ones, their history nor the library id.
    #[test]
    fn a_failed_migration_rolls_back_everything() {
        let mut connection = connection();
        let broken = [
            Migration { version: 1, name: "baseline", sql: MIGRATIONS[0].sql },
            Migration {
                version: 2,
                name: "broken",
                sql: "CREATE TABLE ok (id INTEGER); CREATE TABLE (",
            },
        ];
        assert!(migrate_with(&mut connection, &broken, NOW).is_err());
        let application_id: i32 =
            connection.query_row("PRAGMA application_id", [], |row| row.get(0)).unwrap();
        let objects: i64 = connection
            .query_row("SELECT count(*) FROM sqlite_schema", [], |row| row.get(0))
            .unwrap();
        assert_eq!((application_id, objects), (0, 0));
    }

    #[test]
    fn baseline_enforces_ticket_order_and_cascades_bodies() {
        let mut connection = connection();
        migrate(&mut connection, NOW).unwrap();
        connection
            .execute_batch(
                "INSERT INTO feeds (id, url, created_at, updated_at) VALUES (1, 'u', 't', 't');
                 INSERT INTO entries (id, feed_id, external_id, dedup_key, title, title_search,
                     first_seen_at, created_at, updated_at)
                 VALUES (1, 1, 'e', 'e', 'T', 't', 't', 't', 't');
                 INSERT INTO entry_contents (entry_id, content_html, updated_at)
                 VALUES (1, '<p>x</p>', 't');",
            )
            .unwrap();
        let out_of_order = connection.execute(
            "UPDATE feeds SET refresh_published = 1, refresh_diagnosed = 1 WHERE id = 1",
            [],
        );
        assert!(matches!(out_of_order.map_err(StoreError::from), Err(StoreError::Conflict(_))));
        connection.execute("DELETE FROM entries WHERE id = 1", []).unwrap();
        let bodies: i64 = connection
            .query_row("SELECT count(*) FROM entry_contents", [], |row| row.get(0))
            .unwrap();
        assert_eq!(bodies, 0);
    }
}

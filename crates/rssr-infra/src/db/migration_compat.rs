//! SQLx hashes physical SQL bytes. Accept the two historical checkout newline
//! forms without rewriting migration history or accepting changed SQL content.
use sha2::{Digest, Sha384};
use sqlx::migrate::{AppliedMigration, Migrate, MigrateError, Migration, Migrator};

use super::SqlitePool;

pub(super) async fn run(pool: &SqlitePool, source: &Migrator) -> Result<(), MigrateError> {
    // NativeSqliteBackend holds its existing file migration lock around this
    // whole call. Keep inspection and SQLx execution on the same connection.
    let mut connection = pool.acquire().await?;
    connection.ensure_migrations_table().await?;
    if let Some(version) = connection.dirty_version().await? {
        return Err(MigrateError::Dirty(version));
    }
    let applied = connection.list_applied_migrations().await?;
    compatible_migrator(source, &applied)?.run_direct(&mut *connection).await
}

fn compatible_migrator(
    source: &Migrator,
    applied: &[AppliedMigration],
) -> Result<Migrator, MigrateError> {
    let mut migrations: Vec<_> = source
        .iter()
        .map(|migration| {
            let sql = if migration.sql.contains("\r\n") {
                migration.sql.replace("\r\n", "\n").into()
            } else {
                migration.sql.clone()
            };
            Migration::new(
                migration.version,
                migration.description.clone(),
                migration.migration_type,
                sql,
                migration.no_tx,
            )
        })
        .collect();

    // Validate every applied version before SQLx can execute any pending SQL.
    // Only the in-memory expected checksum changes; the stored history remains
    // byte-for-byte intact, including installed_on and execution_time.
    for installed in applied {
        let Some(migration) = migrations.iter_mut().find(|migration| {
            migration.version == installed.version && !migration.migration_type.is_down_migration()
        }) else {
            if source.ignore_missing {
                continue;
            }
            return Err(MigrateError::VersionMissing(installed.version));
        };
        if migration.checksum == installed.checksum {
            continue;
        }
        let crlf = migration.sql.replace('\n', "\r\n");
        if Sha384::digest(crlf.as_bytes())[..] != installed.checksum[..] {
            return Err(MigrateError::VersionMismatch(installed.version));
        }
        migration.checksum = installed.checksum.clone();
    }

    Ok(Migrator {
        migrations: migrations.into(),
        ignore_missing: source.ignore_missing,
        locking: source.locking,
        no_tx: source.no_tx,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{CONTENT_MIGRATOR, MIGRATOR, create_sqlite_pool};

    fn variant(source: &Migrator, crlf: bool, last: i64) -> Migrator {
        Migrator {
            migrations: source
                .iter()
                .filter(|m| m.version <= last)
                .map(|m| {
                    let lf = m.sql.replace("\r\n", "\n");
                    Migration::new(
                        m.version,
                        m.description.clone(),
                        m.migration_type,
                        if crlf { lf.replace('\n', "\r\n") } else { lf }.into(),
                        m.no_tx,
                    )
                })
                .collect(),
            ..Migrator::DEFAULT
        }
    }

    #[tokio::test]
    async fn fresh_databases_use_lf_checksums_for_both_build_inputs_and_stores() {
        for source in [&MIGRATOR, &CONTENT_MIGRATOR] {
            for crlf in [false, true] {
                let pool = create_sqlite_pool("sqlite::memory:").await.unwrap();
                run(&pool, &variant(source, crlf, i64::MAX)).await.unwrap();
                let stored: Vec<(i64, Vec<u8>)> = sqlx::query_as(
                    "SELECT version, checksum FROM _sqlx_migrations ORDER BY version",
                )
                .fetch_all(&pool)
                .await
                .unwrap();
                let canonical = variant(source, false, i64::MAX);
                let expected: Vec<_> =
                    canonical.iter().map(|m| (m.version, m.checksum.to_vec())).collect();
                assert_eq!(stored, expected);
                run(&pool, &variant(source, !crlf, i64::MAX)).await.unwrap();
                pool.close().await;
            }
        }
    }

    #[tokio::test]
    async fn strict_sqlx_reproduces_the_mismatch_but_compatibility_preserves_history() {
        for source in [&MIGRATOR, &CONTENT_MIGRATOR] {
            let pool = create_sqlite_pool("sqlite::memory:").await.unwrap();
            variant(source, true, i64::MAX).run(&pool).await.unwrap();
            assert!(matches!(
                variant(source, false, i64::MAX).run(&pool).await,
                Err(MigrateError::VersionMismatch(1))
            ));
            let before = history(&pool).await;
            run(&pool, source).await.unwrap();
            assert_eq!(history(&pool).await, before);
            // The original CRLF binary can still reopen this fully migrated DB.
            variant(source, true, i64::MAX).run(&pool).await.unwrap();
            pool.close().await;
        }
    }

    #[tokio::test]
    async fn changed_sql_even_a_comment_is_not_a_newline_variant() {
        let pool = create_sqlite_pool("sqlite::memory:").await.unwrap();
        let mut legacy = variant(&MIGRATOR, false, 7);
        let migration = &mut legacy.migrations.to_mut()[1];
        *migration = Migration::new(
            migration.version,
            migration.description.clone(),
            migration.migration_type,
            format!("{}\n-- changed script\n", migration.sql).into(),
            migration.no_tx,
        );
        legacy.run(&pool).await.unwrap();
        let before = history(&pool).await;
        assert!(matches!(run(&pool, &MIGRATOR).await, Err(MigrateError::VersionMismatch(2))));
        assert_unchanged_before_workspace_upgrade(&pool, before).await;
        pool.close().await;
    }

    #[tokio::test]
    async fn corrupt_dirty_and_unknown_histories_still_fail_before_pending_sql() {
        for case in ["checksum", "dirty", "unknown"] {
            let pool = create_sqlite_pool("sqlite::memory:").await.unwrap();
            variant(&MIGRATOR, true, 7).run(&pool).await.unwrap();
            let sql = match case {
                "checksum" => "UPDATE _sqlx_migrations SET checksum = X'00' WHERE version = 7",
                "dirty" => "UPDATE _sqlx_migrations SET success = false WHERE version = 7",
                _ => {
                    "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) VALUES (999, 'unknown', true, X'00', 0)"
                }
            };
            sqlx::query(sql).execute(&pool).await.unwrap();
            let before = history(&pool).await;
            let error = run(&pool, &MIGRATOR).await.unwrap_err();
            assert!(matches!(
                (case, error),
                ("checksum", MigrateError::VersionMismatch(7))
                    | ("dirty", MigrateError::Dirty(7))
                    | ("unknown", MigrateError::VersionMissing(999))
            ));
            assert_unchanged_before_workspace_upgrade(&pool, before).await;
            pool.close().await;
        }
    }

    async fn history(pool: &SqlitePool) -> Vec<(i64, String, String, bool, Vec<u8>, i64)> {
        sqlx::query_as("SELECT version, description, installed_on, success, checksum, execution_time FROM _sqlx_migrations ORDER BY version")
            .fetch_all(pool).await.unwrap()
    }

    async fn assert_unchanged_before_workspace_upgrade(
        pool: &SqlitePool,
        before: Vec<(i64, String, String, bool, Vec<u8>, i64)>,
    ) {
        assert_eq!(history(pool).await, before);
        let archived_column: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('entries') WHERE name = 'is_archived'",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(archived_column, 0, "rejection must not apply migration 8");
    }
}

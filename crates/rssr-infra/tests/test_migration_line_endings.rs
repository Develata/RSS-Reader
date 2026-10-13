#![cfg(not(target_arch = "wasm32"))]

//! 迁移校验和不随换行变化（审计 G1）。
//!
//! sqlx 把迁移文件的原始字节嵌进程序并据此计算校验和：Windows 检出通常是 CRLF，其余平台
//! 是 LF。同一个库要能在两类构建之间打开；内容真正被改过的迁移仍要拒绝。测试不依赖本机
//! 检出的换行：总是把库里的记录改成“另一种”换行的校验和。

use std::time::{SystemTime, UNIX_EPOCH};

use rssr_infra::db::{
    CONTENT_MIGRATOR, MIGRATOR, SqlitePool, migrate, migrate_content,
    sqlite_native::NativeSqliteBackend, storage_backend::StorageBackend,
};
use sha2::{Digest, Sha384};
use sqlx::migrate::Migrator;

#[derive(Clone, Copy)]
enum Database {
    Index,
    Content,
}

impl Database {
    fn migrator(self) -> &'static Migrator {
        match self {
            Self::Index => &MIGRATOR,
            Self::Content => &CONTENT_MIGRATOR,
        }
    }

    async fn migrate(self, pool: &SqlitePool) -> anyhow::Result<()> {
        match self {
            Self::Index => migrate(pool).await,
            Self::Content => migrate_content(pool).await,
        }
    }
}

/// 同一份 SQL 换成另一种换行后的校验和。
fn other_line_ending_checksum(sql: &str, embedded: &[u8]) -> Vec<u8> {
    let lf = sql.replace("\r\n", "\n");
    let crlf = lf.replace('\n', "\r\n");
    [lf, crlf]
        .iter()
        .map(|variant| Sha384::digest(variant.as_bytes()).to_vec())
        .find(|checksum| checksum.as_slice() != embedded)
        .expect("LF and CRLF hash differently")
}

async fn recorded(pool: &SqlitePool) -> Vec<(i64, Vec<u8>)> {
    sqlx::query_as("SELECT version, checksum FROM _sqlx_migrations ORDER BY version")
        .fetch_all(pool)
        .await
        .expect("read recorded checksums")
}

async fn set_recorded(pool: &SqlitePool, version: i64, checksum: &[u8]) {
    sqlx::query("UPDATE _sqlx_migrations SET checksum = ?1 WHERE version = ?2")
        .bind(checksum)
        .bind(version)
        .execute(pool)
        .await
        .expect("rewrite recorded checksum");
}

async fn check(database: Database) {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_nanos();
    let base_dir = std::env::temp_dir().join(format!("rssr-migration-line-endings-{nonce}"));
    let backend = NativeSqliteBackend::with_path(base_dir.join("rss-reader.db"));
    let pool = match database {
        Database::Index => backend.connect().await.expect("connect index db"),
        Database::Content => backend.connect_content().await.expect("connect content db"),
    };
    database.migrate(&pool).await.expect("first migration");

    // 模拟另一类构建建的库：每条记录都是另一种换行的校验和。
    let other: Vec<(i64, Vec<u8>)> = database
        .migrator()
        .iter()
        .map(|migration| {
            (
                migration.version,
                other_line_ending_checksum(migration.sql.as_str(), &migration.checksum),
            )
        })
        .collect();
    for (version, checksum) in &other {
        set_recorded(&pool, *version, checksum).await;
    }

    database.migrate(&pool).await.expect("a database from the other line ending opens");
    assert_eq!(recorded(&pool).await, other, "the recorded checksums are left as they were");

    // 内容真正被改过的迁移仍然拒绝。
    let (first_version, _) = other[0];
    set_recorded(&pool, first_version, &Sha384::digest(b"-- changed")).await;
    let error = database.migrate(&pool).await.expect_err("a changed migration is rejected");
    assert!(format!("{error:#}").contains("previously applied but has been modified"), "{error:#}");

    pool.close().await;
    let _ = std::fs::remove_dir_all(base_dir);
}

#[tokio::test]
async fn the_index_database_opens_across_line_endings_but_not_after_changes() {
    check(Database::Index).await;
}

#[tokio::test]
async fn the_content_database_opens_across_line_endings_but_not_after_changes() {
    check(Database::Content).await;
}

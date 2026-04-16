#![cfg(not(target_arch = "wasm32"))]

use rssr_infra::db::{
    migrate, migrate_content, sqlite_native::NativeSqliteBackend, storage_backend::StorageBackend,
};
use sqlx::Row;
use std::time::{SystemTime, UNIX_EPOCH};

#[tokio::test]
async fn sqlite_backend_can_bootstrap_schema() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    let base_dir = std::env::temp_dir().join(format!("rssr-bootstrap-{nonce}"));
    let database_path = base_dir.join("rss-reader.db");
    let backend = NativeSqliteBackend::with_path(&database_path);
    let pool = backend.connect().await.expect("connect sqlite file");
    let content_pool = backend.connect_content().await.expect("connect content sqlite file");

    backend.migrate(&pool).await.expect("run index migrations");
    backend.migrate_content(&content_pool).await.expect("run content migrations");
    migrate(&pool).await.expect("index migrate is idempotent");
    migrate_content(&content_pool).await.expect("content migrate is idempotent");

    let feed_count: i64 = sqlx::query("SELECT COUNT(*) AS count FROM feeds")
        .fetch_one(&pool)
        .await
        .expect("query feeds")
        .get("count");

    let entry_count: i64 = sqlx::query("SELECT COUNT(*) AS count FROM entries")
        .fetch_one(&pool)
        .await
        .expect("query entries")
        .get("count");
    let content_count: i64 = sqlx::query("SELECT COUNT(*) AS count FROM entry_contents")
        .fetch_one(&content_pool)
        .await
        .expect("query entry contents")
        .get("count");

    assert_eq!(feed_count, 0);
    assert_eq!(entry_count, 0);
    assert_eq!(content_count, 0);
    assert!(database_path.exists());
    let content_database = base_dir.join("rss-reader-content.db");
    assert!(content_database.exists());

    pool.close().await;
    content_pool.close().await;
    let _ = std::fs::remove_file(&database_path);
    let _ = std::fs::remove_file(format!("{}-wal", database_path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", database_path.display()));
    let _ = std::fs::remove_file(&content_database);
    let _ = std::fs::remove_file(format!("{}-wal", content_database.display()));
    let _ = std::fs::remove_file(format!("{}-shm", content_database.display()));
    let _ = std::fs::remove_dir_all(base_dir);
}

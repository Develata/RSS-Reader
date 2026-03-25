use rssr_infra::db::{sqlite_web::WebSqliteBackend, storage_backend::StorageBackend};
use sqlx::Row;
use tokio::fs;

#[tokio::test]
async fn web_backend_default_path_is_persistent_and_bootstraps() {
    let backend = WebSqliteBackend::default();
    assert_ne!(backend.database_url(), "sqlite::memory:");

    let pool = backend.connect().await.expect("connect sqlite memory");

    backend.migrate(&pool).await.expect("run migrations");

    let feed_count: i64 = sqlx::query("SELECT COUNT(*) AS count FROM feeds")
        .fetch_one(&pool)
        .await
        .expect("query feeds")
        .get("count");

    assert_eq!(feed_count, 0);

    let database_path: String = sqlx::query("PRAGMA database_list")
        .fetch_one(&pool)
        .await
        .expect("query database_list")
        .get("file");

    assert!(
        database_path.ends_with("rss-reader-web.db"),
        "unexpected database path: {database_path}"
    );

    pool.close().await;

    let _ = fs::remove_file(&database_path).await;
    let _ = fs::remove_file(format!("{database_path}-wal")).await;
    let _ = fs::remove_file(format!("{database_path}-shm")).await;
}

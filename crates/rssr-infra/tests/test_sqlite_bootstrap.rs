use rssr_infra::db::{
    migrate, sqlite_native::NativeSqliteBackend, storage_backend::StorageBackend,
};
use sqlx::Row;

#[tokio::test]
async fn sqlite_backend_can_bootstrap_schema() {
    let backend = NativeSqliteBackend::new("sqlite::memory:");
    let pool = backend.connect().await.expect("connect sqlite memory");

    backend.migrate(&pool).await.expect("run migrations");
    migrate(&pool).await.expect("migrate is idempotent");

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

    assert_eq!(feed_count, 0);
    assert_eq!(entry_count, 0);
}

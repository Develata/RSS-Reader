use rssr_infra::db::{sqlite_web::WebSqliteBackend, storage_backend::StorageBackend};
use sqlx::Row;

#[tokio::test]
async fn web_backend_bootstrap_path_is_not_an_immediate_error() {
    let backend = WebSqliteBackend::default();
    let pool = backend.connect().await.expect("connect sqlite memory");

    backend.migrate(&pool).await.expect("run migrations");

    let feed_count: i64 = sqlx::query("SELECT COUNT(*) AS count FROM feeds")
        .fetch_one(&pool)
        .await
        .expect("query feeds")
        .get("count");

    assert_eq!(feed_count, 0);
}

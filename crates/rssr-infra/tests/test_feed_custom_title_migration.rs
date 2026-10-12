#![cfg(not(target_arch = "wasm32"))]

//! 迁移 0009 把指定名称与源标题分开（审计 B6）。升级前的 `title` 一律当作源标题：不复制进
//! `custom_title`，以后继续跟随订阅源改名（不回填旧名称是有意的决定，见交接记录）。

use futures_util::future::BoxFuture;
use rssr_domain::FeedRepository;
use rssr_infra::{
    db::{
        MIGRATOR, feed_repository::SqliteFeedRepository, migrate,
        sqlite_native::NativeSqliteBackend, storage_backend::StorageBackend,
    },
    parser::ParsedFeed,
};
use sqlx::{
    error::BoxDynError,
    migrate::{Migration, MigrationSource, Migrator},
};

#[derive(Debug)]
struct Migrations(Vec<Migration>);

impl MigrationSource<'static> for Migrations {
    fn resolve(self) -> BoxFuture<'static, Result<Vec<Migration>, BoxDynError>> {
        Box::pin(async move { Ok(self.0) })
    }
}

#[tokio::test]
async fn titles_from_before_0009_stay_source_titles_and_keep_following_the_feed() {
    let pool = NativeSqliteBackend::new("sqlite::memory:").connect().await.expect("connect");
    let before_0009: Vec<Migration> =
        MIGRATOR.iter().filter(|migration| migration.version <= 8).cloned().collect();
    Migrator::new(Migrations(before_0009)).await.unwrap().run(&pool).await.expect("migrate to 8");
    for (url, title) in [
        ("https://example.com/named.xml", Some("Old Name")),
        ("https://example.com/bare.xml", None),
    ] {
        sqlx::query(
            "INSERT INTO feeds (url, title, created_at, updated_at) \
             VALUES (?1, ?2, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .bind(url)
        .bind(title)
        .execute(&pool)
        .await
        .expect("legacy feed");
    }

    migrate(&pool).await.expect("upgrade to 0009");

    let rows: Vec<(String, Option<String>, Option<String>)> =
        sqlx::query_as("SELECT url, title, custom_title FROM feeds ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        rows,
        [
            ("https://example.com/named.xml".into(), Some("Old Name".into()), None),
            ("https://example.com/bare.xml".into(), None, None),
        ],
        "existing titles are kept as source titles; nothing is copied into custom_title"
    );

    let feeds = SqliteFeedRepository::new(pool.clone());
    let named = feeds.list_feeds().await.unwrap().into_iter().find(|feed| feed.id == 1).unwrap();
    assert_eq!(
        (named.title.as_deref(), named.source_title.as_deref()),
        (Some("Old Name"), Some("Old Name"))
    );
    feeds
        .update_feed_metadata(
            named.id,
            &ParsedFeed {
                title: Some("Renamed Upstream".into()),
                site_url: None,
                description: None,
                entries: Vec::new(),
            },
        )
        .await
        .expect("refresh metadata");
    let renamed = feeds.get_feed(named.id).await.unwrap().unwrap();
    assert_eq!(renamed.title.as_deref(), Some("Renamed Upstream"));
}

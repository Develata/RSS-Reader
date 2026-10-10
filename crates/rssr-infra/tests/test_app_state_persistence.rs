#![cfg(not(target_arch = "wasm32"))]

//! 应用状态的窄字段更新必须在同一道写锁内读改写（审计 A2）。
//!
//! 需要文件型数据库：`:memory:` 的连接池只有一个连接，事务天然串行，证明不了任何东西。

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rssr_domain::{AppStateRepository, EntriesWorkspaceState};
use rssr_infra::db::{
    app_state_repository::SqliteAppStateRepository, sqlite_native::NativeSqliteBackend,
    storage_backend::StorageBackend,
};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

/// 第一个更新持有写事务时，第二个更新不能先读快照：`BEGIN IMMEDIATE` 在读之前就要拿到写锁。
///
/// 第二个更新走一条 `busy_timeout` 为 0 的连接：正确实现在 `BEGIN IMMEDIATE` 上立刻得到
/// `SQLITE_BUSY`，闭包根本不会运行；延迟 `BEGIN` 则会先读到旧快照、运行闭包，写回时才失败。
/// 断言只看闭包是否运行过，不依赖等待时长（独立评审）。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_update_does_not_read_while_another_holds_the_write_lock() {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_nanos();
    let base_dir = std::env::temp_dir().join(format!("rssr-app-state-update-{nonce}"));
    let database = base_dir.join("rss-reader.db");
    let backend = NativeSqliteBackend::with_path(&database);
    let pool = backend.connect().await.expect("connect index db");
    backend.migrate(&pool).await.expect("migrate index db");
    let repository = Arc::new(SqliteAppStateRepository::new(pool));
    let impatient = SqliteAppStateRepository::new(
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new().filename(&database).busy_timeout(Duration::ZERO),
            )
            .await
            .expect("connect a second time"),
    );

    // 第一个更新在事务里停住：读过快照、尚未写回。
    let (entered_sender, entered) = tokio::sync::oneshot::channel();
    let (release, release_receiver) = mpsc::channel::<()>();
    let first = tokio::spawn({
        let repository = repository.clone();
        async move {
            repository
                .update_snapshot(move |state| {
                    entered_sender.send(()).expect("signal first update");
                    release_receiver.recv().expect("release first update");
                    state.last_opened_feed_id = Some(7);
                })
                .await
        }
    });
    entered.await.expect("first update entered its transaction");

    let workspace =
        EntriesWorkspaceState { show_archived: true, ..EntriesWorkspaceState::default() };
    let read_while_locked = Arc::new(AtomicBool::new(false));
    let blocked = impatient
        .update(Box::new({
            let read_while_locked = read_while_locked.clone();
            let workspace = workspace.clone();
            move |state| {
                read_while_locked.store(true, Ordering::SeqCst);
                state.entries_workspace = workspace;
            }
        }))
        .await;
    assert!(blocked.is_err(), "the second update must not get the write lock");
    assert!(
        !read_while_locked.load(Ordering::SeqCst),
        "the second update read the snapshot while the first held the write lock"
    );

    release.send(()).expect("release first update");
    first.await.expect("join first").expect("first update");
    impatient
        .update(Box::new({
            let workspace = workspace.clone();
            move |state| state.entries_workspace = workspace
        }))
        .await
        .expect("second update after the first committed");

    let persisted = repository.load_snapshot().await.expect("load app state");
    assert_eq!(persisted.last_opened_feed_id, Some(7), "last opened feed was lost");
    assert_eq!(persisted.entries_workspace, workspace, "workspace update was lost");
    let _ = std::fs::remove_dir_all(base_dir);
}

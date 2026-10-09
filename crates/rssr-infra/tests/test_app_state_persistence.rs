#![cfg(not(target_arch = "wasm32"))]

//! 应用状态的窄字段更新必须在同一道写锁内读改写（审计 A2）。
//!
//! 需要文件型数据库：`:memory:` 的连接池只有一个连接，事务天然串行，证明不了任何东西。

use std::{
    sync::{Arc, mpsc},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rssr_domain::{AppStateRepository, EntriesWorkspaceState};
use rssr_infra::db::{
    app_state_repository::SqliteAppStateRepository, sqlite_native::NativeSqliteBackend,
    storage_backend::StorageBackend,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn a_second_update_waits_for_the_first_to_commit() {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_nanos();
    let base_dir = std::env::temp_dir().join(format!("rssr-app-state-update-{nonce}"));
    let backend = NativeSqliteBackend::with_path(base_dir.join("rss-reader.db"));
    let pool = backend.connect().await.expect("connect index db");
    backend.migrate(&pool).await.expect("migrate index db");
    let repository = Arc::new(SqliteAppStateRepository::new(pool));

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
    let second = tokio::spawn({
        let repository = repository.clone();
        let workspace = workspace.clone();
        async move {
            repository.update(Box::new(move |state| state.entries_workspace = workspace)).await
        }
    });
    // 第一个事务持有写锁期间，第二个更新既读不到旧快照，也不能提交。
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!second.is_finished(), "the second update must wait for the first to commit");

    release.send(()).expect("release first update");
    first.await.expect("join first").expect("first update");
    second.await.expect("join second").expect("second update");

    let persisted = repository.load_snapshot().await.expect("load app state");
    assert_eq!(persisted.last_opened_feed_id, Some(7), "last opened feed was lost");
    assert_eq!(persisted.entries_workspace, workspace, "workspace update was lost");
    let _ = std::fs::remove_dir_all(base_dir);
}

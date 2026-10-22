#![cfg(all(target_arch = "wasm32", feature = "memory-store"))]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use futures_util::{FutureExt, poll};
use rssr_infra::store::{LibraryStore, StoreError, StoreOptions};
use wasm_bindgen_test::wasm_bindgen_test;

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn engine_identity_and_shared_sql_features() {
    fn shareable<T: Send + Sync>() {}
    shareable::<LibraryStore>();
    let store = LibraryStore::open_in_memory().await.unwrap();
    assert_eq!(store.journal_mode(), "memory");
    let identity = store
        .read(|conn| {
            let identity: (String, String) =
                conn.query_row("SELECT sqlite_version(), sqlite_source_id()", [], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })?;
            assert_eq!(
                conn.query_row("SELECT fold_for_search('ÄBC中文')", [], |r| r
                    .get::<_, String>(0))?,
                "äbc中文"
            );
            assert_eq!(
                conn.query_row("SELECT sum(value) FROM json_each('[1,2,3]')", [], |r| r
                    .get::<_, i64>(0))?,
                6
            );
            Ok(identity)
        })
        .await
        .unwrap();
    let version: Vec<u32> = identity.0.split('.').map(|part| part.parse().unwrap()).collect();
    assert!(version.as_slice() >= [3, 51, 3].as_slice());
    wasm_bindgen_test::console_log!(
        "sqlite_version={} sqlite_source_id={}",
        identity.0,
        identity.1
    );
    store.close().await.unwrap();
}

#[wasm_bindgen_test]
async fn read_only_and_failed_writes_leave_the_connection_usable() {
    let store = LibraryStore::open_in_memory().await.unwrap();
    store
        .write(|tx| {
            tx.execute_batch("CREATE TABLE check_tx(value INTEGER UNIQUE)")?;
            Ok(())
        })
        .await
        .unwrap();
    assert!(
        store.read(|conn| Ok(conn.execute("INSERT INTO check_tx VALUES(1)", [])?)).await.is_err()
    );
    assert!(
        store
            .write(|tx| {
                tx.execute_batch("INSERT INTO check_tx VALUES(1); INSERT INTO check_tx VALUES(1)")?;
                Ok(())
            })
            .await
            .is_err()
    );
    assert_eq!(
        store
            .read(|conn| Ok(
                conn.query_row("SELECT count(*) FROM check_tx", [], |r| r.get::<_, i64>(0))?
            ))
            .await
            .unwrap(),
        0
    );
    store.write(|tx| Ok(tx.execute("INSERT INTO check_tx VALUES(2)", [])?)).await.unwrap();
    store.close().await.unwrap();
}

#[wasm_bindgen_test]
async fn statement_deadline_rolls_back_and_allows_the_next_task() {
    let store = LibraryStore::open_in_memory_with(StoreOptions {
        write_deadline: Duration::from_millis(5),
        ..StoreOptions::default()
    })
    .await
    .unwrap();
    let error = store
        .write(|tx| {
            tx.execute_batch(
                "CREATE TABLE rollback_me(value INTEGER);
            WITH RECURSIVE seq(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM seq WHERE x<100000000)
            INSERT INTO rollback_me SELECT x FROM seq",
            )?;
            Ok(())
        })
        .await
        .unwrap_err();
    assert!(matches!(error, StoreError::Interrupted), "{error}");
    assert_eq!(
        store
            .read(|conn| Ok(conn.query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name='rollback_me'",
                [],
                |r| r.get::<_, i64>(0)
            )?))
            .await
            .unwrap(),
        0
    );
    store
        .write(|tx| Ok(tx.execute_batch("CREATE TABLE after_deadline(value INTEGER)")?))
        .await
        .unwrap();
    store.close().await.unwrap();
}

struct Capture(Arc<AtomicBool>);
impl Drop for Capture {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[wasm_bindgen_test]
async fn dropped_queued_caller_discards_capture_and_never_executes() {
    let store = LibraryStore::open_in_memory().await.unwrap();
    let dropped = Arc::new(AtomicBool::new(false));
    let capture = Capture(Arc::clone(&dropped));
    let mut call = store
        .write(move |tx| {
            drop(capture);
            tx.execute_batch("CREATE TABLE must_not_run(value INTEGER)")?;
            Ok(())
        })
        .boxed();
    assert!(poll!(call.as_mut()).is_pending());
    drop(call);
    assert!(dropped.load(Ordering::SeqCst), "capture drops at cancellation");
    assert_eq!(
        store
            .read(|conn| Ok(conn.query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name='must_not_run'",
                [],
                |r| r.get::<_, i64>(0)
            )?))
            .await
            .unwrap(),
        0
    );
    store.close().await.unwrap();
}

#[wasm_bindgen_test]
async fn expired_admission_never_runs_the_closure() {
    let store = LibraryStore::open_in_memory_with(StoreOptions {
        admission_wait: Duration::ZERO,
        queue_capacity: 1,
        ..StoreOptions::default()
    })
    .await
    .unwrap();
    let ran = Arc::new(AtomicBool::new(false));
    let ran_task = Arc::clone(&ran);
    let error = store
        .write(move |_| {
            ran_task.store(true, Ordering::SeqCst);
            Ok(())
        })
        .await
        .unwrap_err();
    assert!(matches!(error, StoreError::ConnectionTimeout), "{error}");
    assert!(!ran.load(Ordering::SeqCst));
    store.close().await.unwrap();
}

#[wasm_bindgen_test]
async fn close_drains_submitted_work_and_rejects_every_handle() {
    let store = LibraryStore::open_in_memory().await.unwrap();
    let other = store.clone();
    let mut write =
        store.write(|tx| Ok(tx.execute_batch("CREATE TABLE before_close(value INTEGER)")?)).boxed();
    assert!(poll!(write.as_mut()).is_pending());
    let (written, closed) = futures_util::join!(write, other.close());
    written.unwrap();
    closed.unwrap();
    assert!(matches!(store.read(|_| Ok(())).await, Err(StoreError::Closed)));
    other.close().await.unwrap();
}

#[wasm_bindgen_test]
async fn memory_libraries_are_private_and_reopening_starts_empty() {
    let first = LibraryStore::open_in_memory().await.unwrap();
    first
        .write(|tx| Ok(tx.execute_batch("CREATE TABLE private_value(value INTEGER)")?))
        .await
        .unwrap();
    let second = LibraryStore::open_in_memory().await.unwrap();
    first.close().await.unwrap();
    assert_eq!(
        second
            .read(|conn| Ok(conn.query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name='private_value'",
                [],
                |r| r.get::<_, i64>(0)
            )?))
            .await
            .unwrap(),
        0
    );
    second.close().await.unwrap();
}

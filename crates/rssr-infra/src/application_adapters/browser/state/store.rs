//! One transaction boundary for every browser repository. No lock is held across network I/O.
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use wasm_bindgen::{JsCast, prelude::*};

use super::{
    BrowserState,
    storage::{self, Cache, Changes},
};

#[wasm_bindgen(module = "/src/application_adapters/browser/state/lock.js")]
extern "C" {
    #[wasm_bindgen(catch, js_name = withStorageLock)]
    async fn with_storage_lock(callback: &js_sys::Function) -> Result<JsValue, JsValue>;
}

#[derive(Clone, Default)]
pub struct BrowserStore {
    cache: Arc<Mutex<Cache>>,
}

impl BrowserStore {
    /// Open existing committed data, or adopt the four legacy slices without rewriting them.
    /// Unavailable/corrupt storage is an error, never an apparently successful empty database.
    pub async fn open() -> Result<Self> {
        let store = Self::default();
        store.read(|_| Ok(())).await?;
        Ok(store)
    }

    /// A detached diagnostic/export snapshot; normal repository queries borrow the cache.
    pub async fn snapshot(&self) -> Result<BrowserState> {
        self.read(|state| Ok(state.clone())).await
    }

    pub(crate) async fn read<T: Send + 'static>(
        &self,
        query: impl FnOnce(&BrowserState) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        self.update(move |state| query(state).map(|value| (value, Changes::NONE))).await
    }

    pub(crate) async fn update<T: Send + 'static>(
        &self,
        operation: impl FnOnce(&mut BrowserState) -> Result<(T, Changes)> + Send + 'static,
    ) -> Result<T> {
        // JS futures stay on the host executor. The repository's existing Send future only
        // waits on a Rust channel; no unsafe Send wrapper or domain platform branch is needed.
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let cache = Arc::clone(&self.cache);
        wasm_bindgen_futures::spawn_local(async move {
            let sender = Rc::new(RefCell::new(Some(sender)));
            let cancellation = Rc::clone(&sender);
            let result = Rc::new(RefCell::new(None));
            let output = Rc::clone(&result);
            let mut operation = Some(operation);
            let callback = Closure::<dyn FnMut()>::new(move || {
                // Dropping the caller while queued must not cause a later unexpected write.
                if cancellation.borrow().as_ref().is_none_or(|sender| sender.is_closed()) {
                    return;
                }
                if let Some(operation) = operation.take() {
                    *output.borrow_mut() = Some((|| {
                        let mut cache = cache.lock().map_err(|e| anyhow::anyhow!(e.to_string()))?;
                        storage::transaction(&mut cache, operation)
                    })());
                }
            });
            let locked = with_storage_lock(callback.as_ref().unchecked_ref()).await;
            let value = match locked {
                Ok(_) => {
                    result.borrow_mut().take().unwrap_or_else(|| anyhow::bail!("存储操作已取消"))
                }
                Err(error) => {
                    Err(anyhow::anyhow!("无法取得浏览器存储锁（最多等待 5 秒）: {error:?}"))
                }
            };
            if let Some(sender) = sender.borrow_mut().take() {
                let _ = sender.send(value);
            }
        });
        receiver.await.context("浏览器存储任务已中断")?
    }
}

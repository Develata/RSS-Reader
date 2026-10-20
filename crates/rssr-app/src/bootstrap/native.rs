use std::sync::Arc;

use rssr_api::Backend;
pub use rssr_domain::EntryNavigation as ReaderNavigation;
use rssr_infra::{
    local_backend::{LocalBackend, NativeHost, native_local_backend},
    store::{LibraryStore, local::open_local_library},
};
use tokio::sync::OnceCell;

static APP_SERVICES: OnceCell<Arc<AppServices>> = OnceCell::const_new();

/// A failure to open the library, as a domain error first so its kind (busy, storage, …)
/// survives into the backend error the UI shows (B2-D5).
fn open_failure(error: rssr_infra::store::StoreError) -> anyhow::Error {
    anyhow::Error::new(rssr_domain::DomainError::from(error)).context("打开本地资料库失败")
}

/// This process's backend: the use cases over the local library.
pub struct AppServices {
    backend: Arc<LocalBackend<NativeHost>>,
}

impl AppServices {
    pub async fn shared() -> anyhow::Result<Arc<Self>> {
        APP_SERVICES
            .get_or_try_init(|| async {
                let opened = open_local_library().await.map_err(open_failure)?;
                tracing::info!(library = %opened.path.display(), "本地资料库已打开");
                if let Some(imported) = &opened.imported {
                    tracing::info!(?imported, "已从 v0.1 数据导入资料库");
                }
                Ok(Self::over(opened.store))
            })
            .await
            .map(Arc::clone)
    }

    /// The services over an opened library; [`Self::shared`] builds the process-wide one over the
    /// local library.
    pub(crate) fn over(store: LibraryStore) -> Arc<Self> {
        Arc::new(Self { backend: Arc::new(native_local_backend(store)) })
    }

    pub(crate) fn backend(&self) -> Arc<dyn Backend> {
        self.backend.clone()
    }
}

#[cfg(test)]
mod open_failure_tests {
    use rssr_api::{BackendError, ErrorCode};
    use rssr_infra::store::StoreError;

    #[test]
    fn opening_failures_keep_their_kind() {
        let busy: BackendError = super::open_failure(StoreError::ConnectionTimeout).into();
        assert_eq!(busy.code, ErrorCode::Unavailable);
        let corrupt: BackendError = super::open_failure(StoreError::NotALibrary).into();
        assert_eq!(corrupt.code, ErrorCode::Storage);
    }
}

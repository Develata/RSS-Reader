//! The Web page's backend, chosen once at start: the formal Web build talks to the server that
//! served the page (`HttpBackend` over `/api/v1`, phase C, C4a); the Pages demo keeps its library
//! in memory and runs the same SQL repositories in the page (`LocalBackend`).

use std::sync::Arc;

/// Register before Dioxus history so the old DOM still exists on browser back/forward.
pub(crate) fn install_history_capture() {
    if let Err(error) = js_sys::eval(
        "window.addEventListener('popstate', () => {\n\
         document.dispatchEvent(new Event('rssr-history-leave'));\n\
         }, true);",
    ) {
        tracing::warn!(?error, "无法安装浏览器返回位置采集");
    }
}

use rssr_api::Backend;
pub use rssr_domain::EntryNavigation as ReaderNavigation;
use tokio::sync::OnceCell;

static APP_SERVICES: OnceCell<Arc<AppServices>> = OnceCell::const_new();

/// This page's backend.
pub struct AppServices {
    backend: Arc<dyn Backend + Send + Sync>,
}

impl AppServices {
    pub async fn shared() -> anyhow::Result<Arc<Self>> {
        APP_SERVICES
            .get_or_try_init(|| async { Ok(Arc::new(Self { backend: open_backend().await? })) })
            .await
            .map(Arc::clone)
    }

    pub(crate) fn backend(&self) -> Arc<dyn Backend> {
        self.backend.clone()
    }
}

/// The server that served this page, for pages of this build (`build.rs`).
#[cfg(not(feature = "pages-demo"))]
async fn open_backend() -> anyhow::Result<Arc<dyn Backend + Send + Sync>> {
    use anyhow::Context;
    use rssr_infra::http_backend::HttpBackend;

    let origin = web_sys::window()
        .context("没有浏览器窗口")?
        .location()
        .origin()
        .map_err(|error| anyhow::anyhow!("无法读取页面地址：{error:?}"))?;
    let base = url::Url::parse(&origin).context("页面地址无效")?;
    Ok(Arc::new(HttpBackend::new(base, env!("RSSR_BUILD_ID"), reqwest::Client::new())))
}

#[cfg(feature = "pages-demo")]
async fn open_backend() -> anyhow::Result<Arc<dyn Backend + Send + Sync>> {
    demo_host::open().await
}

/// The Pages demo: the use cases over a private memory SQLite library, with no network and no periodic
/// refresh.
#[cfg(feature = "pages-demo")]
mod demo_host {
    use std::sync::Arc;
    use std::time::Duration;

    use rssr_api::Backend;
    use rssr_application::{RefreshAllInput, RemoteConfigStore};
    use rssr_infra::composition::compose_offline_library_use_cases;
    use rssr_infra::local_backend::{LocalBackend, LocalHost, LocalTask};
    use rssr_infra::platform_clock::now_utc;
    use time::OffsetDateTime;

    pub(super) async fn open() -> anyhow::Result<Arc<dyn Backend + Send + Sync>> {
        let store = crate::demo::open_library().await?;
        let composition = compose_offline_library_use_cases(store);
        Ok(Arc::new(LocalBackend::new(composition.use_cases, WebHost)))
    }

    struct WebHost;

    #[async_trait::async_trait(?Send)]
    impl LocalHost for WebHost {
        fn refresh_concurrency(&self) -> usize {
            RefreshAllInput::default().max_concurrency
        }

        fn require_network(&self) -> anyhow::Result<()> {
            crate::demo::require_network()
        }

        fn runs_auto_refresh(&self) -> bool {
            false
        }

        fn now_utc(&self) -> OffsetDateTime {
            now_utc()
        }

        fn spawn(&self, task: LocalTask) {
            wasm_bindgen_futures::spawn_local(task);
        }

        async fn sleep(&self, duration: Duration) {
            gloo_timers::future::sleep(duration).await;
        }

        fn remote_config_store(
            &self,
            _endpoint: &str,
            _remote_path: &str,
        ) -> anyhow::Result<Box<dyn RemoteConfigStore>> {
            Err(rssr_domain::DomainError::Network("静态演示已禁用远端配置同步。".into()).into())
        }
    }
}

//! The Web page's backend, chosen once at start: the formal Web build talks to the server that
//! served the page (`HttpBackend` over `/api/v1`, phase C, C4a); the Pages demo keeps its library
//! in the browser and runs the use cases in the page (`LocalBackend`, until phase D).

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

/// The Pages demo: the use cases over the browser's storage, with no network and no periodic
/// refresh.
#[cfg(feature = "pages-demo")]
mod demo_host {
    use std::sync::Arc;
    use std::time::Duration;

    use rssr_api::Backend;
    use rssr_application::{ClockPort, RefreshAllInput, RemoteConfigStore};
    use rssr_infra::application_adapters::browser::{
        adapters::BrowserRemoteConfigStore, now_utc, state::BrowserStore,
    };
    use rssr_infra::composition::compose_browser_use_cases;
    use rssr_infra::local_backend::{LocalBackend, LocalHost, LocalTask};
    use time::OffsetDateTime;

    pub(super) async fn open() -> anyhow::Result<Arc<dyn Backend + Send + Sync>> {
        let state = BrowserStore::open_with_initial_state(crate::demo::initial_state()?).await?;
        let client = reqwest::Client::new();
        let use_cases = compose_browser_use_cases(state, client.clone(), Arc::new(BrowserClock));
        Ok(Arc::new(LocalBackend::new(use_cases, WebHost { client })))
    }

    /// The browser: `spawn_local`, timers, its clock, `fetch` for configuration exchange; no image
    /// localization, no network, no periodic refresh.
    struct WebHost {
        client: reqwest::Client,
    }

    #[derive(Clone)]
    struct BrowserClock;

    impl ClockPort for BrowserClock {
        fn now_utc(&self) -> OffsetDateTime {
            now_utc()
        }
    }

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
            endpoint: &str,
            remote_path: &str,
        ) -> anyhow::Result<Box<dyn RemoteConfigStore>> {
            Ok(Box::new(BrowserRemoteConfigStore::new(self.client.clone(), endpoint, remote_path)))
        }
    }
}

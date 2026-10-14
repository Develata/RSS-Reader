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

use std::time::Duration;

use rssr_api::Backend;
use rssr_application::{ClockPort, RefreshAllInput, RemoteConfigStore};
pub use rssr_domain::EntryNavigation as ReaderNavigation;
use rssr_domain::UserSettings;
use rssr_infra::application_adapters::browser::{
    adapters::BrowserRemoteConfigStore, now_utc, state::BrowserStore,
};
use rssr_infra::composition::compose_browser_use_cases;
use rssr_infra::local_backend::{LocalBackend, LocalHost, LocalTask};
use time::OffsetDateTime;
use tokio::sync::OnceCell;

static APP_SERVICES: OnceCell<Arc<AppServices>> = OnceCell::const_new();

/// This page's backend: the use cases over the browser's storage.
pub struct AppServices {
    backend: Arc<LocalBackend<WebHost>>,
}

/// The browser: `spawn_local`, timers, its clock, `fetch` for configuration exchange; no image
/// localization; the Pages demo has no network and no periodic refresh.
pub struct WebHost {
    client: reqwest::Client,
}

#[derive(Clone)]
struct BrowserClock;

impl ClockPort for BrowserClock {
    fn now_utc(&self) -> OffsetDateTime {
        now_utc()
    }
}

impl AppServices {
    pub async fn shared() -> anyhow::Result<Arc<Self>> {
        APP_SERVICES
            .get_or_try_init(|| async {
                #[cfg(feature = "pages-demo")]
                let state =
                    BrowserStore::open_with_initial_state(crate::demo::initial_state()?).await?;
                #[cfg(not(feature = "pages-demo"))]
                let state = BrowserStore::open().await?;
                let client = reqwest::Client::new();
                let use_cases =
                    compose_browser_use_cases(state, client.clone(), Arc::new(BrowserClock));
                Ok(Arc::new(Self {
                    backend: Arc::new(LocalBackend::new(use_cases, WebHost { client })),
                }))
            })
            .await
            .map(Arc::clone)
    }

    pub fn default_settings() -> UserSettings {
        UserSettings::default()
    }

    pub(crate) fn backend(&self) -> Arc<dyn Backend> {
        self.backend.clone()
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
        !crate::demo::ENABLED
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

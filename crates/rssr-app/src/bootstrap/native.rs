//! 桌面与 Android 的宿主：在默认数据目录上打开 [`rssr_host::native::NativeHost`]，进程内共享一份。
use std::sync::Arc;

use anyhow::Context;
use rssr_application::AppUseCases;
pub use rssr_domain::EntryNavigation as ReaderNavigation;
use rssr_domain::UserSettings;
use rssr_host::{HostCapabilities, native::NativeHost};
use rssr_infra::db::sqlite_native::NativeSqliteBackend;
use tokio::sync::OnceCell;

static APP_SERVICES: OnceCell<Arc<AppServices>> = OnceCell::const_new();

pub struct AppServices {
    host: Arc<NativeHost>,
}

impl AppServices {
    pub async fn shared() -> anyhow::Result<Arc<Self>> {
        APP_SERVICES
            .get_or_try_init(|| async {
                let backend = NativeSqliteBackend::from_default_location()
                    .context("确定本地数据库位置失败")?;
                Ok(Arc::new(Self { host: NativeHost::open(backend).await? }))
            })
            .await
            .map(Arc::clone)
    }

    pub fn default_settings() -> UserSettings {
        UserSettings::default()
    }

    pub(crate) fn use_cases(&self) -> AppUseCases {
        self.host.use_cases()
    }

    pub(crate) fn host_capabilities(self: &Arc<Self>) -> HostCapabilities {
        self.host.host_capabilities()
    }
}

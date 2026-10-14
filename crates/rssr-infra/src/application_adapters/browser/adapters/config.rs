use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::StatusCode;
use rssr_application::{OpmlCodecPort, RemoteConfigStore};

use crate::{
    application_adapters::browser::config::remote_url,
    opml::OpmlCodec,
    remote_failure::{invalid, network},
};

/// 委托给原生端同一个 [`OpmlCodec`]（审计 D5）：两端的名称规则与 XML 校验一致。
#[derive(Clone, Default)]
pub struct BrowserOpmlCodec;

impl OpmlCodecPort for BrowserOpmlCodec {
    fn encode(&self, feeds: &[rssr_domain::OpmlFeed]) -> Result<String> {
        OpmlCodec::new().encode(feeds)
    }

    fn decode(&self, raw: &str) -> Result<Vec<rssr_domain::ConfigFeed>> {
        OpmlCodec::new().decode(raw)
    }
}

/// Each WebDAV request, its body included, ends within this bound, as on the native client
/// (`config_sync::webdav`): a server that never finishes its answer cannot hold the settings
/// page, whose saves wait for a pull in flight (B2-D4).
const WEBDAV_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct BrowserRemoteConfigStore {
    client: reqwest::Client,
    endpoint: String,
    remote_path: String,
    timeout: Duration,
}

impl BrowserRemoteConfigStore {
    pub fn new(client: reqwest::Client, endpoint: &str, remote_path: &str) -> Self {
        Self {
            client,
            endpoint: endpoint.to_string(),
            remote_path: remote_path.to_string(),
            timeout: WEBDAV_REQUEST_TIMEOUT,
        }
    }

    /// A different bound for each request (30 seconds by default).
    pub fn with_request_timeout(self, timeout: Duration) -> Self {
        Self { timeout, ..self }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl RemoteConfigStore for BrowserRemoteConfigStore {
    async fn upload_config(&self, raw: &str) -> Result<()> {
        let url = remote_url(&self.endpoint, &self.remote_path).map_err(invalid)?;
        async {
            self.client
                .put(url)
                .timeout(self.timeout)
                .header("content-type", "application/json")
                .body(raw.to_string())
                .send()
                .await
                .context("上传配置到 WebDAV 失败")?
                .error_for_status()
                .context("WebDAV 上传失败")?;
            Ok(())
        }
        .await
        .map_err(network)
    }

    async fn download_config(&self) -> Result<Option<String>> {
        let url = remote_url(&self.endpoint, &self.remote_path).map_err(invalid)?;
        async {
            let response = self
                .client
                .get(url)
                .timeout(self.timeout)
                .send()
                .await
                .context("从 WebDAV 下载配置失败")?;
            if response.status() == StatusCode::NOT_FOUND {
                return Ok(None);
            }
            let raw = response.error_for_status().context("WebDAV 下载失败")?.text().await?;
            Ok(Some(raw))
        }
        .await
        .map_err(network)
    }
}

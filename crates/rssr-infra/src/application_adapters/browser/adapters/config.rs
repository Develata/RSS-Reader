use anyhow::{Context, Result};
use reqwest::StatusCode;
use rssr_application::{OpmlCodecPort, RemoteConfigStore};

use crate::application_adapters::browser::config::{decode_opml, encode_opml, remote_url};

#[derive(Clone, Default)]
pub struct BrowserOpmlCodec;

impl OpmlCodecPort for BrowserOpmlCodec {
    fn encode(&self, feeds: &[rssr_domain::ConfigFeed]) -> Result<String> {
        encode_opml(feeds)
    }

    fn decode(&self, raw: &str) -> Result<Vec<rssr_domain::ConfigFeed>> {
        decode_opml(raw)
    }
}

#[derive(Clone)]
pub struct BrowserRemoteConfigStore {
    client: reqwest::Client,
    endpoint: String,
    remote_path: String,
}

impl BrowserRemoteConfigStore {
    pub fn new(client: reqwest::Client, endpoint: &str, remote_path: &str) -> Self {
        Self { client, endpoint: endpoint.to_string(), remote_path: remote_path.to_string() }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl RemoteConfigStore for BrowserRemoteConfigStore {
    async fn upload_config(&self, raw: &str) -> Result<()> {
        self.client
            .put(remote_url(&self.endpoint, &self.remote_path)?)
            .header("content-type", "application/json")
            .body(raw.to_string())
            .send()
            .await
            .context("上传配置到 WebDAV 失败")?
            .error_for_status()
            .context("WebDAV 上传失败")?;
        Ok(())
    }

    async fn download_config(&self) -> Result<Option<String>> {
        let response = self
            .client
            .get(remote_url(&self.endpoint, &self.remote_path)?)
            .send()
            .await
            .context("从 WebDAV 下载配置失败")?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let raw = response.error_for_status().context("WebDAV 下载失败")?.text().await?;
        Ok(Some(raw))
    }
}

use anyhow::{Context, Result};
use reqwest::{Client, StatusCode};
use url::Url;

#[derive(Debug, Clone)]
pub struct WebDavConfigSync {
    client: Client,
    pub endpoint: Url,
    pub remote_path: String,
}

impl WebDavConfigSync {
    pub fn new(endpoint: impl AsRef<str>, remote_path: impl Into<String>) -> Result<Self> {
        Ok(Self {
            client: Client::new(),
            endpoint: Url::parse(endpoint.as_ref()).context("无效的 WebDAV endpoint")?,
            remote_path: remote_path.into(),
        })
    }

    pub fn remote_url(&self) -> Result<Url> {
        self.endpoint
            .join(self.remote_path.trim_start_matches('/'))
            .context("拼接 WebDAV 远端路径失败")
    }

    pub async fn upload_text(&self, body: &str) -> Result<()> {
        let response = self
            .client
            .put(self.remote_url()?)
            .header("content-type", "application/json")
            .body(body.to_string())
            .send()
            .await
            .context("上传配置到 WebDAV 失败")?;

        if response.status().is_success() {
            return Ok(());
        }

        anyhow::bail!("WebDAV 上传失败: {}", response.status());
    }

    pub async fn download_text(&self) -> Result<Option<String>> {
        let response =
            self.client.get(self.remote_url()?).send().await.context("从 WebDAV 下载配置失败")?;

        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if !response.status().is_success() {
            anyhow::bail!("WebDAV 下载失败: {}", response.status());
        }

        Ok(Some(response.text().await.context("读取 WebDAV 配置响应失败")?))
    }
}

#[cfg(test)]
mod tests {
    use super::WebDavConfigSync;

    #[test]
    fn remote_url_joins_endpoint_and_path() {
        let sync = WebDavConfigSync::new("https://dav.example.com/base/", "config/state.json")
            .expect("create webdav config");

        assert_eq!(
            sync.remote_url().expect("resolve remote url").as_str(),
            "https://dav.example.com/base/config/state.json"
        );
    }
}

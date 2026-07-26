use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::{Client, StatusCode, header};
use url::Url;

const WEBDAV_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const WEBDAV_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// 远端配置包是一个 JSON 文件，正常只有几十 KB；设个上限避免被超大响应拖垮。
const MAX_REMOTE_CONFIG_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct WebDavConfigSync {
    client: Client,
    /// 已剥离 userinfo 的端点。凭据单独存放，避免被拼进日志或错误信息里。
    pub endpoint: Url,
    pub remote_path: String,
    credentials: Option<WebDavCredentials>,
}

#[derive(Debug, Clone)]
struct WebDavCredentials {
    username: String,
    password: Option<String>,
}

impl WebDavConfigSync {
    /// 从端点 URL 建立同步客户端。
    ///
    /// 绝大多数 WebDAV 服务（Nextcloud、坚果云等）都要求认证，因此支持在端点里带 userinfo
    /// （`https://user:pass@dav.example.com/base/`）：凭据会被取出来走 HTTP Basic，并从 URL 上
    /// 剥掉，之后所有请求和错误信息里都不再出现。
    ///
    /// 之所以走 URL 而不是新增配置项：端点本身是设置页的会话内状态，既不写盘也不进配置包，
    /// 把密码放在这里不会被 `export_config` 导出，也不会被推到远端。
    pub fn new(endpoint: impl AsRef<str>, remote_path: impl Into<String>) -> Result<Self> {
        let mut endpoint = Url::parse(endpoint.as_ref()).context("无效的 WebDAV endpoint")?;
        let credentials = take_url_credentials(&mut endpoint);

        Ok(Self {
            client: Client::builder()
                .timeout(WEBDAV_REQUEST_TIMEOUT)
                .connect_timeout(WEBDAV_CONNECT_TIMEOUT)
                .build()
                .unwrap_or_else(|_| Client::new()),
            endpoint,
            remote_path: remote_path.into(),
            credentials,
        })
    }

    pub fn remote_url(&self) -> Result<Url> {
        let mut collection = self.endpoint.clone();
        if !collection.path().ends_with('/') {
            collection.set_path(&format!("{}/", collection.path()));
        }

        collection
            .join(self.remote_path.trim_start_matches('/'))
            .context("拼接 WebDAV 远端路径失败")
    }

    pub async fn upload_text(&self, body: &str) -> Result<()> {
        let response = self
            .authenticated(self.client.put(self.remote_url()?))
            .header(header::CONNECTION, "close")
            .header(header::CONTENT_TYPE, "application/json")
            .body(body.to_string())
            .send()
            .await
            .context("上传配置到 WebDAV 失败")?;

        if response.status().is_success() {
            return Ok(());
        }

        anyhow::bail!("WebDAV 上传失败: {}", unauthorized_hint(response.status()));
    }

    pub async fn download_text(&self) -> Result<Option<String>> {
        let mut response = self
            .authenticated(self.client.get(self.remote_url()?))
            .header(header::CONNECTION, "close")
            .send()
            .await
            .context("从 WebDAV 下载配置失败")?;

        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if !response.status().is_success() {
            anyhow::bail!("WebDAV 下载失败: {}", unauthorized_hint(response.status()));
        }

        // 边下边累计，不把任意大小的远端响应整个读进内存。
        let mut buffered = Vec::new();
        while let Some(chunk) = response.chunk().await.context("读取 WebDAV 配置响应失败")?
        {
            anyhow::ensure!(
                buffered.len() + chunk.len() <= MAX_REMOTE_CONFIG_BYTES,
                "WebDAV 配置响应超过 {MAX_REMOTE_CONFIG_BYTES} 字节上限"
            );
            buffered.extend_from_slice(&chunk);
        }

        Ok(Some(String::from_utf8(buffered).context("WebDAV 配置响应不是有效 UTF-8")?))
    }

    fn authenticated(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.credentials {
            Some(credentials) => {
                request.basic_auth(&credentials.username, credentials.password.as_deref())
            }
            None => request,
        }
    }
}

/// 取出并清除 URL 上的 userinfo。用户名为空视为没有提供凭据。
fn take_url_credentials(endpoint: &mut Url) -> Option<WebDavCredentials> {
    let username = percent_decode(endpoint.username());
    let password = endpoint.password().map(percent_decode);

    // 无论是否解析出凭据都要清空，避免 userinfo 残留在后续请求 URL 与错误信息里。
    let _ = endpoint.set_username("");
    let _ = endpoint.set_password(None);

    (!username.is_empty()).then_some(WebDavCredentials { username, password })
}

fn percent_decode(raw: &str) -> String {
    percent_encoding::percent_decode_str(raw).decode_utf8_lossy().into_owned()
}

/// 401/403 单看状态码很难判断是路径写错还是没登录，这里补一句提示。
fn unauthorized_hint(status: StatusCode) -> String {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            format!(
                "{status}（该端点需要认证，请在 endpoint 中带上 https://用户名:密码@主机/ 形式的凭据）"
            )
        }
        other => other.to_string(),
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

    #[test]
    fn remote_url_treats_endpoint_as_collection_without_trailing_slash() {
        let sync = WebDavConfigSync::new("https://dav.example.com/base", "config/state.json")
            .expect("create webdav config");

        assert_eq!(
            sync.remote_url().expect("resolve remote url").as_str(),
            "https://dav.example.com/base/config/state.json"
        );
    }

    #[test]
    fn credentials_are_taken_from_endpoint_and_stripped_from_url() {
        let sync = WebDavConfigSync::new(
            "https://alice:s3cr3t@dav.example.com/base/",
            "config/state.json",
        )
        .expect("create webdav config");

        let credentials = sync.credentials.as_ref().expect("credentials parsed");
        assert_eq!(credentials.username, "alice");
        assert_eq!(credentials.password.as_deref(), Some("s3cr3t"));

        // 端点与最终请求 URL 里都不能再残留凭据，否则会顺着日志和错误信息泄漏出去。
        assert_eq!(sync.endpoint.as_str(), "https://dav.example.com/base/");
        assert_eq!(
            sync.remote_url().expect("resolve remote url").as_str(),
            "https://dav.example.com/base/config/state.json"
        );
    }

    #[test]
    fn percent_encoded_credentials_are_decoded() {
        let sync =
            WebDavConfigSync::new("https://user%40example.com:p%40ss@dav.example.com/", "s.json")
                .expect("create webdav config");

        let credentials = sync.credentials.as_ref().expect("credentials parsed");
        assert_eq!(credentials.username, "user@example.com");
        assert_eq!(credentials.password.as_deref(), Some("p@ss"));
    }

    #[test]
    fn endpoint_without_credentials_stays_anonymous() {
        let sync = WebDavConfigSync::new("https://dav.example.com/base/", "config/state.json")
            .expect("create webdav config");

        assert!(sync.credentials.is_none());
    }
}

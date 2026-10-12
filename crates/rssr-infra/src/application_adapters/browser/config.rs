use anyhow::Context;
use url::Url;

pub fn remote_url(endpoint: &str, remote_path: &str) -> anyhow::Result<Url> {
    let mut collection = Url::parse(endpoint).context("无效的 WebDAV endpoint")?;
    if !collection.path().ends_with('/') {
        collection.set_path(&format!("{}/", collection.path()));
    }
    collection.join(remote_path.trim_start_matches('/')).context("拼接 WebDAV 远端路径失败")
}

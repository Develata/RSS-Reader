use std::{collections::HashSet, fs, path::Path};

use anyhow::{anyhow, ensure};
use rssr_domain::ConfigPackage;
use url::Url;

pub fn encode_config_package(package: &ConfigPackage) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(package)?)
}

pub fn decode_config_package(raw: &str) -> anyhow::Result<ConfigPackage> {
    let package: ConfigPackage = serde_json::from_str(raw)?;
    validate_config_package(&package)?;
    Ok(package)
}

pub fn read_config_package(path: impl AsRef<Path>) -> anyhow::Result<ConfigPackage> {
    let raw = fs::read_to_string(path)?;
    decode_config_package(&raw)
}

pub fn write_config_package(path: impl AsRef<Path>, package: &ConfigPackage) -> anyhow::Result<()> {
    let raw = encode_config_package(package)?;
    fs::write(path, raw)?;
    Ok(())
}

pub fn validate_config_package(package: &ConfigPackage) -> anyhow::Result<()> {
    ensure!(package.version >= 1, "配置包版本必须大于等于 1");
    ensure!(package.settings.refresh_interval_minutes >= 1, "刷新间隔必须大于等于 1 分钟");
    ensure!(
        (0.8..=1.5).contains(&package.settings.reader_font_scale),
        "阅读字号缩放必须在 0.8 到 1.5 之间"
    );

    let mut normalized_urls = HashSet::new();
    for feed in &package.feeds {
        let normalized = normalize_feed_url(&feed.url)?;
        ensure!(
            normalized_urls.insert(normalized.clone()),
            "配置包中包含重复的 feed URL：{normalized}"
        );
    }

    Ok(())
}

fn normalize_feed_url(raw: &str) -> anyhow::Result<String> {
    let mut url = Url::parse(raw).map_err(|err| anyhow!("无效的 feed URL `{raw}`: {err}"))?;
    url.set_fragment(None);
    Ok(url.to_string())
}

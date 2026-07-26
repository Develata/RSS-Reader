use std::{collections::HashSet, fs, path::Path};

use anyhow::{anyhow, ensure};
use rssr_domain::{
    ConfigPackage, MAX_ARCHIVE_AFTER_MONTHS, MAX_ENTRIES_PAGE_SIZE, MAX_REFRESH_INTERVAL_MINUTES,
    normalize_feed_url as normalize_domain_feed_url,
};
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

// 这里的规则必须与 `rssr_application::import_export_service::rules` 保持一致：两边校验的是同一个
// 配置包契约，任何一边放宽都会让一份包在导入路径通过、在文件路径被拒（或反之）。
pub fn validate_config_package(package: &ConfigPackage) -> anyhow::Result<()> {
    ensure!(package.version == 2, "配置包版本必须等于 2");
    ensure!(
        (1..=MAX_REFRESH_INTERVAL_MINUTES).contains(&package.settings.refresh_interval_minutes),
        "刷新间隔必须在 1 到 {MAX_REFRESH_INTERVAL_MINUTES} 分钟之间"
    );
    ensure!(
        (1..=MAX_ARCHIVE_AFTER_MONTHS).contains(&package.settings.archive_after_months),
        "自动归档阈值必须在 1 到 {MAX_ARCHIVE_AFTER_MONTHS} 个月之间"
    );
    ensure!(
        (1..=MAX_ENTRIES_PAGE_SIZE).contains(&package.settings.entries_page_size),
        "文章页每页数量必须在 1 到 {MAX_ENTRIES_PAGE_SIZE} 之间"
    );
    ensure!(
        (0.8..=1.5).contains(&package.settings.reader_font_scale),
        "阅读字号缩放必须在 0.8 到 1.5 之间"
    );

    let mut normalized_urls = HashSet::new();
    for feed in &package.feeds {
        let normalized = normalize_feed_url_string(&feed.url)?;
        ensure!(
            normalized_urls.insert(normalized.clone()),
            "配置包中包含重复的 feed URL：{normalized}"
        );
    }

    Ok(())
}

fn normalize_feed_url_string(raw: &str) -> anyhow::Result<String> {
    let url = Url::parse(raw).map_err(|err| anyhow!("无效的 feed URL `{raw}`: {err}"))?;
    Ok(normalize_domain_feed_url(&url).to_string())
}

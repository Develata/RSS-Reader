use anyhow::{Context, Result, ensure};
use rssr_domain::{ConfigPackage, UserSettings, normalize_feed_url};
use url::Url;

pub(super) fn import_field(value: Option<String>, existed: bool) -> Option<String> {
    if existed { value.or(Some(String::new())) } else { value }
}

pub(super) fn validate_config_package(package: &ConfigPackage) -> Result<()> {
    ensure!(package.version == 2, "配置包版本必须等于 2");
    validate_settings(&package.settings)?;

    let mut seen_urls = std::collections::HashSet::new();
    for feed in &package.feeds {
        let normalized = normalize_feed_url(
            &Url::parse(&feed.url).with_context(|| format!("无效的订阅 URL：{}", feed.url))?,
        );
        ensure!(
            seen_urls.insert(normalized.to_string()),
            "配置包中包含重复的 feed URL：{}",
            feed.url
        );
    }

    Ok(())
}

fn validate_settings(settings: &UserSettings) -> Result<()> {
    ensure!(settings.refresh_interval_minutes >= 1, "刷新间隔必须大于等于 1 分钟");
    ensure!(settings.archive_after_months >= 1, "自动归档阈值必须大于等于 1 个月");
    ensure!(
        (0.8..=1.5).contains(&settings.reader_font_scale),
        "阅读字号缩放必须在 0.8 到 1.5 之间"
    );
    Ok(())
}

use anyhow::{Context, Result, ensure};
use rssr_domain::{ConfigPackage, MAX_ENTRIES_PAGE_SIZE, UserSettings, normalize_feed_url};
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
        (1..=MAX_ENTRIES_PAGE_SIZE).contains(&settings.entries_page_size),
        "文章页每页数量必须在 1 到 {MAX_ENTRIES_PAGE_SIZE} 之间"
    );
    ensure!(
        (0.8..=1.5).contains(&settings.reader_font_scale),
        "阅读字号缩放必须在 0.8 到 1.5 之间"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use rssr_domain::{ConfigPackage, UserSettings};
    use time::OffsetDateTime;

    use super::validate_config_package;

    #[test]
    fn rejects_config_package_with_zero_entries_page_size() {
        let mut settings = UserSettings::default();
        settings.entries_page_size = 0;
        let package = ConfigPackage {
            version: 2,
            exported_at: OffsetDateTime::UNIX_EPOCH,
            feeds: Vec::new(),
            settings,
        };

        let err = validate_config_package(&package).expect_err("reject zero page size");

        assert!(err.to_string().contains("文章页每页数量"));
    }
}

//! 配置与设置的唯一校验来源。
//!
//! 这里的规则同时被三条路径使用：设置页保存（`SettingsService::save`）、配置包导入
//! （`ImportExportService`）、以及配置文件编解码（`rssr_infra::config_sync::file_format`）。
//! 三处曾各自实现过一份，规则一度不一致：文件路径接受 `version >= 1` 且完全不校验
//! `entries_page_size`，导致同一份配置包在一条路径通过、在另一条被拒。校验的是纯领域类型、
//! 不涉及任何 I/O，因此归属 domain 层。

use std::collections::HashSet;

use url::Url;

use crate::{
    DomainError,
    feed::normalize_feed_url,
    settings::{
        ConfigPackage, MAX_ARCHIVE_AFTER_MONTHS, MAX_ENTRIES_PAGE_SIZE,
        MAX_REFRESH_INTERVAL_MINUTES, UserSettings,
    },
};

/// 当前配置包格式版本。导出时写入，导入时要求精确匹配。
pub const CONFIG_PACKAGE_VERSION: u32 = 2;

pub const MIN_READER_FONT_SCALE: f32 = 0.8;
pub const MAX_READER_FONT_SCALE: f32 = 1.5;

fn invalid(message: impl Into<String>) -> DomainError {
    DomainError::InvalidInput(message.into())
}

/// 校验用户设置的取值范围。
///
/// 上界不只是为了拦住无意义的输入：`archive_after_months` 过大时归档分界会越出
/// `time::Date` 支持的年份区间，`refresh_interval_minutes` 过大时
/// `上次刷新时间 + 间隔` 会让 `OffsetDateTime` 加溢出。
pub fn validate_user_settings(settings: &UserSettings) -> crate::Result<()> {
    if !(1..=MAX_REFRESH_INTERVAL_MINUTES).contains(&settings.refresh_interval_minutes) {
        return Err(invalid(format!(
            "刷新间隔必须在 1 到 {MAX_REFRESH_INTERVAL_MINUTES} 分钟之间"
        )));
    }
    if !(1..=MAX_ARCHIVE_AFTER_MONTHS).contains(&settings.archive_after_months) {
        return Err(invalid(format!(
            "自动归档阈值必须在 1 到 {MAX_ARCHIVE_AFTER_MONTHS} 个月之间"
        )));
    }
    if !(1..=MAX_ENTRIES_PAGE_SIZE).contains(&settings.entries_page_size) {
        return Err(invalid(format!("文章页每页数量必须在 1 到 {MAX_ENTRIES_PAGE_SIZE} 之间")));
    }
    if !(MIN_READER_FONT_SCALE..=MAX_READER_FONT_SCALE).contains(&settings.reader_font_scale) {
        return Err(invalid(format!(
            "阅读字号缩放必须在 {MIN_READER_FONT_SCALE} 到 {MAX_READER_FONT_SCALE} 之间"
        )));
    }

    Ok(())
}

/// 校验配置包：版本、设置取值范围，以及归一化后不允许出现重复的 feed URL。
pub fn validate_config_package(package: &ConfigPackage) -> crate::Result<()> {
    if package.version != CONFIG_PACKAGE_VERSION {
        return Err(invalid(format!("配置包版本必须等于 {CONFIG_PACKAGE_VERSION}")));
    }
    validate_user_settings(&package.settings)?;

    let mut seen_urls = HashSet::with_capacity(package.feeds.len());
    for feed in &package.feeds {
        let normalized = parse_and_normalize_feed_url(&feed.url)?;
        if !seen_urls.insert(normalized.clone()) {
            return Err(invalid(format!("配置包中包含重复的 feed URL：{normalized}")));
        }
    }

    Ok(())
}

/// 解析并归一化一个 feed URL，失败时给出带原始输入的错误。
pub fn parse_and_normalize_feed_url(raw: &str) -> crate::Result<String> {
    let url =
        Url::parse(raw).map_err(|error| invalid(format!("无效的 feed URL：{raw}（{error}）")))?;
    Ok(normalize_feed_url(&url).to_string())
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use super::{CONFIG_PACKAGE_VERSION, validate_config_package, validate_user_settings};
    use crate::settings::{ConfigFeed, ConfigPackage, UserSettings};

    fn package(feeds: Vec<ConfigFeed>, settings: UserSettings) -> ConfigPackage {
        ConfigPackage {
            version: CONFIG_PACKAGE_VERSION,
            exported_at: OffsetDateTime::UNIX_EPOCH,
            feeds,
            settings,
        }
    }

    fn feed(url: &str) -> ConfigFeed {
        ConfigFeed { url: url.to_string(), title: None, folder: None }
    }

    #[test]
    fn accepts_default_settings() {
        assert!(validate_user_settings(&UserSettings::default()).is_ok());
    }

    #[test]
    fn rejects_out_of_range_settings() {
        let cases = [
            (UserSettings { refresh_interval_minutes: 0, ..UserSettings::default() }, "刷新间隔"),
            (
                UserSettings { refresh_interval_minutes: u32::MAX, ..UserSettings::default() },
                "刷新间隔",
            ),
            (UserSettings { archive_after_months: 0, ..UserSettings::default() }, "自动归档阈值"),
            (
                UserSettings { archive_after_months: 4_000_000, ..UserSettings::default() },
                "自动归档阈值",
            ),
            (UserSettings { entries_page_size: 0, ..UserSettings::default() }, "文章页每页数量"),
            (UserSettings { entries_page_size: 201, ..UserSettings::default() }, "文章页每页数量"),
            (UserSettings { reader_font_scale: 0.1, ..UserSettings::default() }, "阅读字号缩放"),
        ];

        for (settings, expected) in cases {
            let error = validate_user_settings(&settings).expect_err("expected rejection");
            assert!(error.to_string().contains(expected), "expected `{expected}` in `{error}`");
        }
    }

    #[test]
    fn rejects_wrong_package_version() {
        let mut invalid = package(Vec::new(), UserSettings::default());
        invalid.version = 1;

        let error = validate_config_package(&invalid).expect_err("expected rejection");

        assert!(error.to_string().contains("配置包版本"));
    }

    #[test]
    fn rejects_duplicate_feed_urls_after_normalization() {
        let invalid = package(
            vec![
                feed("https://example.com/feed.xml#fragment"),
                feed("https://example.com:443/feed.xml"),
            ],
            UserSettings::default(),
        );

        let error = validate_config_package(&invalid).expect_err("expected rejection");

        assert!(error.to_string().contains("重复的 feed URL"));
    }

    #[test]
    fn rejects_invalid_feed_url() {
        let invalid = package(vec![feed("not-a-url")], UserSettings::default());

        let error = validate_config_package(&invalid).expect_err("expected rejection");

        assert!(error.to_string().contains("无效的 feed URL"));
    }
}

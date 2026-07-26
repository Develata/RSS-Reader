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

/// 校验自定义 CSS 的括号、引号与注释是否闭合。
///
/// 返回原始原因字符串而不是 [`DomainError`]，因为两个调用方要用不同的方式包装它：
/// 设置页直接拼进面向用户的提示，配置包校验则包成 `DomainError`。
///
/// **这不是安全边界**：它只保证这段 CSS 不会因为没闭合的括号而把后续样式吃掉，
/// 不试图限制 CSS 能表达什么。自定义 CSS 本来就是用户自己写给自己的应用的。
///
/// 注意本函数**不参与** [`validate_user_settings`]：设置页保存路径上的调用点
/// （`settings_page/save/session.rs` 与 `theme_apply.rs`）在本函数搬进 domain 之前就已经
/// 在调用它了，语义与提示文案保持不变；而把它塞进 `validate_user_settings` 会让它
/// 顺带作用到所有写设置的路径上，可能突然拒绝用户早已存好的 CSS。
pub fn validate_custom_css(raw: &str) -> Result<(), &'static str> {
    let mut stack = Vec::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut in_comment = false;
    let mut escaped = false;
    let mut chars = raw.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_comment {
            if ch == '*' && chars.peek() == Some(&'/') {
                let _ = chars.next();
                in_comment = false;
            }
            continue;
        }

        if escaped {
            escaped = false;
            continue;
        }

        match ch {
            '\\' if in_single_quote || in_double_quote => escaped = true,
            '\'' if !in_double_quote => in_single_quote = !in_single_quote,
            '"' if !in_single_quote => in_double_quote = !in_double_quote,
            '/' if !in_single_quote && !in_double_quote && chars.peek() == Some(&'*') => {
                let _ = chars.next();
                in_comment = true;
            }
            '{' | '(' | '[' if !in_single_quote && !in_double_quote => stack.push(ch),
            '}' | ')' | ']' if !in_single_quote && !in_double_quote => {
                let Some(open) = stack.pop() else {
                    return Err("存在未匹配的右括号或右花括号");
                };
                if !matches!((open, ch), ('{', '}') | ('(', ')') | ('[', ']')) {
                    return Err("括号或花括号没有正确配对");
                }
            }
            _ => {}
        }
    }

    if in_comment {
        return Err("注释没有正确闭合");
    }
    if in_single_quote || in_double_quote {
        return Err("字符串引号没有正确闭合");
    }
    if !stack.is_empty() {
        return Err("存在未闭合的括号或花括号");
    }

    Ok(())
}

/// 校验配置包：版本、设置取值范围、自定义 CSS 是否闭合，以及归一化后不允许出现重复的 feed URL。
pub fn validate_config_package(package: &ConfigPackage) -> crate::Result<()> {
    if package.version != CONFIG_PACKAGE_VERSION {
        return Err(invalid(format!("配置包版本必须等于 {CONFIG_PACKAGE_VERSION}")));
    }
    validate_user_settings(&package.settings)?;
    // 导入与 WebDAV 拉取此前完全不看 custom_css，一段没闭合的 CSS 会直接进 `<style>`，
    // 把后面的样式规则整段吃掉。设置页保存路径早就有这道校验，这里补上的是导入侧的缺口。
    validate_custom_css(&package.settings.custom_css)
        .map_err(|reason| invalid(format!("自定义 CSS 格式无效：{reason}")))?;

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

    use super::{
        CONFIG_PACKAGE_VERSION, validate_config_package, validate_custom_css,
        validate_user_settings,
    };
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
    fn accepts_css_with_quotes_comments_and_nesting() {
        let cases = [
            "",
            ":root { --x: 1px; }",
            "/* } 注释里的右花括号不算 */ a { color: red; }",
            "a::after { content: \"}\"; }",
            "a::after { content: '{'; }",
            "a::after { content: \"\\\"\"; }",
            "@media (min-width: 40rem) { a { color: red; } }",
            "a { background: url(data:image/svg+xml;base64,AAAA); }",
        ];

        for case in cases {
            assert!(validate_custom_css(case).is_ok(), "应当接受：{case}");
        }
    }

    #[test]
    fn rejects_css_that_would_swallow_following_rules() {
        let cases = [
            ("a { color: red;", "存在未闭合的括号或花括号"),
            ("a { color: red; } }", "存在未匹配的右括号或右花括号"),
            ("a { color: red; ]", "括号或花括号没有正确配对"),
            ("a { content: \"未闭合; }", "字符串引号没有正确闭合"),
            ("/* 未闭合注释 a { color: red; }", "注释没有正确闭合"),
        ];

        for (case, expected) in cases {
            assert_eq!(validate_custom_css(case), Err(expected), "应当拒绝：{case}");
        }
    }

    /// 导入路径此前完全不看 `custom_css`，一段没闭合的 CSS 会直接进 `<style>`。
    #[test]
    fn rejects_config_package_with_unbalanced_custom_css() {
        let invalid = package(
            Vec::new(),
            UserSettings { custom_css: "a { color: red;".to_string(), ..UserSettings::default() },
        );

        let error = validate_config_package(&invalid).expect_err("expected rejection");

        assert!(error.to_string().contains("自定义 CSS 格式无效"));
    }

    /// 反面：设置页保存路径不经过 `validate_config_package`，也不该因为 CSS 校验而改变行为。
    /// `validate_user_settings` 必须继续放行任何 `custom_css`，否则用户早已存好的 CSS
    /// 可能在某次保存时突然被拒。
    #[test]
    fn user_settings_validation_ignores_custom_css() {
        let settings =
            UserSettings { custom_css: "a { color: red;".to_string(), ..UserSettings::default() };

        assert!(validate_user_settings(&settings).is_ok());
    }

    #[test]
    fn rejects_invalid_feed_url() {
        let invalid = package(vec![feed("not-a-url")], UserSettings::default());

        let error = validate_config_package(&invalid).expect_err("expected rejection");

        assert!(error.to_string().contains("无效的 feed URL"));
    }
}

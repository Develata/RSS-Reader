use anyhow::Result;
use rssr_domain::ConfigPackage;

/// 导入的名称和分组总是“设置或清除”：文件里没有就清除（空串），不管订阅原来是否存在、是否
/// 已被删除。以前按导入前的订阅快照区分，重新激活已删除的订阅时会留下旧值（审计 B6）。
pub(super) fn import_field(value: Option<String>) -> String {
    value.unwrap_or_default()
}

/// 配置包校验的唯一实现在 `rssr_domain::validation`，这里只做错误类型转换。
/// 不要在本层复制一份规则：曾经三处各写一份，规则很快就出现了分叉。
pub(super) fn validate_config_package(package: &ConfigPackage) -> Result<()> {
    Ok(rssr_domain::validate_config_package(package)?)
}

#[cfg(test)]
mod tests {
    use rssr_domain::{ConfigPackage, UserSettings};
    use time::OffsetDateTime;

    use super::validate_config_package;

    #[test]
    fn rejects_config_package_with_zero_entries_page_size() {
        let settings = UserSettings { entries_page_size: 0, ..UserSettings::default() };
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

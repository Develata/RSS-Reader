use std::{fs, path::Path};

use rssr_domain::ConfigPackage;

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

/// 配置包校验的唯一实现在 `rssr_domain::validation`，这里只做错误类型转换。
/// 本层曾经自带一份更宽松的规则（接受 `version >= 1`、不校验 `entries_page_size`），
/// 结果同一份配置包在文件路径和导入路径上的结论不一致。
pub fn validate_config_package(package: &ConfigPackage) -> anyhow::Result<()> {
    Ok(rssr_domain::validate_config_package(package)?)
}

use rssr_domain::ConfigPackage;

pub fn encode_config_package(package: &ConfigPackage) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(package)?)
}

pub fn decode_config_package(raw: &str) -> anyhow::Result<ConfigPackage> {
    Ok(serde_json::from_str(raw)?)
}

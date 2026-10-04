fn main() {
    println!("cargo:rerun-if-env-changed=RELEASE_TAG");
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os != "windows" || !matches!(target_env.as_str(), "gnu" | "msvc") {
        return;
    }

    let icon_path = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("../../icons/icon.ico");
    let mut resource = winresource::WindowsResource::new();
    resource.set_icon(icon_path.to_string_lossy().as_ref());

    // Release workflows provide the tag; local builds retain the Cargo version.
    let release_tag =
        std::env::var("RELEASE_TAG").unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_owned());
    let version = release_tag.trim().strip_prefix('v').unwrap_or(release_tag.trim());
    let parts: Vec<u16> = version
        .split('.')
        .map(|part| part.parse().expect("release version must contain u16 numbers"))
        .collect();
    assert_eq!(parts.len(), 3, "release version must be major.minor.patch");
    let numeric_version =
        (u64::from(parts[0]) << 48) | (u64::from(parts[1]) << 32) | (u64::from(parts[2]) << 16);
    resource
        .set("FileVersion", version)
        .set("ProductVersion", version)
        .set_version_info(winresource::VersionInfo::FILEVERSION, numeric_version)
        .set_version_info(winresource::VersionInfo::PRODUCTVERSION, numeric_version);
    resource.compile().expect("embed Windows icon resource");
}

fn main() {
    #[cfg(target_os = "windows")]
    {
        let icon_path = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("../../icons/icon.ico");
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon(icon_path.to_string_lossy().as_ref());
        resource.compile().expect("embed Windows icon resource");
    }
}

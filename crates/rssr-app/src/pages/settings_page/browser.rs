const REPOSITORY_URL: &str = "https://github.com/Develata/RSS-Reader";

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn open_repository_url() -> Result<(), String> {
    webbrowser::open(REPOSITORY_URL).map(|_| ()).map_err(|err| err.to_string())
}

#[cfg(target_arch = "wasm32")]
pub(super) fn open_repository_url() -> Result<(), String> {
    web_sys::window()
        .ok_or_else(|| "浏览器窗口不可用".to_string())?
        .open_with_url_and_target(REPOSITORY_URL, "_blank")
        .map(|_| ())
        .map_err(|err| format!("{err:?}"))
}

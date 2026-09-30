//! Deployment policy only; application/domain behavior remains shared.
pub(crate) const ENABLED: bool = cfg!(all(target_arch = "wasm32", feature = "pages-demo"));
pub(crate) const NOTICE: &str = "公开静态演示：首次使用载入合成样例，操作保存在当前浏览器。GitHub Pages 没有 feed 代理；受浏览器 CORS 限制，本演示已禁用添加订阅、手动/自动刷新和远端配置同步。阅读、搜索、收藏与主题设置可用。";

#[cfg(target_arch = "wasm32")]
pub(crate) fn require_network() -> anyhow::Result<()> {
    anyhow::ensure!(
        !ENABLED,
        "静态演示已禁用此网络操作：GitHub Pages 不提供 feed/CORS 代理。请使用桌面版或自行部署 rssr-web。"
    );
    Ok(())
}

#[cfg(all(target_arch = "wasm32", feature = "pages-demo"))]
pub(crate) fn initial_state()
-> anyhow::Result<rssr_infra::application_adapters::browser::state::BrowserState> {
    use rssr_infra::application_adapters::browser::state::BrowserState;
    Ok(BrowserState {
        core: serde_json::from_str(include_str!(
            "../../../tests/fixtures/browser_state/reader_demo_core.json"
        ))?,
        app_state: serde_json::from_str(include_str!(
            "../../../tests/fixtures/browser_state/reader_demo_app_state.json"
        ))?,
        entry_flags: serde_json::from_str(include_str!(
            "../../../tests/fixtures/browser_state/reader_demo_entry_flags.json"
        ))?,
        entry_content: serde_json::from_str(include_str!(
            "../../../tests/fixtures/browser_state/reader_demo_entry_content.json"
        ))?,
    })
}

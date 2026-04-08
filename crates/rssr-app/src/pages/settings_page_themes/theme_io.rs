use dioxus::prelude::*;
use crate::status::{set_status_error, set_status_info};

#[cfg(not(target_arch = "wasm32"))]
async fn pick_css_file_contents() -> anyhow::Result<Option<String>> {
    #[cfg(target_os = "android")]
    {
        anyhow::bail!("Android 端暂未接入系统文件选择器，请先手动粘贴 CSS 内容。");
    }

    #[cfg(not(target_os = "android"))]
    {
        let file = rfd::AsyncFileDialog::new().add_filter("CSS", &["css"]).pick_file().await;

        let Some(file) = file else {
            return Ok(None);
        };

        let bytes = file.read().await;
        let raw = String::from_utf8(bytes)
            .map_err(|err| anyhow::anyhow!("CSS 文件不是有效 UTF-8：{err}"))?;
        Ok(Some(raw))
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn import_css_file(
    theme: crate::theme::ThemeController,
    draft: Signal<rssr_domain::UserSettings>,
    preset_choice: Signal<String>,
    status: Signal<String>,
    status_tone: Signal<String>,
) {
    use super::theme_apply::apply_custom_css_from_raw;

    spawn(async move {
        match pick_css_file_contents().await {
            Ok(Some(raw)) => apply_custom_css_from_raw(
                theme,
                draft,
                preset_choice,
                status,
                status_tone,
                raw,
                "已从文件载入并应用自定义 CSS。".to_string(),
            ),
            Ok(None) => set_status_info(status, status_tone, "已取消载入 CSS 文件。"),
            Err(err) => set_status_error(status, status_tone, format!("载入 CSS 文件失败：{err}")),
        }
    });
}

#[cfg(target_arch = "wasm32")]
pub(super) fn trigger_css_file_input_in_browser() -> anyhow::Result<()> {
    use js_sys::wasm_bindgen::JsCast;

    let window = web_sys::window().ok_or_else(|| anyhow::anyhow!("浏览器窗口不可用"))?;
    let document = window.document().ok_or_else(|| anyhow::anyhow!("浏览器文档不可用"))?;
    let input = document
        .get_element_by_id("custom-css-file-input")
        .ok_or_else(|| anyhow::anyhow!("未找到 CSS 文件输入节点"))?
        .dyn_into::<web_sys::HtmlInputElement>()
        .map_err(|_| anyhow::anyhow!("CSS 文件输入节点类型不匹配"))?;
    input.set_value("");
    input.click();

    Ok(())
}

pub(super) fn export_css_file(raw: String, status: Signal<String>, status_tone: Signal<String>) {
    if raw.trim().is_empty() {
        set_status_info(status, status_tone, "当前没有可导出的自定义 CSS。");
        return;
    }

    #[cfg(target_arch = "wasm32")]
    {
        match save_css_file_in_browser(&raw) {
            Ok(()) => set_status_info(status, status_tone, "已导出当前自定义 CSS。"),
            Err(err) => set_status_error(status, status_tone, format!("导出 CSS 文件失败：{err}")),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        spawn(async move {
            match save_css_file(&raw).await {
                Ok(true) => set_status_info(status, status_tone, "已导出当前自定义 CSS。"),
                Ok(false) => set_status_info(status, status_tone, "已取消导出 CSS 文件。"),
                Err(err) => {
                    set_status_error(status, status_tone, format!("导出 CSS 文件失败：{err}"))
                }
            }
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn save_css_file(raw: &str) -> anyhow::Result<bool> {
    #[cfg(target_os = "android")]
    {
        let _ = raw;
        anyhow::bail!("Android 端暂未接入系统文件保存器，请先复制 CSS 内容后手动保存。");
    }

    #[cfg(not(target_os = "android"))]
    {
        let file = rfd::AsyncFileDialog::new()
            .set_file_name("rss-reader-theme.css")
            .add_filter("CSS", &["css"])
            .save_file()
            .await;

        let Some(file) = file else {
            return Ok(false);
        };

        file.write(raw.as_bytes()).await?;
        Ok(true)
    }
}

#[cfg(target_arch = "wasm32")]
fn save_css_file_in_browser(raw: &str) -> anyhow::Result<()> {
    use js_sys::wasm_bindgen::{JsCast, JsValue};

    let window = web_sys::window().ok_or_else(|| anyhow::anyhow!("浏览器窗口不可用"))?;
    let document = window.document().ok_or_else(|| anyhow::anyhow!("浏览器文档不可用"))?;
    let body = document.body().ok_or_else(|| anyhow::anyhow!("浏览器页面 body 不可用"))?;

    let parts = js_sys::Array::new();
    parts.push(&JsValue::from_str(raw));
    let bag = web_sys::BlobPropertyBag::new();
    bag.set_type("text/css;charset=utf-8");
    let blob = web_sys::Blob::new_with_str_sequence_and_options(&parts, &bag)
        .map_err(|err| anyhow::anyhow!("创建 CSS 导出内容失败: {err:?}"))?;

    let object_url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|err| anyhow::anyhow!("创建下载链接失败: {err:?}"))?;

    let anchor = document
        .create_element("a")
        .map_err(|err| anyhow::anyhow!("创建下载节点失败: {err:?}"))?
        .dyn_into::<web_sys::HtmlAnchorElement>()
        .map_err(|_| anyhow::anyhow!("下载节点类型不匹配"))?;

    anchor.set_href(&object_url);
    anchor.set_download("rss-reader-theme.css");
    anchor
        .style()
        .set_property("display", "none")
        .map_err(|err| anyhow::anyhow!("设置下载节点样式失败: {err:?}"))?;

    body.append_child(&anchor).map_err(|err| anyhow::anyhow!("插入下载节点失败: {err:?}"))?;
    anchor.click();
    let _ = body.remove_child(&anchor);
    let _ = web_sys::Url::revoke_object_url(&object_url);

    Ok(())
}

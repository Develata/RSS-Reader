#[cfg(not(target_arch = "wasm32"))]
pub(super) async fn pick_css_file_contents() -> anyhow::Result<Option<String>> {
    #[cfg(target_os = "android")]
    {
        super::theme_android_import::pick_css_bytes().await?.map(decode_css_file).transpose()
    }

    #[cfg(not(target_os = "android"))]
    {
        let file = rfd::AsyncFileDialog::new().add_filter("CSS", &["css"]).pick_file().await;
        read_css_file_selection(file).await
    }
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
async fn read_css_file_selection(file: Option<rfd::FileHandle>) -> anyhow::Result<Option<String>> {
    let Some(file) = file else { return Ok(None) };
    // rfd::FileHandle::read panics on I/O errors; the page needs a recoverable failure.
    let bytes = tokio::fs::read(file.path()).await?;
    decode_css_file(bytes).map(Some)
}

fn decode_css_file(bytes: Vec<u8>) -> anyhow::Result<String> {
    String::from_utf8(bytes).map_err(|err| anyhow::anyhow!("CSS 文件不是有效 UTF-8：{err}"))
}

#[cfg(target_arch = "wasm32")]
pub(super) async fn read_css_file_in_browser(
    file: dioxus::html::FileData,
) -> anyhow::Result<String> {
    let file = file
        .inner()
        .downcast_ref::<web_sys::File>()
        .ok_or_else(|| anyhow::anyhow!("无法访问浏览器选择的 CSS 文件"))?;
    // The existing gloo reader reports error events and aborts when the page task is dropped.
    // Read bytes first so malformed UTF-8 cannot silently become replacement characters.
    let bytes = gloo_file::futures::read_as_bytes(&gloo_file::Blob::from(file.clone())).await?;
    let mut raw = decode_css_file(bytes)?;
    // Preserve FileReader::readAsText's handling of the optional UTF-8 BOM on Web.
    if raw.starts_with('\u{feff}') {
        raw.drain(..3);
    }
    Ok(raw)
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

#[cfg(not(target_arch = "wasm32"))]
pub(super) async fn save_css_file(raw: &str) -> anyhow::Result<bool> {
    #[cfg(target_os = "android")]
    {
        return super::theme_android_export::save_css_file_with_android_picker(raw).await;
    }

    #[cfg(not(target_os = "android"))]
    {
        let file = rfd::AsyncFileDialog::new()
            .set_file_name("rss-reader-theme.css")
            .add_filter("CSS", &["css"])
            .save_file()
            .await;

        write_css_file_selection(raw, file).await
    }
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
async fn write_css_file_selection(
    raw: &str,
    file: Option<rfd::FileHandle>,
) -> anyhow::Result<bool> {
    let Some(file) = file else { return Ok(false) };
    file.write(raw.as_bytes()).await?;
    Ok(true)
}

#[cfg(target_arch = "wasm32")]
pub(super) fn save_css_file_in_browser(raw: &str) -> anyhow::Result<()> {
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

    let anchor = document
        .create_element("a")
        .map_err(|err| anyhow::anyhow!("创建下载节点失败: {err:?}"))?
        .dyn_into::<web_sys::HtmlAnchorElement>()
        .map_err(|_| anyhow::anyhow!("下载节点类型不匹配"))?;

    anchor.set_download("rss-reader-theme.css");
    anchor
        .style()
        .set_property("display", "none")
        .map_err(|err| anyhow::anyhow!("设置下载节点样式失败: {err:?}"))?;

    let object_url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|err| anyhow::anyhow!("创建下载链接失败: {err:?}"))?;
    anchor.set_href(&object_url);
    let result = (|| {
        body.append_child(&anchor).map_err(|err| anyhow::anyhow!("插入下载节点失败: {err:?}"))?;
        anchor.click();
        Ok(())
    })();
    let _ = body.remove_child(&anchor);
    let _ = web_sys::Url::revoke_object_url(&object_url);

    result
}

#[cfg(all(test, not(target_arch = "wasm32"), not(target_os = "android")))]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!(
                "rssr-theme-file-{}-{}",
                std::process::id(),
                SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
            ));
            std::fs::create_dir(&dir).unwrap();
            Self(dir)
        }

        fn file(&self) -> PathBuf {
            self.0.join("theme.css")
        }
        fn selected(&self) -> Option<rfd::FileHandle> {
            Some(self.file().into())
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(self.file());
            let _ = std::fs::remove_dir(&self.0);
        }
    }

    #[tokio::test]
    async fn selected_file_round_trips_exact_utf8_bytes() {
        let fixture = Fixture::new();
        for raw in ["", "\u{feff}/* 主题 */\r\nbody { --测试: 1; }\r\n"] {
            assert!(write_css_file_selection(raw, fixture.selected()).await.unwrap());
            assert_eq!(std::fs::read(fixture.file()).unwrap(), raw.as_bytes());
            assert_eq!(
                read_css_file_selection(fixture.selected()).await.unwrap().as_deref(),
                Some(raw)
            );
        }
    }

    #[tokio::test]
    async fn cancelled_selection_does_not_read_or_write() {
        assert_eq!(read_css_file_selection(None).await.unwrap(), None);
        assert!(!write_css_file_selection("/* unchanged */", None).await.unwrap());
    }

    #[tokio::test]
    async fn invalid_encoding_is_an_error_without_rewriting_the_source() {
        let fixture = Fixture::new();
        let bytes = b"/*\xff*/";
        std::fs::write(fixture.file(), bytes).unwrap();
        let error = read_css_file_selection(fixture.selected()).await.unwrap_err();
        assert!(error.to_string().starts_with("CSS 文件不是有效 UTF-8："));
        assert_eq!(std::fs::read(fixture.file()).unwrap(), bytes);
    }

    #[tokio::test]
    async fn missing_selected_file_reports_error_instead_of_panicking() {
        let fixture = Fixture::new();
        assert!(read_css_file_selection(fixture.selected()).await.is_err());
    }

    #[tokio::test]
    async fn failed_write_can_retry_with_the_same_bytes() {
        let fixture = Fixture::new();
        let raw = "/* 保存失败后重试 */";
        assert!(write_css_file_selection(raw, Some(fixture.0.clone().into())).await.is_err());
        assert!(!fixture.file().exists());
        assert!(write_css_file_selection(raw, fixture.selected()).await.unwrap());
        assert_eq!(std::fs::read(fixture.file()).unwrap(), raw.as_bytes());
    }
}

use dioxus::prelude::document;

pub(super) async fn read_browser_clipboard_text() -> anyhow::Result<Option<String>> {
    document::eval(
        r#"
        if (typeof navigator === "undefined" || !navigator.clipboard || !navigator.clipboard.readText) {
            return null;
        }
        return navigator.clipboard.readText();
        "#,
    )
    .join::<Option<String>>()
    .await
    .map_err(|err| anyhow::anyhow!(err.to_string()))
}

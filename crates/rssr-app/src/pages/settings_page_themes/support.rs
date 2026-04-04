use dioxus::prelude::*;
use rssr_domain::UserSettings;

use crate::{
    bootstrap::AppServices,
    status::{set_status_error, set_status_info},
    theme::ThemeController,
};

fn newsprint_theme_css() -> &'static str {
    include_str!("../../../../../assets/themes/newsprint.css")
}

fn atlas_sidebar_theme_css() -> &'static str {
    include_str!("../../../../../assets/themes/atlas-sidebar.css")
}

fn forest_desk_theme_css() -> &'static str {
    include_str!("../../../../../assets/themes/forest-desk.css")
}

fn midnight_ledger_theme_css() -> &'static str {
    include_str!("../../../../../assets/themes/midnight-ledger.css")
}

pub(super) fn preset_css(key: &str) -> &'static str {
    match key {
        "none" => "",
        "atlas-sidebar" => atlas_sidebar_theme_css(),
        "newsprint" => newsprint_theme_css(),
        "forest-desk" => forest_desk_theme_css(),
        "midnight-ledger" => midnight_ledger_theme_css(),
        _ => "",
    }
}

pub(super) fn preset_display_name(key: &str) -> &'static str {
    match key {
        "atlas-sidebar" => "Atlas Sidebar",
        "newsprint" => "Newsprint",
        "forest-desk" => "Amethyst Glass",
        "midnight-ledger" => "Midnight Ledger",
        _ => "自定义主题",
    }
}

pub(crate) fn detect_preset_key(raw: &str) -> &'static str {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        "none"
    } else if trimmed == atlas_sidebar_theme_css().trim() {
        "atlas-sidebar"
    } else if trimmed == newsprint_theme_css().trim() {
        "newsprint"
    } else if trimmed == forest_desk_theme_css().trim() {
        "forest-desk"
    } else if trimmed == midnight_ledger_theme_css().trim() {
        "midnight-ledger"
    } else {
        "custom"
    }
}

pub(super) fn custom_css_source_label(raw: &str) -> &'static str {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        "未启用自定义 CSS"
    } else if trimmed == atlas_sidebar_theme_css().trim() {
        "内置主题：Atlas Sidebar"
    } else if trimmed == newsprint_theme_css().trim() {
        "内置主题：Newsprint"
    } else if trimmed == forest_desk_theme_css().trim() {
        "内置主题：Amethyst Glass"
    } else if trimmed == midnight_ledger_theme_css().trim() {
        "内置主题：Midnight Ledger"
    } else {
        "自定义主题"
    }
}

pub(super) fn apply_builtin_theme(
    theme: ThemeController,
    mut draft: Signal<UserSettings>,
    mut preset_choice: Signal<String>,
    status: Signal<String>,
    status_tone: Signal<String>,
    preset_key: &str,
    preset_name: &str,
) {
    let mut next = draft();
    next.custom_css = preset_css(preset_key).to_string();
    preset_choice.set(preset_key.to_string());
    let applied = next.clone();
    draft.set(next);
    apply_settings_immediately(
        theme,
        draft,
        preset_choice,
        status,
        status_tone,
        applied,
        format!("已应用示例主题：{preset_name}。"),
    );
}

pub(super) fn apply_settings_immediately(
    mut theme: ThemeController,
    mut draft: Signal<UserSettings>,
    mut preset_choice: Signal<String>,
    status: Signal<String>,
    status_tone: Signal<String>,
    next: UserSettings,
    success_message: String,
) {
    let previous = (theme.settings)();
    let previous_preset = detect_preset_key(&previous.custom_css).to_string();
    theme.settings.set(next.clone());
    spawn(async move {
        match AppServices::shared().await {
            Ok(services) => match services.save_settings(&next).await {
                Ok(()) => set_status_info(status, status_tone, success_message),
                Err(err) => {
                    theme.settings.set(previous.clone());
                    draft.set(previous);
                    preset_choice.set(previous_preset);
                    set_status_error(status, status_tone, format!("保存设置失败：{err}"));
                }
            },
            Err(err) => {
                theme.settings.set(previous.clone());
                draft.set(previous);
                preset_choice.set(previous_preset);
                set_status_error(status, status_tone, format!("初始化应用失败：{err}"));
            }
        }
    });
}

#[derive(Clone, Copy)]
pub(super) struct BuiltinThemePreset {
    pub key: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub notes: &'static str,
    pub swatches: [&'static str; 3],
}

pub(super) fn builtin_theme_presets() -> [BuiltinThemePreset; 4] {
    [
        BuiltinThemePreset {
            key: "atlas-sidebar",
            name: "Atlas Sidebar",
            description: "把应用改成更接近侧栏工作台的布局，导航和内容区彻底分栏。",
            notes: "头部变成左侧信息栏，页面导航垂直停靠，整体更像编辑器或知识库工具。",
            swatches: ["#f1efe8", "#b24c3d", "#1f2430"],
        },
        BuiltinThemePreset {
            key: "newsprint",
            name: "Newsprint",
            description: "偏纸面和报刊感，标题更有旧式出版物气质。",
            notes: "更窄圆角、两列统计卡片、阅读页更长的正文版心。",
            swatches: ["#efe6d6", "#8b3d2b", "#241d16"],
        },
        BuiltinThemePreset {
            key: "forest-desk",
            name: "Amethyst Glass",
            description: "紫蓝渐变和高通透毛玻璃，界面更梦幻，也更轻盈。",
            notes: "发光药丸按钮、玻璃面板和更宽松的阅读排版，适合沉浸式浏览。",
            swatches: ["#e0c3fc", "#8b5cf6", "#1f2937"],
        },
        BuiltinThemePreset {
            key: "midnight-ledger",
            name: "Midnight Ledger",
            description: "深色账本风，强调夜间阅读和更稳的对比。",
            notes: "深底配冷色强调，卡片层次更明显，正文更沉浸。",
            swatches: ["#0f1518", "#4fb7b1", "#eef5f7"],
        },
    ]
}

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
    theme: ThemeController,
    draft: Signal<UserSettings>,
    preset_choice: Signal<String>,
    status: Signal<String>,
    status_tone: Signal<String>,
) {
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

pub(super) fn apply_custom_css_from_raw(
    theme: ThemeController,
    mut draft: Signal<UserSettings>,
    mut preset_choice: Signal<String>,
    status: Signal<String>,
    status_tone: Signal<String>,
    raw: String,
    success_message: String,
) {
    if let Err(err) = validate_custom_css(&raw) {
        set_status_error(status, status_tone, format!("自定义 CSS 格式无效：{err}"));
        return;
    }

    let mut next = draft();
    next.custom_css = raw;
    preset_choice.set(detect_preset_key(&next.custom_css).to_string());
    let applied = next.clone();
    draft.set(next);
    apply_settings_immediately(
        theme,
        draft,
        preset_choice,
        status,
        status_tone,
        applied,
        success_message,
    );
}

pub(super) fn validate_custom_css(raw: &str) -> Result<(), &'static str> {
    let mut stack = Vec::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut in_comment = false;
    let mut escaped = false;
    let mut chars = raw.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_comment {
            if ch == '*' && chars.peek() == Some(&'/') {
                let _ = chars.next();
                in_comment = false;
            }
            continue;
        }

        if escaped {
            escaped = false;
            continue;
        }

        match ch {
            '\\' if in_single_quote || in_double_quote => escaped = true,
            '\'' if !in_double_quote => in_single_quote = !in_single_quote,
            '"' if !in_single_quote => in_double_quote = !in_single_quote,
            '/' if !in_single_quote && !in_double_quote && chars.peek() == Some(&'*') => {
                let _ = chars.next();
                in_comment = true;
            }
            '{' | '(' | '[' if !in_single_quote && !in_double_quote => stack.push(ch),
            '}' | ')' | ']' if !in_single_quote && !in_double_quote => {
                let Some(open) = stack.pop() else {
                    return Err("存在未匹配的右括号或右花括号");
                };
                if !matches!((open, ch), ('{', '}') | ('(', ')') | ('[', ']')) {
                    return Err("括号或花括号没有正确配对");
                }
            }
            _ => {}
        }
    }

    if in_comment {
        return Err("注释没有正确闭合");
    }
    if in_single_quote || in_double_quote {
        return Err("字符串引号没有正确闭合");
    }
    if !stack.is_empty() {
        return Err("存在未闭合的括号或花括号");
    }

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

#[cfg(test)]
mod tests {
    use super::{
        custom_css_source_label, detect_preset_key, forest_desk_theme_css, newsprint_theme_css,
    };

    #[test]
    fn detect_preset_key_keeps_unknown_css_as_custom() {
        let custom = ":root { --panel: #123456; }\n[data-page=\"reader\"] { gap: 99px; }";
        assert_eq!(detect_preset_key(custom), "custom");
        assert_eq!(custom_css_source_label(custom), "自定义主题");
    }

    #[test]
    fn detect_preset_key_matches_builtin_presets_exactly() {
        assert_eq!(detect_preset_key(""), "none");
        assert_eq!(detect_preset_key(newsprint_theme_css()), "newsprint");
        assert_eq!(detect_preset_key(forest_desk_theme_css()), "forest-desk");
    }
}

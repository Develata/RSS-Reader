use dioxus::prelude::*;
use rssr_domain::{ListDensity, StartupView, ThemeMode};

use crate::{
    app::AppNav, bootstrap::AppServices, components::status_banner::StatusBanner,
    theme::ThemeController,
};

#[component]
pub fn SettingsPage() -> Element {
    let mut theme = use_context::<ThemeController>();
    let mut draft = use_signal(|| (theme.settings)());
    let mut preset_choice =
        use_signal(|| detect_preset_key(&(theme.settings)().custom_css).to_string());
    let mut endpoint = use_signal(String::new);
    let mut remote_path = use_signal(|| "config/rss-reader.json".to_string());
    let mut status = use_signal(|| "在这里管理主题、阅读偏好和远端配置交换。".to_string());

    let _ = use_resource(move || async move {
        match AppServices::shared().await {
            Ok(services) => match services.load_settings().await {
                Ok(settings) => {
                    preset_choice.set(detect_preset_key(&settings.custom_css).to_string());
                    draft.set(settings);
                }
                Err(err) => status.set(format!("读取设置失败：{err}")),
            },
            Err(err) => status.set(format!("初始化应用失败：{err}")),
        }
    });

    rsx! {
        section { class: "page page-settings", "data-page": "settings",
            AppNav {}
            h2 { "设置" }
            StatusBanner { message: status(), tone: "info".to_string() }
            div { class: "settings-grid",
                div { class: "settings-card",
                    h3 { "阅读外观" }
                    label { class: "field-label", "主题" }
                    select {
                        class: "select-input",
                        "data-action": "theme-mode",
                        value: "{theme_value(draft().theme)}",
                        onchange: move |event| {
                            let mut next = draft();
                            next.theme = parse_theme_mode(&event.value());
                            draft.set(next);
                        },
                        option { value: "system", "跟随系统" }
                        option { value: "light", "浅色" }
                        option { value: "dark", "深色" }
                    }
                    label { class: "field-label", "列表密度" }
                    select {
                        class: "select-input",
                        "data-action": "list-density",
                        value: "{density_value(draft().list_density)}",
                        onchange: move |event| {
                            let mut next = draft();
                            next.list_density = parse_list_density(&event.value());
                            draft.set(next);
                        },
                        option { value: "comfortable", "舒适" }
                        option { value: "compact", "紧凑" }
                    }
                    label { class: "field-label", "启动视图" }
                    select {
                        class: "select-input",
                        "data-action": "startup-view",
                        value: "{startup_value(draft().startup_view)}",
                        onchange: move |event| {
                            let mut next = draft();
                            next.startup_view = parse_startup_view(&event.value());
                            draft.set(next);
                        },
                        option { value: "all", "全部文章" }
                        option { value: "last_feed", "上次订阅" }
                    }
                    label { class: "field-label", "刷新间隔（分钟）" }
                    input {
                        class: "text-input",
                        "data-action": "refresh-interval",
                        value: "{draft().refresh_interval_minutes}",
                        oninput: move |event| {
                            if let Ok(minutes) = event.value().parse::<u32>() {
                                let mut next = draft();
                                next.refresh_interval_minutes = minutes;
                                draft.set(next);
                            }
                        }
                    }
                    label { class: "field-label", "阅读字号缩放" }
                    input {
                        class: "text-input",
                        "data-action": "reader-font-scale",
                        value: "{draft().reader_font_scale}",
                        oninput: move |event| {
                            if let Ok(scale) = event.value().parse::<f32>() {
                                let mut next = draft();
                                next.reader_font_scale = scale;
                                draft.set(next);
                            }
                        }
                    }
                    label { class: "field-label", "自定义 CSS" }
                    textarea {
                        class: "text-area",
                        "data-action": "custom-css",
                        value: "{draft().custom_css}",
                        placeholder: "[data-page=\"reader\"] .reader-body {{ max-width: 72ch; }}",
                        oninput: move |event| {
                            let mut next = draft();
                            next.custom_css = event.value();
                            preset_choice.set(detect_preset_key(&next.custom_css).to_string());
                            draft.set(next);
                        }
                    }
                    p {
                        class: "page-intro",
                        "data-action": "current-custom-css-source",
                        "当前样式来源：{custom_css_source_label(&draft().custom_css)}"
                    }
                    div { class: "inline-actions",
                        button {
                            class: "button secondary",
                            "data-action": "import-custom-css-file",
                            onclick: move |_| {
                                let mut draft = draft;
                                let mut status = status;
                                spawn(async move {
                                    match pick_css_file_contents().await {
                                        Ok(Some(raw)) => {
                                            let mut next = draft();
                                            next.custom_css = raw;
                                            preset_choice.set(detect_preset_key(&next.custom_css).to_string());
                                            draft.set(next);
                                            status.set("已从文件载入自定义 CSS。点击“保存设置”即可生效。".to_string());
                                        }
                                        Ok(None) => status.set("已取消载入 CSS 文件。".to_string()),
                                        Err(err) => status.set(format!("载入 CSS 文件失败：{err}")),
                                    }
                                });
                            },
                            "导入主题文件"
                        }
                        button {
                            class: "button secondary",
                            "data-action": "export-custom-css-file",
                            onclick: move |_| {
                                let raw = draft().custom_css;
                                let mut status = status;
                                spawn(async move {
                                    if raw.trim().is_empty() {
                                        status.set("当前没有可导出的自定义 CSS。".to_string());
                                        return;
                                    }

                                    match save_css_file(&raw).await {
                                        Ok(true) => status.set("已导出当前自定义 CSS。".to_string()),
                                        Ok(false) => status.set("已取消导出 CSS 文件。".to_string()),
                                        Err(err) => status.set(format!("导出 CSS 文件失败：{err}")),
                                    }
                                });
                            },
                            "导出当前 CSS"
                        }
                    }
                    label { class: "field-label", "内置主题预设" }
                    div { class: "inline-actions",
                        select {
                            class: "select-input",
                            "data-action": "preset-theme-select",
                            value: "{preset_choice}",
                            onchange: move |event| preset_choice.set(event.value()),
                            option { value: "none", "无预设" }
                            option { value: "newsprint", "Newsprint" }
                            option { value: "forest-desk", "Forest Desk" }
                            option { value: "midnight-ledger", "Midnight Ledger" }
                        }
                        button {
                            class: "button secondary",
                            "data-action": "apply-selected-theme",
                            onclick: move |_| {
                                let choice = preset_choice();
                                if choice == "none" {
                                    let mut next = draft();
                                    next.custom_css.clear();
                                    draft.set(next);
                                    status.set("已清空自定义 CSS。点击“保存设置”即可生效。".to_string());
                                    return;
                                }
                                let mut next = draft();
                                next.custom_css = preset_css(choice.as_str()).to_string();
                                preset_choice.set(choice.clone());
                                draft.set(next);
                                status.set(format!(
                                    "已载入示例主题：{}。点击“保存设置”即可生效。",
                                    preset_display_name(choice.as_str())
                                ));
                            },
                            "载入所选主题"
                        }
                    }
                    p { class: "page-intro", "可直接载入内置示例主题，或清空当前自定义 CSS。载入后点击“保存设置”生效。" }
                    div { class: "preset-grid",
                        button {
                            class: "button secondary",
                            "data-action": "apply-theme-newsprint",
                            onclick: move |_| {
                                let mut next = draft();
                                next.custom_css = newsprint_theme_css().to_string();
                                preset_choice.set("newsprint".to_string());
                                draft.set(next);
                                status.set("已载入示例主题：Newsprint。点击“保存设置”即可生效。".to_string());
                            },
                            "Newsprint"
                        }
                        button {
                            class: "button secondary",
                            "data-action": "apply-theme-forest-desk",
                            onclick: move |_| {
                                let mut next = draft();
                                next.custom_css = forest_desk_theme_css().to_string();
                                preset_choice.set("forest-desk".to_string());
                                draft.set(next);
                                status.set("已载入示例主题：Forest Desk。点击“保存设置”即可生效。".to_string());
                            },
                            "Forest Desk"
                        }
                        button {
                            class: "button secondary",
                            "data-action": "apply-theme-midnight-ledger",
                            onclick: move |_| {
                                let mut next = draft();
                                next.custom_css = midnight_ledger_theme_css().to_string();
                                preset_choice.set("midnight-ledger".to_string());
                                draft.set(next);
                                status.set("已载入示例主题：Midnight Ledger。点击“保存设置”即可生效。".to_string());
                            },
                            "Midnight Ledger"
                        }
                        button {
                            class: "button secondary danger-outline",
                            "data-action": "clear-custom-css",
                            onclick: move |_| {
                                let mut next = draft();
                                next.custom_css.clear();
                                preset_choice.set("none".to_string());
                                draft.set(next);
                                status.set("已清空自定义 CSS。点击“保存设置”即可生效。".to_string());
                            },
                            "清空 CSS"
                        }
                    }
                    button {
                        class: "button",
                        "data-action": "save-settings",
                        onclick: move |_| {
                            let next = draft();
                            let mut status = status;
                            spawn(async move {
                                match AppServices::shared().await {
                                    Ok(services) => match services.save_settings(&next).await {
                                        Ok(()) => {
                                            theme.settings.set(next);
                                            status.set("设置已保存。".to_string());
                                        }
                                        Err(err) => status.set(format!("保存设置失败：{err}")),
                                    },
                                    Err(err) => status.set(format!("初始化应用失败：{err}")),
                                }
                            });
                        },
                        "保存设置"
                    }
                }
                div { class: "settings-card",
                    h3 { "WebDAV 配置交换" }
                    label { class: "field-label", "Endpoint" }
                    input {
                        class: "text-input",
                        "data-action": "webdav-endpoint",
                        value: "{endpoint}",
                        placeholder: "https://dav.example.com/base/",
                        oninput: move |event| endpoint.set(event.value())
                    }
                    label { class: "field-label", "Remote Path" }
                    input {
                        class: "text-input",
                        "data-action": "webdav-remote-path",
                        value: "{remote_path}",
                        placeholder: "config/rss-reader.json",
                        oninput: move |event| remote_path.set(event.value())
                    }
                    div { class: "inline-actions",
                        button {
                            class: "button secondary",
                            "data-action": "push-webdav",
                            onclick: move |_| {
                                let endpoint = endpoint();
                                let remote_path = remote_path();
                                let mut status = status;
                                spawn(async move {
                                    match AppServices::shared().await {
                                        Ok(services) => match services.push_remote_config(&endpoint, &remote_path).await {
                                            Ok(()) => status.set("配置已上传到 WebDAV。".to_string()),
                                            Err(err) => status.set(format!("上传配置失败：{err}")),
                                        },
                                        Err(err) => status.set(format!("初始化应用失败：{err}")),
                                    }
                                });
                            },
                            "上传配置"
                        }
                        button {
                            class: "button secondary",
                            "data-action": "pull-webdav",
                            onclick: move |_| {
                                let endpoint = endpoint();
                                let remote_path = remote_path();
                                let mut status = status;
                                let mut draft = draft;
                                spawn(async move {
                                    match AppServices::shared().await {
                                        Ok(services) => match services.pull_remote_config(&endpoint, &remote_path).await {
                                            Ok(true) => match services.load_settings().await {
                                                Ok(settings) => {
                                                    draft.set(settings.clone());
                                                    theme.settings.set(settings);
                                                    status.set("已从 WebDAV 下载并导入配置。".to_string());
                                                }
                                                Err(err) => status.set(format!("导入后读取设置失败：{err}")),
                                            },
                                            Ok(false) => status.set("远端配置不存在。".to_string()),
                                            Err(err) => status.set(format!("下载配置失败：{err}")),
                                        },
                                        Err(err) => status.set(format!("初始化应用失败：{err}")),
                                    }
                                });
                            },
                            "下载配置"
                        }
                    }
                }
            }
        }
    }
}

fn theme_value(value: ThemeMode) -> &'static str {
    match value {
        ThemeMode::Light => "light",
        ThemeMode::Dark => "dark",
        ThemeMode::System => "system",
    }
}

fn parse_theme_mode(raw: &str) -> ThemeMode {
    match raw {
        "light" => ThemeMode::Light,
        "dark" => ThemeMode::Dark,
        _ => ThemeMode::System,
    }
}

fn density_value(value: ListDensity) -> &'static str {
    match value {
        ListDensity::Comfortable => "comfortable",
        ListDensity::Compact => "compact",
    }
}

fn parse_list_density(raw: &str) -> ListDensity {
    match raw {
        "compact" => ListDensity::Compact,
        _ => ListDensity::Comfortable,
    }
}

fn startup_value(value: StartupView) -> &'static str {
    match value {
        StartupView::All => "all",
        StartupView::LastFeed => "last_feed",
    }
}

fn parse_startup_view(raw: &str) -> StartupView {
    match raw {
        "last_feed" => StartupView::LastFeed,
        _ => StartupView::All,
    }
}

fn newsprint_theme_css() -> &'static str {
    include_str!("../../../../assets/themes/newsprint.css")
}

fn forest_desk_theme_css() -> &'static str {
    include_str!("../../../../assets/themes/forest-desk.css")
}

fn midnight_ledger_theme_css() -> &'static str {
    include_str!("../../../../assets/themes/midnight-ledger.css")
}

fn preset_css(key: &str) -> &'static str {
    match key {
        "none" => "",
        "forest-desk" => forest_desk_theme_css(),
        "midnight-ledger" => midnight_ledger_theme_css(),
        _ => newsprint_theme_css(),
    }
}

fn preset_display_name(key: &str) -> &'static str {
    match key {
        "forest-desk" => "Forest Desk",
        "midnight-ledger" => "Midnight Ledger",
        _ => "Newsprint",
    }
}

fn detect_preset_key(raw: &str) -> &'static str {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        "none"
    } else if trimmed == forest_desk_theme_css().trim() {
        "forest-desk"
    } else if trimmed == midnight_ledger_theme_css().trim() {
        "midnight-ledger"
    } else {
        "newsprint"
    }
}

fn custom_css_source_label(raw: &str) -> &'static str {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        "未启用自定义 CSS"
    } else if trimmed == newsprint_theme_css().trim() {
        "内置主题：Newsprint"
    } else if trimmed == forest_desk_theme_css().trim() {
        "内置主题：Forest Desk"
    } else if trimmed == midnight_ledger_theme_css().trim() {
        "内置主题：Midnight Ledger"
    } else {
        "自定义主题"
    }
}

async fn pick_css_file_contents() -> anyhow::Result<Option<String>> {
    let file = rfd::AsyncFileDialog::new().add_filter("CSS", &["css"]).pick_file().await;

    let Some(file) = file else {
        return Ok(None);
    };

    let bytes = file.read().await;
    let raw =
        String::from_utf8(bytes).map_err(|err| anyhow::anyhow!("CSS 文件不是有效 UTF-8：{err}"))?;
    Ok(Some(raw))
}

async fn save_css_file(raw: &str) -> anyhow::Result<bool> {
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

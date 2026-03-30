use dioxus::prelude::*;
use rssr_domain::{ListDensity, StartupView, ThemeMode, UserSettings};

use crate::{
    app::AppNav, bootstrap::AppServices, components::status_banner::StatusBanner,
    theme::ThemeController,
};

const REPOSITORY_URL: &str = "https://github.com/Develata/RSS-Reader";

#[component]
pub fn SettingsPage() -> Element {
    let mut theme = use_context::<ThemeController>();
    let mut draft = use_signal(|| (theme.settings)());
    let mut preset_choice =
        use_signal(|| detect_preset_key(&(theme.settings)().custom_css).to_string());
    let mut endpoint = use_signal(String::new);
    let mut remote_path = use_signal(|| "config/rss-reader.json".to_string());
    let status = use_signal(|| "在这里管理主题、阅读偏好和远端配置交换。".to_string());
    let status_tone = use_signal(|| "info".to_string());

    let _ = use_resource(move || async move {
        match AppServices::shared().await {
            Ok(services) => match services.load_settings().await {
                Ok(settings) => {
                    preset_choice.set(detect_preset_key(&settings.custom_css).to_string());
                    draft.set(settings);
                }
                Err(err) => set_status_error(status, status_tone, format!("读取设置失败：{err}")),
            },
            Err(err) => set_status_error(status, status_tone, format!("初始化应用失败：{err}")),
        }
    });

    rsx! {
        section { class: "page page-settings", "data-page": "settings",
            AppNav {}
            div { class: "page-header",
                h2 { "设置" }
                button {
                    class: "icon-link-button",
                    "data-action": "open-github-repo",
                    r#type: "button",
                        aria_label: "打开项目 GitHub 仓库",
                        title: "打开项目 GitHub 仓库",
                        onclick: move |_| {
                            if let Err(err) = open_repository_url() {
                                set_status_error(status, status_tone, format!("打开 GitHub 仓库失败：{err}"));
                            }
                        },
                    svg {
                        xmlns: "http://www.w3.org/2000/svg",
                        view_box: "0 0 24 24",
                        width: "18",
                        height: "18",
                        "aria-hidden": "true",
                        fill: "currentColor",
                        path {
                            d: "M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.009-.866-.014-1.7-2.782.605-3.369-1.344-3.369-1.344-.455-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.071 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.091-.647.349-1.088.635-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.027A9.564 9.564 0 0 1 12 6.844c.85.004 1.705.115 2.504.337 1.909-1.297 2.748-1.027 2.748-1.027.546 1.378.203 2.397.1 2.65.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.31.678.921.678 1.857 0 1.34-.012 2.422-.012 2.75 0 .268.18.58.688.481A10.02 10.02 0 0 0 22 12.017C22 6.484 17.523 2 12 2z"
                        }
                    }
                }
            }
            StatusBanner { message: status(), tone: status_tone() }
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
                                spawn(async move {
                                    match pick_css_file_contents().await {
                                        Ok(Some(raw)) => {
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
                                                "已从文件载入并应用自定义 CSS。".to_string(),
                                            );
                                        }
                                        Ok(None) => set_status_info(status, status_tone, "已取消载入 CSS 文件。".to_string()),
                                        Err(err) => set_status_error(status, status_tone, format!("载入 CSS 文件失败：{err}")),
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
                                spawn(async move {
                                    if raw.trim().is_empty() {
                                        set_status_info(status, status_tone, "当前没有可导出的自定义 CSS。".to_string());
                                        return;
                                    }

                                    match save_css_file(&raw).await {
                                        Ok(true) => set_status_info(status, status_tone, "已导出当前自定义 CSS。".to_string()),
                                        Ok(false) => set_status_info(status, status_tone, "已取消导出 CSS 文件。".to_string()),
                                        Err(err) => set_status_error(status, status_tone, format!("导出 CSS 文件失败：{err}")),
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
                            option { value: "custom", "自定义主题" }
                            option { value: "atlas-sidebar", "Atlas Sidebar" }
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
                                    let applied = next.clone();
                                    draft.set(next);
                                    apply_settings_immediately(
                                        theme,
                                        draft,
                                        preset_choice,
                                        status,
                                        status_tone,
                                        applied,
                                        "已清空自定义 CSS。".to_string(),
                                    );
                                    return;
                                }
                                if choice == "custom" {
                                    set_status_info(
                                        status,
                                        status_tone,
                                        "当前是自定义主题，请直接编辑 CSS 或从文件导入。".to_string(),
                                    );
                                    return;
                                }
                                let mut next = draft();
                                next.custom_css = preset_css(choice.as_str()).to_string();
                                preset_choice.set(choice.clone());
                                let applied = next.clone();
                                draft.set(next);
                                apply_settings_immediately(
                                    theme,
                                    draft,
                                    preset_choice,
                                    status,
                                    status_tone,
                                    applied,
                                    format!("已应用示例主题：{}。", preset_display_name(choice.as_str())),
                                );
                            },
                            "载入所选主题"
                        }
                    }
                    div { class: "theme-gallery", "data-action": "theme-gallery",
                        for preset in builtin_theme_presets() {
                            {
                                let is_active = detect_preset_key(&draft().custom_css) == preset.key;
                                let preset_key = preset.key.to_string();
                                let remove_preset_key = preset_key.clone();
                                let preset_name = preset.name;
                                let preset_description = preset.description;
                                let preset_notes = preset.notes;
                                let preset_swatches = preset.swatches;
                                rsx! {
                                    article {
                                        class: if is_active { "theme-card is-active" } else { "theme-card" },
                                        key: "{preset.key}",
                                        "data-action": "theme-card",
                                        "data-theme-preset": "{preset.key}",
                                        h4 { class: "theme-card__title", "{preset_name}" }
                                        p { class: "theme-card__description", "{preset_description}" }
                                        div { class: "theme-card__swatches",
                                            for swatch in preset_swatches {
                                                span {
                                                    class: "theme-card__swatch",
                                                    style: "background:{swatch}",
                                                }
                                            }
                                        }
                                        p { class: "theme-card__notes", "{preset_notes}" }
                                        button {
                                            class: if is_active { "button" } else { "button secondary" },
                                            "data-action": "apply-theme-card",
                                            onclick: move |_| {
                                                let mut next = draft();
                                                next.custom_css = preset_css(preset_key.as_str()).to_string();
                                                preset_choice.set(preset_key.clone());
                                                let applied = next.clone();
                                                draft.set(next);
                                                apply_settings_immediately(
                                                    theme,
                                                    draft,
                                                    preset_choice,
                                                    status,
                                                    status_tone,
                                                    applied,
                                                    format!("已从主题卡片应用：{}。", preset_name),
                                                );
                                            },
                                            if is_active { "当前已选" } else { "使用这套主题" }
                                        }
                                        button {
                                            class: "button secondary danger-outline",
                                            "data-action": "remove-theme-card",
                                            onclick: move |_| {
                                                if detect_preset_key(&draft().custom_css) != remove_preset_key.as_str() {
                                                    set_status_info(status, status_tone, format!("当前并未启用主题：{}。", preset_name));
                                                    return;
                                                }
                                                let mut next = draft();
                                                next.custom_css.clear();
                                                preset_choice.set("none".to_string());
                                                let applied = next.clone();
                                                draft.set(next);
                                                apply_settings_immediately(
                                                    theme,
                                                    draft,
                                                    preset_choice,
                                                    status,
                                                    status_tone,
                                                    applied,
                                                    format!("已移除主题：{}。", preset_name),
                                                );
                                            },
                                            "移除这套主题"
                                        }
                                    }
                                }
                            }
                        }
                    }
                    p { class: "page-intro", "可直接载入内置示例主题，或清空当前自定义 CSS。预置主题会立即生效并自动保存；手动编辑 CSS 后再点击“保存设置”。" }
                    div { class: "preset-grid",
                        button {
                            class: "button secondary",
                            "data-action": "apply-theme-atlas-sidebar",
                            onclick: move |_| {
                                let mut next = draft();
                                next.custom_css = atlas_sidebar_theme_css().to_string();
                                preset_choice.set("atlas-sidebar".to_string());
                                let applied = next.clone();
                                draft.set(next);
                                apply_settings_immediately(
                                    theme,
                                    draft,
                                    preset_choice,
                                    status,
                                    status_tone,
                                    applied,
                                    "已应用示例主题：Atlas Sidebar。".to_string(),
                                );
                            },
                            "Atlas Sidebar"
                        }
                        button {
                            class: "button secondary",
                            "data-action": "apply-theme-newsprint",
                            onclick: move |_| {
                                let mut next = draft();
                                next.custom_css = newsprint_theme_css().to_string();
                                preset_choice.set("newsprint".to_string());
                                let applied = next.clone();
                                draft.set(next);
                                apply_settings_immediately(
                                    theme,
                                    draft,
                                    preset_choice,
                                    status,
                                    status_tone,
                                    applied,
                                    "已应用示例主题：Newsprint。".to_string(),
                                );
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
                                let applied = next.clone();
                                draft.set(next);
                                apply_settings_immediately(
                                    theme,
                                    draft,
                                    preset_choice,
                                    status,
                                    status_tone,
                                    applied,
                                    "已应用示例主题：Forest Desk。".to_string(),
                                );
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
                                let applied = next.clone();
                                draft.set(next);
                                apply_settings_immediately(
                                    theme,
                                    draft,
                                    preset_choice,
                                    status,
                                    status_tone,
                                    applied,
                                    "已应用示例主题：Midnight Ledger。".to_string(),
                                );
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
                                let applied = next.clone();
                                draft.set(next);
                                apply_settings_immediately(
                                    theme,
                                    draft,
                                    preset_choice,
                                    status,
                                    status_tone,
                                    applied,
                                    "已清空自定义 CSS。".to_string(),
                                );
                            },
                            "清空 CSS"
                        }
                    }
                    button {
                        class: "button",
                            "data-action": "save-settings",
                            onclick: move |_| {
                                let next = draft();
                                spawn(async move {
                                    match AppServices::shared().await {
                                    Ok(services) => match services.save_settings(&next).await {
                                        Ok(()) => {
                                            theme.settings.set(next);
                                            set_status_info(status, status_tone, "设置已保存。".to_string());
                                        }
                                        Err(err) => set_status_error(status, status_tone, format!("保存设置失败：{err}")),
                                    },
                                    Err(err) => set_status_error(status, status_tone, format!("初始化应用失败：{err}")),
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
                                spawn(async move {
                                    match AppServices::shared().await {
                                        Ok(services) => match services.push_remote_config(&endpoint, &remote_path).await {
                                            Ok(()) => set_status_info(status, status_tone, "配置已上传到 WebDAV。".to_string()),
                                            Err(err) => set_status_error(status, status_tone, format!("上传配置失败：{err}")),
                                        },
                                        Err(err) => set_status_error(status, status_tone, format!("初始化应用失败：{err}")),
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
                                let mut draft = draft;
                                spawn(async move {
                                    match AppServices::shared().await {
                                        Ok(services) => match services.pull_remote_config(&endpoint, &remote_path).await {
                                            Ok(true) => match services.load_settings().await {
                                                Ok(settings) => {
                                                    preset_choice.set(detect_preset_key(&settings.custom_css).to_string());
                                                    draft.set(settings.clone());
                                                    theme.settings.set(settings);
                                                    set_status_info(status, status_tone, "已从 WebDAV 下载并导入配置。".to_string());
                                                }
                                                Err(err) => set_status_error(status, status_tone, format!("导入后读取设置失败：{err}")),
                                            },
                                            Ok(false) => set_status_info(status, status_tone, "远端配置不存在。".to_string()),
                                            Err(err) => set_status_error(status, status_tone, format!("下载配置失败：{err}")),
                                        },
                                        Err(err) => set_status_error(status, status_tone, format!("初始化应用失败：{err}")),
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

#[cfg(not(target_arch = "wasm32"))]
fn open_repository_url() -> Result<(), String> {
    webbrowser::open(REPOSITORY_URL).map(|_| ()).map_err(|err| err.to_string())
}

#[cfg(target_arch = "wasm32")]
fn open_repository_url() -> Result<(), String> {
    web_sys::window()
        .ok_or_else(|| "浏览器窗口不可用".to_string())?
        .open_with_url_and_target(REPOSITORY_URL, "_blank")
        .map(|_| ())
        .map_err(|err| format!("{err:?}"))
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

fn atlas_sidebar_theme_css() -> &'static str {
    include_str!("../../../../assets/themes/atlas-sidebar.css")
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
        "atlas-sidebar" => atlas_sidebar_theme_css(),
        "newsprint" => newsprint_theme_css(),
        "forest-desk" => forest_desk_theme_css(),
        "midnight-ledger" => midnight_ledger_theme_css(),
        _ => "",
    }
}

fn preset_display_name(key: &str) -> &'static str {
    match key {
        "atlas-sidebar" => "Atlas Sidebar",
        "newsprint" => "Newsprint",
        "forest-desk" => "Forest Desk",
        "midnight-ledger" => "Midnight Ledger",
        _ => "自定义主题",
    }
}

fn detect_preset_key(raw: &str) -> &'static str {
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

fn custom_css_source_label(raw: &str) -> &'static str {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        "未启用自定义 CSS"
    } else if trimmed == atlas_sidebar_theme_css().trim() {
        "内置主题：Atlas Sidebar"
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

fn apply_settings_immediately(
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

fn set_status_info(mut status: Signal<String>, mut status_tone: Signal<String>, message: String) {
    status.set(message);
    status_tone.set("info".to_string());
}

fn set_status_error(mut status: Signal<String>, mut status_tone: Signal<String>, message: String) {
    status.set(message);
    status_tone.set("error".to_string());
}

#[derive(Clone, Copy)]
struct BuiltinThemePreset {
    key: &'static str,
    name: &'static str,
    description: &'static str,
    notes: &'static str,
    swatches: [&'static str; 3],
}

fn builtin_theme_presets() -> [BuiltinThemePreset; 4] {
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
            name: "Forest Desk",
            description: "轻盈的绿色工作台风格，界面更清爽、留白更多。",
            notes: "按钮更圆，设置页更像左右工作区，适合白天阅读。",
            swatches: ["#e6efe8", "#236846", "#1a2a21"],
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
async fn save_css_file(raw: &str) -> anyhow::Result<bool> {
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

    Ok(true)
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

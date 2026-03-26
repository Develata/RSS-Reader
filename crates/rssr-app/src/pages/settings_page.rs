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
    let mut endpoint = use_signal(String::new);
    let mut remote_path = use_signal(|| "config/rss-reader.json".to_string());
    let mut status = use_signal(|| "在这里管理主题、阅读偏好和远端配置交换。".to_string());

    let _ = use_resource(move || async move {
        match AppServices::shared().await {
            Ok(services) => match services.load_settings().await {
                Ok(settings) => draft.set(settings),
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
                            draft.set(next);
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

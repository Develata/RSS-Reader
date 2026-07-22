fn newsprint_theme_css() -> &'static str {
    include_str!("../../../../../../assets/themes/newsprint.css")
}

fn atlas_sidebar_theme_css() -> &'static str {
    include_str!("../../../../../../assets/themes/atlas-sidebar.css")
}

fn amethyst_glass_theme_css() -> &'static str {
    include_str!("../../../../../../assets/themes/amethyst-glass.css")
}

fn midnight_ledger_theme_css() -> &'static str {
    include_str!("../../../../../../assets/themes/midnight-ledger.css")
}

/// 历史版本的内置主题 CSS。用户设置里只持久化 CSS 文本本身，
/// 主题被改版后旧文本必须仍能识别回对应预设，否则会退化显示为「自定义主题」，
/// 且「移除这套主题」不再命中。新改版一次就在这里追加一条冻结副本。
const LEGACY_PRESET_CSS: &[(&str, &str)] = &[
    ("atlas-sidebar", include_str!("../../../../../../assets/themes/legacy/atlas-sidebar-v1.css")),
    ("newsprint", include_str!("../../../../../../assets/themes/legacy/newsprint-v1.css")),
    ("amethyst-glass", include_str!("../../../../../../assets/themes/legacy/forest-desk-v1.css")),
];

pub(crate) fn preset_css(key: &str) -> &'static str {
    match key {
        "none" => "",
        "atlas-sidebar" => atlas_sidebar_theme_css(),
        "newsprint" => newsprint_theme_css(),
        "amethyst-glass" => amethyst_glass_theme_css(),
        "midnight-ledger" => midnight_ledger_theme_css(),
        _ => "",
    }
}

pub(crate) fn preset_display_name(key: &str) -> &'static str {
    match key {
        "atlas-sidebar" => "Atlas Sidebar",
        "newsprint" => "Newsprint",
        "amethyst-glass" => "Amethyst Glass",
        "midnight-ledger" => "Midnight Ledger",
        _ => "自定义主题",
    }
}

/// 主题身份比较：忽略回车符。主题文本会经由不同平台的检出（CRLF/LF）
/// 嵌入二进制或持久化到配置，行尾差异不构成不同主题。零分配，UTF-8
/// 字节级等价即字符串等价。
fn css_text_eq(left: &str, right: &str) -> bool {
    left.bytes().filter(|&byte| byte != b'\r').eq(right.bytes().filter(|&byte| byte != b'\r'))
}

pub(crate) fn detect_preset_key(raw: &str) -> &'static str {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "none";
    }
    for key in ["atlas-sidebar", "newsprint", "amethyst-glass", "midnight-ledger"] {
        if css_text_eq(trimmed, preset_css(key).trim()) {
            return key;
        }
    }
    for (key, css) in LEGACY_PRESET_CSS {
        if css_text_eq(trimmed, css.trim()) {
            return key;
        }
    }
    "custom"
}

#[derive(Clone, Copy)]
pub(super) struct BuiltinThemePreset {
    pub key: &'static str,
    pub name: &'static str,
    pub swatches: [&'static str; 3],
}

pub(super) fn builtin_theme_presets() -> [BuiltinThemePreset; 4] {
    [
        BuiltinThemePreset {
            key: "atlas-sidebar",
            name: "Atlas Sidebar",
            swatches: ["#f1efe8", "#b24c3d", "#1f2430"],
        },
        BuiltinThemePreset {
            key: "newsprint",
            name: "Newsprint",
            swatches: ["#efe6d6", "#8b3d2b", "#241d16"],
        },
        BuiltinThemePreset {
            key: "amethyst-glass",
            name: "Amethyst Glass",
            swatches: ["#e0c3fc", "#8b5cf6", "#1f2937"],
        },
        BuiltinThemePreset {
            key: "midnight-ledger",
            name: "Midnight Ledger",
            swatches: ["#0f1518", "#4fb7b1", "#eef5f7"],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::{
        LEGACY_PRESET_CSS, amethyst_glass_theme_css, detect_preset_key, newsprint_theme_css,
    };

    #[test]
    fn detect_preset_key_keeps_unknown_css_as_custom() {
        let custom = ":root { --panel: #123456; }\n[data-page=\"reader\"] { gap: 99px; }";
        assert_eq!(detect_preset_key(custom), "custom");
    }

    #[test]
    fn detect_preset_key_matches_builtin_presets_exactly() {
        assert_eq!(detect_preset_key(""), "none");
        assert_eq!(detect_preset_key(newsprint_theme_css()), "newsprint");
        assert_eq!(detect_preset_key(amethyst_glass_theme_css()), "amethyst-glass");
    }

    #[test]
    fn detect_preset_key_maps_legacy_css_to_current_keys() {
        for (expected_key, legacy_css) in LEGACY_PRESET_CSS {
            assert_eq!(detect_preset_key(legacy_css), *expected_key);
        }
    }

    #[test]
    fn detect_preset_key_ignores_line_ending_differences() {
        let current_crlf = newsprint_theme_css().replace('\n', "\r\n");
        assert_eq!(detect_preset_key(&current_crlf), "newsprint");

        for (expected_key, legacy_css) in LEGACY_PRESET_CSS {
            let legacy_crlf = legacy_css.replace('\n', "\r\n");
            assert_eq!(detect_preset_key(&legacy_crlf), *expected_key);
        }
    }

    #[test]
    fn legacy_css_differs_from_current_presets() {
        use super::{css_text_eq, preset_css};
        for (key, legacy_css) in LEGACY_PRESET_CSS {
            assert!(
                !css_text_eq(legacy_css.trim(), preset_css(key).trim()),
                "legacy copy of {key} is identical to current css; freeze is redundant"
            );
        }
    }
}

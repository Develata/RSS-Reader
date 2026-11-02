fn amethyst_glass_theme_css() -> &'static str {
    include_str!("../../../../../../assets/themes/amethyst-glass.css")
}

/// 历史版本的内置主题 CSS。用户设置里只持久化 CSS 文本本身，
/// 主题被改版后旧文本必须仍能识别回对应预设，否则会退化显示为「自定义主题」，
/// 且当前主题的识别与已选状态不再匹配。新改版一次就在这里追加一条冻结副本。
const LEGACY_PRESET_CSS: &[(&str, &str)] = &[
    (
        "amethyst-glass",
        include_str!("../../../../../../assets/themes/legacy/amethyst-glass-v4.css"),
    ),
    (
        "amethyst-glass",
        include_str!("../../../../../../assets/themes/legacy/amethyst-glass-v3.css"),
    ),
    (
        "amethyst-glass",
        include_str!("../../../../../../assets/themes/legacy/amethyst-glass-v2.css"),
    ),
    (
        "amethyst-glass",
        include_str!("../../../../../../assets/themes/legacy/amethyst-glass-v1.css"),
    ),
    ("amethyst-glass", include_str!("../../../../../../assets/themes/legacy/forest-desk-v1.css")),
];

/// 已下线的内置主题（Atlas Sidebar、Newsprint、Midnight Ledger 的各版本）。
/// 存下的 CSS 仍原样生效，只识别为 `RETIRED_PRESET_KEY`，让设置页提示换回现有主题；
/// Midnight Ledger 的配色已并入默认主题的深色外观。
const RETIRED_PRESET_CSS: &[&str] = &[
    include_str!("../../../../../../assets/themes/legacy/atlas-sidebar-v4.css"),
    include_str!("../../../../../../assets/themes/legacy/atlas-sidebar-v3.css"),
    include_str!("../../../../../../assets/themes/legacy/atlas-sidebar-v2.css"),
    include_str!("../../../../../../assets/themes/legacy/atlas-sidebar-v1.css"),
    include_str!("../../../../../../assets/themes/legacy/newsprint-v2.css"),
    include_str!("../../../../../../assets/themes/legacy/newsprint-v1.css"),
    include_str!("../../../../../../assets/themes/legacy/midnight-ledger-v3.css"),
    include_str!("../../../../../../assets/themes/legacy/midnight-ledger-v2.css"),
    include_str!("../../../../../../assets/themes/legacy/midnight-ledger-v1.css"),
];

pub(crate) const RETIRED_PRESET_KEY: &str = "retired";

pub(crate) fn preset_css(key: &str) -> &'static str {
    match key {
        "amethyst-glass" => amethyst_glass_theme_css(),
        _ => "",
    }
}

pub(crate) fn preset_display_name(key: &str) -> &'static str {
    match key {
        "amethyst-glass" => "Amethyst Glass",
        RETIRED_PRESET_KEY => "已下线的内置主题",
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
    if css_text_eq(trimmed, preset_css("amethyst-glass").trim()) {
        return "amethyst-glass";
    }
    for (key, css) in LEGACY_PRESET_CSS {
        if css_text_eq(trimmed, css.trim()) {
            return key;
        }
    }
    if RETIRED_PRESET_CSS.iter().any(|css| css_text_eq(trimmed, css.trim())) {
        return RETIRED_PRESET_KEY;
    }
    "custom"
}

#[derive(Clone, Copy)]
pub(super) struct BuiltinThemePreset {
    pub key: &'static str,
    pub name: &'static str,
    pub swatches: [&'static str; 3],
}

pub(super) fn builtin_theme_presets() -> [BuiltinThemePreset; 1] {
    [BuiltinThemePreset {
        key: "amethyst-glass",
        name: "Amethyst Glass",
        swatches: ["#e0c3fc", "#8b5cf6", "#241b45"],
    }]
}

#[cfg(test)]
mod tests {
    use super::{
        LEGACY_PRESET_CSS, RETIRED_PRESET_CSS, RETIRED_PRESET_KEY, amethyst_glass_theme_css,
        detect_preset_key,
    };

    #[test]
    fn detect_preset_key_keeps_unknown_css_as_custom() {
        let custom = ":root { --panel: #123456; }\n[data-page=\"reader\"] { gap: 99px; }";
        assert_eq!(detect_preset_key(custom), "custom");
    }

    #[test]
    fn detect_preset_key_matches_builtin_presets_exactly() {
        assert_eq!(detect_preset_key(""), "none");
        assert_eq!(detect_preset_key(amethyst_glass_theme_css()), "amethyst-glass");
    }

    #[test]
    fn detect_preset_key_maps_legacy_css_to_current_keys() {
        for (expected_key, legacy_css) in LEGACY_PRESET_CSS {
            assert_eq!(detect_preset_key(legacy_css), *expected_key);
        }
    }

    #[test]
    fn detect_preset_key_recognizes_retired_presets() {
        for retired_css in RETIRED_PRESET_CSS {
            assert_eq!(detect_preset_key(retired_css), RETIRED_PRESET_KEY);
            assert_eq!(detect_preset_key(&retired_css.replace('\n', "\r\n")), RETIRED_PRESET_KEY);
        }
    }

    #[test]
    fn detect_preset_key_ignores_line_ending_differences() {
        let current_crlf = amethyst_glass_theme_css().replace('\n', "\r\n");
        assert_eq!(detect_preset_key(&current_crlf), "amethyst-glass");

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

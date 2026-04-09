fn newsprint_theme_css() -> &'static str {
    include_str!("../../../../../../assets/themes/newsprint.css")
}

fn atlas_sidebar_theme_css() -> &'static str {
    include_str!("../../../../../../assets/themes/atlas-sidebar.css")
}

fn forest_desk_theme_css() -> &'static str {
    include_str!("../../../../../../assets/themes/forest-desk.css")
}

fn midnight_ledger_theme_css() -> &'static str {
    include_str!("../../../../../../assets/themes/midnight-ledger.css")
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
            key: "forest-desk",
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
    use super::{detect_preset_key, forest_desk_theme_css, newsprint_theme_css};

    #[test]
    fn detect_preset_key_keeps_unknown_css_as_custom() {
        let custom = ":root { --panel: #123456; }\n[data-page=\"reader\"] { gap: 99px; }";
        assert_eq!(detect_preset_key(custom), "custom");
    }

    #[test]
    fn detect_preset_key_matches_builtin_presets_exactly() {
        assert_eq!(detect_preset_key(""), "none");
        assert_eq!(detect_preset_key(newsprint_theme_css()), "newsprint");
        assert_eq!(detect_preset_key(forest_desk_theme_css()), "forest-desk");
    }
}

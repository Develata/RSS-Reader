//! 默认样式 token 契约：
//! 1. `tokens.css` 之外的默认样式表不得出现硬编码颜色（hex / rgb / rgba / hsl），
//!    只能消费 token —— 这是「覆写变量即可整体换肤」承诺的回归门。
//! 2. `tokens.css` 必须持续暴露文档化的公开 token
//!    （见 docs/design/theme-author-selector-reference.md「可用 CSS 变量」）。
//! 3. 表面背景 token（`BACKGROUND_TOKENS`，可以是渐变）只能作为背景层，不能放进渐变当色标；
//!    `--surface-tint` 用作色标，必须是颜色。

const TOKENS: &str = include_str!("../../../assets/styles/tokens.css");

const TOKEN_CONSUMING_SHEETS: &[(&str, &str)] = &[
    ("shell.css", include_str!("../../../assets/styles/shell.css")),
    ("workspaces.css", include_str!("../../../assets/styles/workspaces.css")),
    ("entries.css", include_str!("../../../assets/styles/entries.css")),
    ("reader.css", include_str!("../../../assets/styles/reader.css")),
    ("responsive.css", include_str!("../../../assets/styles/responsive.css")),
    ("reading-workspace.css", include_str!("../../../assets/styles/reading-workspace.css")),
];

const PUBLIC_TOKENS: &[&str] = &[
    // 第 1 层：基础调色板
    "--bg:",
    "--panel:",
    "--panel-strong:",
    "--ink:",
    "--muted:",
    "--line:",
    "--accent:",
    "--accent-strong:",
    "--shadow:",
    "--font-display:",
    "--font-ui:",
    // 第 2 层：语义 token（文档化子集）
    "--app-bg:",
    "--app-bg-dark:",
    "--line-soft:",
    "--surface-panel:",
    "--surface-reader:",
    "--surface-card:",
    "--surface-veil:",
    "--surface-chip:",
    "--surface-chip-hover:",
    "--surface-tint:",
    "--input-bg:",
    "--accent-soft:",
    "--accent-line:",
    "--accent-line-faint:",
    "--focus-ring-color:",
    "--focus-ring-shadow:",
    "--danger:",
    "--status-info-bg:",
    "--status-error-fg:",
    "--status-success-fg:",
    "--reader-ink:",
    "--reader-measure:",
    "--image-viewer-bg:",
    "--image-viewer-backdrop:",
    "--image-viewer-control-bg:",
    "--image-viewer-ink:",
    "--shadow-soft:",
    "--shadow-lift:",
    "--shadow-card:",
    "--shadow-float:",
    "--inset-highlight:",
    "--radius-panel:",
    "--radius-card:",
    "--radius-card-sm:",
    "--radius-control:",
    "--radius-pill:",
    "--button-bg:",
    "--button-fg:",
    "--button-shadow:",
    "--button-radius:",
    "--button-min-height:",
    "--button-padding:",
    "--button-secondary-bg:",
    "--button-secondary-fg:",
    "--button-danger-bg:",
    "--button-danger-outline-bg:",
    "--shell-max-width:",
    "--rail-width:",
    "--transition-quick:",
];

fn hardcoded_color_violations(name: &str, css: &str) -> Vec<String> {
    let mut violations = Vec::new();
    for (index, line) in css.lines().enumerate() {
        let lower = line.to_ascii_lowercase();
        let has_color_function = ["rgb(", "rgba(", "hsl(", "hsla("].iter().any(|prefix| {
            lower.match_indices(prefix).any(|(start, _)| {
                // Relative colors may derive an opaque surface from a theme token.
                !lower[start + prefix.len()..].trim_start().starts_with("from var(")
            })
        });
        let has_hex_color = line.char_indices().any(|(byte_index, character)| {
            character == '#'
                && line[byte_index + 1..]
                    .chars()
                    .next()
                    .is_some_and(|next| next.is_ascii_hexdigit())
        });
        if has_color_function || has_hex_color {
            violations.push(format!("{name}:{}: {}", index + 1, line.trim()));
        }
    }
    violations
}

#[test]
fn relative_token_colors_are_not_literal_palette_values() {
    assert!(
        hardcoded_color_violations("test", "background: rgb(from var(--panel-strong) r g b / 1);")
            .is_empty()
    );
    for literal in ["rgb(255 255 255)", "rgb(from white r g b)", "#ffffff"] {
        assert_eq!(hardcoded_color_violations("test", literal).len(), 1);
    }
}

#[test]
fn default_styles_outside_tokens_do_not_hardcode_colors() {
    let violations: Vec<String> = TOKEN_CONSUMING_SHEETS
        .iter()
        .flat_map(|(name, css)| hardcoded_color_violations(name, css))
        .collect();
    assert!(
        violations.is_empty(),
        "默认样式出现硬编码颜色（应改为消费 tokens.css 中的变量）：\n{}",
        violations.join("\n")
    );
}

#[test]
fn tokens_stylesheet_exposes_documented_public_tokens() {
    let missing: Vec<&str> =
        PUBLIC_TOKENS.iter().copied().filter(|token| !TOKENS.contains(token)).collect();
    assert!(
        missing.is_empty(),
        "tokens.css 缺少文档化公开 token（若确需改名，先同步 docs/design/theme-author-selector-reference.md）：\n{}",
        missing.join("\n")
    );
}

/// Surface tokens a theme may set to an image (a gradient), not only a color.
const BACKGROUND_TOKENS: &[&str] = &[
    "var(--surface-panel)",
    "var(--surface-reader)",
    "var(--surface-card)",
    "var(--surface-veil)",
    "var(--surface-chip)",
    "var(--surface-chip-hover)",
];

/// Each background token inside a gradient's argument list (`name:line`).
fn surface_tokens_used_as_gradient_stops(name: &str, css: &str) -> Vec<String> {
    let mut violations = Vec::new();
    for (start, _) in css.match_indices("gradient(") {
        let args_start = start + "gradient(".len();
        let mut depth = 1;
        let args_end = css[args_start..]
            .char_indices()
            .find_map(|(offset, character)| {
                match character {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                (depth == 0).then_some(args_start + offset)
            })
            .unwrap_or(css.len());
        let args = &css[args_start..args_end];
        if BACKGROUND_TOKENS.iter().any(|token| args.contains(token)) {
            let line = css[..start].lines().count();
            violations.push(format!("{name}:{line}"));
        }
    }
    violations
}

#[test]
fn surface_token_guard_detects_gradient_stops() {
    let nested = "background: linear-gradient(180deg, var(--surface-card), var(--surface-tint));";
    assert_eq!(surface_tokens_used_as_gradient_stops("test", nested).len(), 1);
    let layered =
        "background: linear-gradient(180deg, transparent, var(--accent)), var(--surface-card);";
    assert!(surface_tokens_used_as_gradient_stops("test", layered).is_empty());
}

#[test]
fn default_styles_layer_surface_tokens_instead_of_interpolating_them() {
    let violations: Vec<String> = TOKEN_CONSUMING_SHEETS
        .iter()
        .flat_map(|(name, css)| surface_tokens_used_as_gradient_stops(name, css))
        .collect();
    assert!(
        violations.is_empty(),
        "表面背景 token 可能是渐变，放进渐变当色标会使整条 background 失效：\n{}",
        violations.join("\n")
    );
}

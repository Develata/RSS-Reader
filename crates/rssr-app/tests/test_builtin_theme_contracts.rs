use std::collections::BTreeSet;

const DEFAULT_TOKENS: &str = include_str!("../../../assets/styles/tokens.css");
const AMETHYST_GLASS: &str = include_str!("../../../assets/themes/amethyst-glass.css");

const THEMES: &[(&str, &str)] = &[("amethyst-glass.css", AMETHYST_GLASS)];

const BANNED_SELECTORS: &[&str] = &[
    ".app-nav",
    ".reader-page",
    ".reader-header",
    ".reader-toolbar",
    ".entry-filters",
    ".web-auth-",
    ".button.secondary",
    ".button.danger",
    ".button.danger-outline",
    ".theme-card.is-active",
    ".entry-filters__source-chip.is-selected",
    ".reader-bottom-bar__button.is-",
    "feed-card__title",
    "feed-card__meta",
    "entry-card__title",
    "entry-card__meta",
    "theme-card__description",
    "theme-card__notes",
];

const REQUIRED_PATTERNS: &[&str] = &[
    "data-layout=\"app-nav-shell\"",
    "data-nav",
    "data-variant=\"secondary\"",
    "data-layout=\"reader-body\"",
];

#[test]
fn builtin_themes_do_not_reintroduce_legacy_selector_contracts() {
    for (name, css) in THEMES {
        for banned in BANNED_SELECTORS {
            assert!(
                !css.contains(banned),
                "{name} reintroduced banned selector contract: {banned}"
            );
        }
    }
}

#[test]
fn builtin_themes_reference_current_semantic_interfaces() {
    for (name, css) in THEMES {
        for required in REQUIRED_PATTERNS {
            assert!(
                css.contains(required),
                "{name} is missing required semantic contract: {required}"
            );
        }
    }
}

/// Custom properties declared in the first rule whose prelude starts at `prelude`.
/// Token rules hold only declarations, so the rule ends at the first `}`.
fn declared_tokens(css: &str, prelude: &str) -> BTreeSet<String> {
    let start = css.find(prelude).unwrap_or_else(|| panic!("missing rule `{prelude}`"));
    let body_start = start + css[start..].find('{').expect("rule body") + 1;
    let body_end = body_start + css[body_start..].find('}').expect("rule end");
    css[body_start..body_end]
        .lines()
        .filter_map(|line| line.trim().strip_prefix("--"))
        .filter_map(|line| line.split_once(':'))
        .map(|(name, _)| format!("--{name}"))
        .collect()
}

/// The tokens the default dark appearance flips. Every complete scheme must declare all of them,
/// or it silently mixes in the default palette.
fn mode_tokens() -> BTreeSet<String> {
    declared_tokens(DEFAULT_TOKENS, "\n.theme-dark {")
}

fn assert_covers(name: &str, scheme: &str, declared: &BTreeSet<String>) {
    let missing: Vec<_> = mode_tokens().difference(declared).cloned().collect();
    assert!(missing.is_empty(), "{name} {scheme} scheme falls back to defaults for {missing:?}");
}

#[test]
fn default_light_and_dark_schemes_flip_the_same_tokens() {
    assert_covers("tokens.css", "light", &declared_tokens(DEFAULT_TOKENS, "\n:root {"));
    assert_eq!(
        declared_tokens(DEFAULT_TOKENS, "@media (prefers-color-scheme: dark) {\n  .theme-system"),
        mode_tokens(),
        "tokens.css follow-system dark must match .theme-dark"
    );
}

#[test]
fn builtin_themes_are_complete_in_both_schemes() {
    for (name, css) in THEMES {
        assert_covers(name, "light", &declared_tokens(css, "\n:root {"));
        let dark = declared_tokens(css, "\n.theme-dark {");
        assert_covers(name, "dark", &dark);
        assert_eq!(
            declared_tokens(css, "@media (prefers-color-scheme: dark) {\n  .theme-system"),
            dark,
            "{name} follow-system dark must match .theme-dark"
        );
    }
}

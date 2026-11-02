use std::collections::{BTreeMap, BTreeSet};

#[allow(dead_code)]
mod css_scan;

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

/// Custom properties (name -> whitespace-normalized value) declared in the first rule whose
/// prelude starts at `prelude`. Token rules hold only declarations, so the rule ends at the
/// first top-level `}`. A declaration naming something like a custom property that is not one
/// identifier (`--dan/**/ger`, `-- x`) fails instead of being read as a token.
fn declarations(css: &str, prelude: &str) -> BTreeMap<String, String> {
    let css = css_scan::without_comments(css);
    let start = css.find(prelude).unwrap_or_else(|| panic!("missing rule `{prelude}`"));
    // The rule's own brace follows the prelude (a `@media {` inside the prelude is not it).
    let header_end = start + prelude.trim_end_matches('{').len();
    let body_start = header_end + css[header_end..].find('{').expect("rule body") + 1;
    let body_end = css_scan::rule_end(&css, body_start).expect("rule end");
    css_scan::top_level_declarations(&css[body_start..body_end])
        .into_iter()
        .filter_map(|declaration| {
            let (name, value) = declaration.split_once(':')?;
            let name = name.trim();
            if css_scan::is_custom_property(name) {
                Some((name.to_string(), value.split_whitespace().collect::<Vec<_>>().join(" ")))
            } else {
                assert!(!name.contains("--"), "unreadable custom property name `{name}`");
                None
            }
        })
        .collect()
}

fn declared_tokens(css: &str, prelude: &str) -> BTreeSet<String> {
    declarations(css, prelude).into_keys().collect()
}

const FOLLOW_SYSTEM_DARK: &str = "@media (prefers-color-scheme: dark) {\n  .theme-system";

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
fn token_parser_ignores_commented_declarations_and_reads_values() {
    let css = "\n.theme-dark {\n  /*\n  --danger: red;\n  */\n  --ink:\n    #fff;\n  color-scheme: dark;\n}";
    let parsed = declarations(css, "\n.theme-dark {");
    assert_eq!(parsed.len(), 1, "{parsed:?}");
    assert_eq!(parsed["--ink"], "#fff");
}

#[test]
fn token_parser_keeps_semicolons_inside_strings_and_parentheses() {
    let css = "\n:root {\n  --a: url(\"data:image/svg+xml;one\");\n  --b: url(data:x;two);\n  --c: \"}\";\n}";
    let parsed = declarations(css, "\n:root {");
    assert_eq!(parsed["--a"], "url(\"data:image/svg+xml;one\")");
    assert_eq!(parsed["--b"], "url(data:x;two)");
    assert_eq!(parsed["--c"], "\"}\"");
}

#[test]
#[should_panic(expected = "unreadable custom property name")]
fn token_parser_rejects_a_name_split_by_a_comment() {
    declarations("\n.theme-dark {\n  --dan/**/ger: red;\n}", "\n.theme-dark {");
}

#[test]
fn default_light_and_dark_schemes_flip_the_same_tokens() {
    assert_covers("tokens.css", "light", &declared_tokens(DEFAULT_TOKENS, "\n:root {"));
    assert_eq!(
        declarations(DEFAULT_TOKENS, FOLLOW_SYSTEM_DARK),
        declarations(DEFAULT_TOKENS, "\n.theme-dark {"),
        "tokens.css follow-system dark must declare exactly the .theme-dark values"
    );
}

#[test]
fn builtin_themes_are_complete_in_both_schemes() {
    for (name, css) in THEMES {
        assert_covers(name, "light", &declared_tokens(css, "\n:root {"));
        let dark = declarations(css, "\n.theme-dark {");
        assert_covers(name, "dark", &dark.keys().cloned().collect());
        assert_eq!(
            declarations(css, FOLLOW_SYSTEM_DARK),
            dark,
            "{name} follow-system dark must declare exactly the .theme-dark values"
        );
    }
}

const ATLAS_SIDEBAR: &str = include_str!("../../../assets/themes/atlas-sidebar.css");
const NEWSPRINT: &str = include_str!("../../../assets/themes/newsprint.css");
const FOREST_DESK: &str = include_str!("../../../assets/themes/forest-desk.css");
const MIDNIGHT_LEDGER: &str = include_str!("../../../assets/themes/midnight-ledger.css");

const THEMES: &[(&str, &str)] = &[
    ("atlas-sidebar.css", ATLAS_SIDEBAR),
    ("newsprint.css", NEWSPRINT),
    ("forest-desk.css", FOREST_DESK),
    ("midnight-ledger.css", MIDNIGHT_LEDGER),
];

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

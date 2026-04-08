# 2026-04-08 | Entries CSS surface thinning

## Summary
- Removed several purely visual modifier classes from entries controls markup.
- Moved those presentation differences back into CSS base selectors and attribute selectors.
- Replaced directory expansion visual state class with `aria-expanded`.
- Removed the entries-only reading-header modifier class.
- Replaced inline title and empty-state branching in RSX with small structural helpers.
- Wrapped entries empty states in a stable `entries-page__state` container.
- Renamed the back-link wrapper to a page-semantic class.

## Why
- The entries page should keep converging toward stable structure and semantics in RSX, with presentation handled by CSS.
- Several classes existed only to tweak visual treatment, not to represent behavior or durable structure.
- Cleaning those boundaries now makes later desktop/mobile CSS iteration cheaper without changing product behavior.

## Impact
- Entries page only.
- No intended behavior change.
- Cleaner RSX surface for future UI work.

## Files
- crates/rssr-app/src/pages/entries_page.rs
- crates/rssr-app/src/pages/entries_page_controls.rs
- assets/styles/entries.css
- assets/styles/responsive.css

## Verification
- cargo fmt --all
- cargo check -p rssr-app
- cargo check -p rssr-app --target wasm32-unknown-unknown
- git diff --check

## Notes For Next Agent
- The next worthwhile CSS thinning target is `entries_page_cards.rs`, especially action button grouping and metadata blocks.
- No additional bootstrap work is needed before continuing entries-page UI cleanup.

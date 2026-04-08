# 2026-04-08 | Entries headless command surface step 1

## Summary
- Introduced an entries-page command surface for entry card actions.
- Moved read/star mutations out of `entries_page_cards.rs` click closures into explicit command dispatch.
- Added entries page bindings to centralize reload and status application.

## Why
- The entries page still had direct business-action closures even after application/use-case consolidation.
- This is the same first-step pattern already used successfully in the feeds page.
- It moves the entries module closer to the documented headless active interface target without changing UI behavior.

## Impact
- Entries page only.
- Read/unread and star/unstar actions now flow through command dispatch.
- No intended visual change.

## Files
- crates/rssr-app/src/pages.rs
- crates/rssr-app/src/pages/entries_page.rs
- crates/rssr-app/src/pages/entries_page_cards.rs
- crates/rssr-app/src/pages/entries_page_commands.rs
- crates/rssr-app/src/pages/entries_page_dispatch.rs
- crates/rssr-app/src/pages/entries_page_bindings.rs

## Verification
- cargo fmt --all
- cargo check -p rssr-app
- cargo check -p rssr-app --target wasm32-unknown-unknown
- git diff --check

## Notes For Next Agent
- This only covers card actions, not entries-page query loading or preference persistence.
- The next natural headless step for entries is to decide whether filters/grouping persistence should get the same command/binding treatment.

# 2026-04-08 | Entries headless command surface step 2

## Summary
- Routed entries-page browsing preference persistence through the same command/dispatch/bindings path used for card actions.
- Added a silent command outcome for background preference saves so successful persistence does not spam the status banner.
- Removed direct settings load/mutate/save logic from the entries-page effect body.

## Why
- The entries page still saved grouping and filter preferences through ad hoc service calls inside a page effect.
- This was inconsistent with the headless direction already started for entry-card actions.
- The page should assemble state and dispatch commands, not implement persistence flow inline.

## Impact
- Entries page only.
- Grouping mode, archived visibility, read/star filters, and source filters still persist the same way.
- Successful preference saves remain quiet; failures now flow through the same command outcome path.

## Files
- crates/rssr-app/src/pages/entries_page.rs
- crates/rssr-app/src/pages/entries_page_commands.rs
- crates/rssr-app/src/pages/entries_page_dispatch.rs
- crates/rssr-app/src/pages/entries_page_bindings.rs

## Verification
- cargo fmt --all
- cargo check -p rssr-app
- cargo check -p rssr-app --target wasm32-unknown-unknown
- git diff --check

## Notes For Next Agent
- The next entries headless step is likely query loading and filter form changes, not card actions or preference persistence anymore.
- If status handling grows further, consider promoting a more generic optional-status outcome shape across page command modules.

# 2026-04-08 | Web command surface thinning

## Summary
- Removed `rssr-app/src/bootstrap/web/mutations.rs`.
- Routed Web entry mutations through shared `EntryService`.
- Routed Web settings load/save through shared `SettingsService`.
- Moved browser last-opened-feed persistence behind `BrowserAppStateAdapter` methods.
- Removed stale browser-only `validate_settings()` helper after command-side routing no longer needed it.

## Why
- `rssr-app` wasm bootstrap should continue converging toward assembly/platform glue.
- The previous `mutations.rs` kept browser state write logic in the app crate even though repository/service abstractions already existed.
- This aligns Web command-side behavior with native, which already uses `EntryService` and `SettingsService`.

## Impact
- Web only.
- No intended product behavior change.
- `mutations.rs` is no longer a maintenance hotspot.

## Files
- crates/rssr-app/src/bootstrap/web.rs
- crates/rssr-infra/src/application_adapters/browser/adapters.rs
- crates/rssr-infra/src/application_adapters/browser/config.rs
- crates/rssr-app/src/bootstrap/web/mutations.rs (deleted)

## Verification
- cargo fmt --all
- cargo check -p rssr-infra
- cargo check -p rssr-app
- cargo check -p rssr-app --target wasm32-unknown-unknown
- git diff --check

## Notes For Next Agent
- `refresh.rs` remains in `rssr-app` because it is still platform scheduling glue, not shared application behavior.
- Query-side Web logic still reads directly from browser persisted state through infra browser query helpers; that is acceptable for now.

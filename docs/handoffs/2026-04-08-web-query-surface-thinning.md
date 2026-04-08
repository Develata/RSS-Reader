# 2026-04-08 | Web query surface thinning

## Summary
- Routed Web feed listing through shared `FeedService`.
- Routed Web entry listing, entry lookup, and reader navigation through shared `EntryService`.
- Removed the last direct query-side reads of browser persisted state from `rssr-app/src/bootstrap/web.rs`.

## Why
- `rssr-app` wasm bootstrap should keep converging toward platform assembly rather than maintaining browser-specific query orchestration.
- Browser repositories in `rssr-infra` already expose the necessary query behavior.
- This keeps Web closer to the same service-oriented shape as native.

## Impact
- Web only.
- No intended product behavior change.
- `web.rs` is now primarily assembly, auto-refresh wiring, and remote-config glue.

## Files
- crates/rssr-app/src/bootstrap/web.rs

## Verification
- cargo fmt --all
- cargo check -p rssr-app
- cargo check -p rssr-app --target wasm32-unknown-unknown
- git diff --check

## Notes For Next Agent
- Query-side browser behavior still ultimately reads from browser persisted state, but now only through infra repositories.
- The next architectural question is whether browser app-state should become a formal repository/port, not whether `web.rs` should keep local query logic.

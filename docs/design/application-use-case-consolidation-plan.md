# Application Use Case Consolidation Plan

## Purpose

This plan defines the next architecture step after the UI runtime and host capability narrowing
work. The goal is to make the application layer the stable home for RSS-Reader's core use cases,
without moving host lifecycle, platform adapters, UI copy, or presentation policy into
`rssr-application`.

This plan follows constitution `1.3.0`: skeleton boundaries first, then module boundaries, then
implementation details.

## Skeleton Boundary

The work stays inside the existing product skeleton:

- Subscription management
- Feed refresh
- Basic config exchange

It does not introduce a new product axis. It does not add article sync, account sync, AI features,
plugin loading, a rule engine, or a new global store.

## Module List

- `rssr-domain`
  - Owns stable entities, value objects, repository traits, and domain rules.
  - No planned first-step changes.
- `rssr-application`
  - Owns shared use cases and workflow outcomes.
  - First-step target: subscription lifecycle workflow.
- `rssr-infra`
  - Owns SQLite, browser storage, HTTP fetch, parser, OPML, WebDAV, and adapter implementations.
  - No planned first-step changes except tests if a contract gap appears.
- `rssr-app`
  - Owns UI runtime, host lifecycle, platform capabilities, and visible outcome translation.
  - First-step target: consume the narrower application workflow instead of reassembling
    subscription lifecycle decisions.
- `rssr-cli`
  - Owns CLI argument parsing, file I/O, terminal output, and exit behavior.
  - First-step target: delegate optional first refresh to the application workflow.

## Truth Sources

- Feeds: `FeedRepository`
- Entries and read/starred flags: `EntryRepository`
- Durable settings: `SettingsRepository`
- App workspace state and last-opened feed: `AppStateRepository` through `AppStateService`
- Config exchange payload: `ImportExportService` and config package v2 schema
- Browser persisted app state: `rssr-web-app-state-v2`

No UI cache, CLI state, browser helper seed, or smoke fixture may become a parallel truth source.

## Adapter And Capability Boundaries

- SQLite persistence enters through infra repositories and application ports.
- Browser storage enters through browser infra adapters.
- HTTP feed fetch enters through `FeedRefreshSourcePort`.
- Feed commit/writeback enters through `RefreshStorePort`.
- OPML codec enters through `OpmlCodecPort`.
- WebDAV enters through `RemoteConfigStore`.
- Native image localization remains a host worker. It consumes refresh localization candidates but
  must not become part of the shared refresh use case.
- Auto-refresh scheduling remains a host capability. It may call application use cases, but its
  loop, timing, and task lifecycle remain outside `rssr-application`.

## Failure, Observability, And Idempotency

Subscription lifecycle must define:

- Invalid URL: rejected by `FeedService` before persistence.
- Duplicate or normalized URL: handled by repository upsert semantics.
- First refresh failure: returned as a structured refresh outcome, not hidden by the add step.
- Missing refresh target after add: reported as first refresh failure.
- Remove subscription: clears matching last-opened feed state after feed removal.
- Repeated remove: must remain repository-defined and not corrupt app state.

Host layers may translate outcomes into user-facing messages, logs, or exit status. They must not
invent new business semantics.

## First Implementation Step

Scope:

- Add one application-level subscription lifecycle input and outcome that supports optional first
  refresh.
- Keep existing `add_subscription` and `add_subscription_and_refresh` as compatibility helpers if
  useful, but make the new lifecycle method the common implementation path.
- Update CLI to delegate `--skip-refresh` to the lifecycle method.
- Update native/web refresh capabilities to call the lifecycle method with first refresh enabled.
- Add application tests for both refresh and no-refresh branches.

Out of scope for the first step:

- Refresh-all summary helpers.
- Config exchange workflow changes.
- Auto-refresh scheduling.
- Native image localization worker changes.
- UI copy changes.

## Validation Plan

Minimum validation for the first step:

- `cargo fmt --check`
- `cargo test -p rssr-application`
- `cargo check -p rssr-app`
- `cargo check -p rssr-app --target wasm32-unknown-unknown`
- `cargo check -p rssr-cli`
- `cargo test -p rssr-infra --test test_subscription_contract_harness`
- `scripts/run_rssr_web_browser_feed_smoke.sh --skip-build`

If the first step touches browser-visible behavior, also run Windows visible Chrome regression.

## Follow-Up Steps

After subscription lifecycle is stable:

1. Refresh outcome summarization
   - Consider application helpers for stable failure summaries and counts.
   - Keep platform-specific localization and scheduling in host capabilities.
2. Config exchange consolidation
   - Check for remaining duplicated JSON/OPML/WebDAV validation and cleanup logic.
   - Keep remote store construction in adapters.
3. Verification hardening
   - Add focused contract tests when a shared outcome or migration boundary changes.

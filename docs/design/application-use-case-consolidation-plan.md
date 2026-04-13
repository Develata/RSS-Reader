# Application Use Case Consolidation Plan

## Purpose

This plan defines the next architecture step after the UI runtime and host capability narrowing
work. The goal is to make the application layer the stable home for RSS-Reader's core use cases,
without moving host lifecycle, platform adapters, UI copy, or presentation policy into
`rssr-application`.

This plan follows constitution `1.3.0`: skeleton boundaries first, then module boundaries, then
implementation details.

## Current Status

The original first-step targets in this plan are no longer just proposals. The following
consolidation work is already on the mainline:

- Subscription lifecycle moved into `SubscriptionWorkflow`
- Refresh outcome summaries moved into `RefreshService`
- Config import/export and remote config outcomes moved into `ImportExportService`
- Entries workspace bootstrap/save decisions moved into `EntriesWorkspaceService`
- Feed snapshot query moved into `FeedsSnapshotService`
- CLI feed listing moved into `FeedCatalogService`
- Feed summaries queries for startup/workspace moved off `FeedService`
- Startup last-opened feed validation now uses the feed repository's single-feed lookup instead of
  feed summary aggregation.
- Pure facade services removed:
  - `ShellService`
  - `SettingsPageService`
  - `EntryService`

The next step is no longer "add the first shared workflow". The next step is to keep the
application layer coherent while these use cases continue to split by responsibility.

## Runtime Boundary Check 2026-04-13

Reviewed files:

- `crates/rssr-application/src/composition.rs`
- `crates/rssr-app/src/ui/runtime/services.rs`

Current result:

- `AppUseCases::compose()` is still the only application composition entry.
- UI runtime ports call `AppUseCases`; they do not take repositories directly.
- Runtime host capabilities are still limited to host-owned behavior:
  - auto-refresh lifecycle
  - refresh execution capability with host-visible outcomes
  - remote config transport
  - clipboard access
- Deleted facades (`ShellService`, `SettingsPageService`, `EntryService`) have not reappeared in
  composition or runtime calls.
- Current naming still matches the classification baseline below.

No code change is required from this check. The next application-layer work should still be driven
by real boundary pressure, not by mass renaming.

## Startup Query Narrowing 2026-04-13

Reviewed files:

- `crates/rssr-application/src/startup_service.rs`
- `crates/rssr-application/src/entries_workspace_service.rs`
- `crates/rssr-application/src/app_state_service.rs`
- `crates/rssr-application/src/settings_service.rs`
- `crates/rssr-application/src/settings_sync_service.rs`

Current result:

- `StartupService` uses `FeedRepository::get_feed(feed_id)` to validate a last-opened feed instead
  of loading all feed summaries.
- `EntriesWorkspaceService` still loads feed summaries because its bootstrap output can return the
  feeds list to the UI; that remains a query surface, not a command-service passthrough.
- `AppStateService` remains the owner for app-state slices such as entries workspace state and
  last-opened feed. Replacing it with direct repository access in callers would duplicate snapshot
  read/modify/write rules.
- `SettingsSyncService` remains thin but intentional: it translates a remote pull/import outcome
  into the settings snapshot the host needs after import.

No workflow split is required from this check. The useful code change is the narrower startup
existence query, not a broad service-to-repository rewrite.

## ImportExportService Boundary Check 2026-04-13

Reviewed files:

- `crates/rssr-application/src/import_export_service.rs`
- `crates/rssr-application/src/import_export_service/tests.rs`
- `crates/rssr-application/src/composition.rs`
- `crates/rssr-app/src/ui/runtime/services.rs`
- `crates/rssr-cli/src/main.rs`

Current result:

- `ImportExportService` should remain one service for now.
- JSON config export/import is still the source of truth for config exchange.
- OPML export/import is an interop view over the same feed membership data.
- Remote config push/pull only transports the same config package payload.
- Removed feed cleanup is a direct consequence of config import replacing feed membership, not a new
  product axis.
- UI runtime and CLI still call this service through `AppUseCases`; they do not reconstruct config
  exchange decisions locally.

The split trigger remains future growth beyond config-exchange semantics: continuous sync,
conflict resolution, account identity, background scheduling, or a separate OPML subscription
management surface. None of those is present in the current implementation.

## Boundary Checklist Sweep 2026-04-13

Reviewed files and searches:

- `rg` for repository types in `crates/rssr-app/src` and `crates/rssr-cli/src`
- `rg` for `list_feeds`, `list_summaries`, `get_feed`, `list_entries`, and `get_entry` in
  `crates/rssr-application/src`
- `crates/rssr-app/src/bootstrap/native.rs`
- `crates/rssr-app/src/bootstrap/web.rs`
- `crates/rssr-cli/src/main.rs`

Current result:

- UI runtime and CLI command handlers still enter the application layer through `AppUseCases`.
- Repository construction in native, web, and CLI bootstrap remains composition wiring, not command
  handler access.
- Application-layer query methods match their current output surfaces:
  - `FeedCatalogService` uses full feed entities for CLI listing.
  - `FeedsSnapshotService` and `EntriesWorkspaceService` use feed summaries where their outputs need
    summaries.
  - `StartupService` uses single-feed lookup for last-opened feed existence.
  - `ImportExportService` uses full feed entities where config export/import and OPML interop need
    full feed membership data.
- Native `ImageLocalizationWorker` holds `SqliteEntryRepository` directly as a host worker for
  hash-checked background HTML localization writeback. This remains the documented native image
  localization exception and must not be copied into UI runtime ports.

No code change is required from this sweep.

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
  - Current target: keep command/query/workflow boundaries clean and avoid service-to-service
    query passthrough.
- `rssr-infra`
  - Owns SQLite, browser storage, HTTP fetch, parser, OPML, WebDAV, and adapter implementations.
  - No planned first-step changes except tests if a contract gap appears.
- `rssr-app`
  - Owns UI runtime, host lifecycle, platform capabilities, and visible outcome translation.
  - Current target: keep calling shared use cases and host capabilities without reassembling
    business decisions locally.
- `rssr-cli`
  - Owns CLI argument parsing, file I/O, terminal output, and exit behavior.
  - Current target: keep shell behavior thin and route feed/config actions through application use
    cases instead of repositories.

## Application Boundary Rules

### Query and command split

- Query use cases may depend directly on repositories when they only read stable truth sources.
- Command use cases may depend on repositories and write-side ports needed for the mutation.
- Query use cases must not route reads through command services just to reuse a method name.
- Command services should not grow "small query helpers" unless the query is inseparable from the
  command invariant.

### Workflow versus single-purpose service

- A workflow service is allowed to orchestrate multiple use cases or ports when it represents one
  real product action with branching outcomes.
- A single-purpose service should stay narrow and map cleanly to one page/bootstrap/query/command
  responsibility.
- Host layers may compose multiple application calls for presentation, but they must not recreate
  business branching that already exists in a workflow.

In the current codebase this distinction is intentional:

- `SubscriptionWorkflow` stays a workflow because it spans multiple use cases and state effects:
  subscription mutation, optional first refresh, and last-opened-feed cleanup on removal.
- `RefreshService` stays a service because it owns one coherent capability family: resolve refresh
  targets, invoke refresh source/store ports, and return stable refresh outcomes. It is internally
  multi-step, but it does not orchestrate across unrelated product actions the way
  `SubscriptionWorkflow` does.
- `ImportExportService` also stays a service for now because JSON config export/import, OPML
  exchange, and remote config push/pull are all part of one config-exchange capability family.
  It should only be split when one branch starts carrying materially different lifecycle or
  side-effect coordination that no longer belongs to the same exchange boundary.

### Naming baseline

The project does not need an immediate mass rename. It does need stable naming rules:

- Use `*Workflow` for multi-step business actions with branching flow, such as
  `SubscriptionWorkflow`.
- Use `*Service` for stable application use cases, whether they are query-oriented or
  command-oriented.
- Query-oriented services should prefer names that describe the returned surface or view, such as
  `FeedsSnapshotService`, `FeedCatalogService`, and `EntriesListService`.
- Command-oriented services should prefer names that describe the acted-on domain object, such as
  `FeedService` after it was reduced to add/remove subscription behavior.
- Page-shaped services such as `SettingsPageService` are acceptable only when they consolidate one
  UI surface's stable application semantics and do not absorb presentation-only policy.

### Current classification baseline

- Workflow:
  - `SubscriptionWorkflow`
- Feed query services:
  - `FeedCatalogService`
  - `FeedsSnapshotService`
- Feed command service:
  - `FeedService`
- Entry/query interaction services:
  - `EntriesListService`
  - `EntriesWorkspaceService`
  - `ReaderService`
- Settings/app-state coordination:
  - `SettingsService`
  - `SettingsSyncService`
  - `AppStateService`
  - `StartupService`

This classification is a boundary baseline, not a rename order. The main rule is to keep future
changes consistent with it.

`SettingsSyncService` is intentionally retained even though it is thin. Its job is not generic
repository access; it defines the application outcome for "remote config pull has already been
applied, now expose whether it imported and what the current effective settings are". That
semantic is stable enough to justify its own use case.

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

## Current Implementation Priority

Scope:

- Keep new query use cases independent from command services.
- Keep shells and UI runtime on `AppUseCases` instead of direct repository access.
- Add documentation-level guidance before any mass rename.
- Rename only when a name actively hides a boundary or causes repeated misuse.

Out of scope for the current step:

- Workspace-wide renaming from `*Service` to `*Query` / `*Command`.
- Reorganizing modules just for visual symmetry.
- Moving host lifecycle, browser storage, or presentation policy into `rssr-application`.

## Validation Plan

Minimum validation for boundary-only follow-up steps:

- `cargo fmt --check`
- `cargo test -p rssr-application`
- `cargo test -p rssr-app`
- `cargo test -p rssr-cli`
- `cargo check --workspace`
- `git diff --check`

If a step touches browser-visible behavior, also run the relevant UI regression and browser feed
smoke gates.

## Follow-Up Steps

1. Naming consistency review
   - Decide whether any current service name actively obscures its boundary.
   - Prefer rule enforcement and small targeted renames over mass churn.
2. Workflow/service split review
   - Re-check whether future multi-step actions should become workflows or remain services.
   - Do not create new workflows just because a service has multiple internal steps.
   - In particular, watch `ImportExportService` for growth beyond config-exchange semantics.
3. Verification hardening
   - Add focused contract tests when a shared outcome or migration boundary changes.

Reference checklist:

- `docs/design/application-use-case-boundary-checklist.md`

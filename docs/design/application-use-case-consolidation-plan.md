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

The next step is no longer "add the first shared workflow". The next step is to keep the
application layer coherent while these use cases continue to split by responsibility.

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
  - `SettingsPageService`
  - `AppStateService`
  - `StartupService`

This classification is a boundary baseline, not a rename order. The main rule is to keep future
changes consistent with it.

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
2. Settings and shell boundary review
   - Re-check whether `SettingsPageService`, `ShellService`, and `StartupService` still reflect
     stable application semantics instead of page/runtime convenience.
3. Verification hardening
   - Add focused contract tests when a shared outcome or migration boundary changes.

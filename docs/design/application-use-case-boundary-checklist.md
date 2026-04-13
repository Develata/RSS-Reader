# Application Use Case Boundary Checklist

Use this checklist when changing `rssr-application`, UI runtime ports, CLI commands, or host
capabilities. It turns the current consolidation plan into a review gate that can be applied before
code changes are merged.

## Skeleton First

- The change fits the existing RSS reader skeleton:
  - subscription management
  - feed refresh
  - reading and read/starred state
  - durable settings
  - basic config exchange
- The change does not introduce a new product axis such as account sync, article sync, AI analysis,
  plugin loading, social sharing, or a rules engine.
- If the change needs a new product axis, stop and write a skeleton-level proposal before editing
  application services.

## Runtime Entry

- UI runtime ports call `AppUseCases` or host capabilities, not repositories.
- CLI command handlers call `AppUseCases`, not repositories.
- Host capabilities stay host-owned:
  - lifecycle and scheduling
  - platform transport
  - clipboard
  - browser/native-specific remote config wiring
  - host-visible refresh execution outcomes
- Host capabilities may call application use cases, but must not recreate business branching that
  already exists in an application workflow or service.

## Query And Command Boundaries

- Query use cases read stable truth sources directly through repositories when they only need data.
- Query use cases do not route through command services just to reuse a convenient method name.
- Command use cases own mutation validation and write-side ports for that mutation.
- Command services do not collect unrelated query helpers.
- Prefer the narrowest repository method that matches the use case:
  - use single-entity lookup for existence checks
  - use summary aggregation only when the caller needs summaries
  - use full entity lists only when export, refresh target creation, or interop requires full data

## Workflow Versus Service

- Use `*Workflow` only for a product action that spans multiple use cases or ports with branching
  outcomes.
- Keep `*Service` for a stable single capability family, even when the internal implementation has
  multiple steps.
- Do not split a service merely because it has several methods.
- Split only when a method family starts carrying materially different lifecycle, failure, state, or
  dependency rules.

## Thin Service Review

A thin service may remain if it owns a stable semantic boundary. Current accepted examples:

- `AppStateService`: owns snapshot slice read/modify/write semantics.
- `SettingsSyncService`: maps a remote pull/import outcome to the effective settings snapshot
  needed by the host.

A thin service should be removed if it only forwards to another service or repository without
adding one of:

- validation
- state slice ownership
- outcome translation
- failure boundary
- cross-port coordination for one product action

## ImportExportService Review

`ImportExportService` remains one service while all of these stay true:

- JSON config export/import is the source of truth for config exchange.
- OPML export/import is an interop view over the same feed membership data.
- Remote config push/pull only transports the same config package payload.
- Removed feed cleanup is a consequence of config import replacing feed membership.
- The service does not grow scheduling, conflict resolution, multi-device merge policy, account
  identity, or background sync lifecycle.

Split it only if a branch gains a different lifecycle or system identity. Examples that would
require a new design review:

- remote config becomes continuous sync
- import grows interactive conflict resolution
- OPML becomes a separate subscription management surface instead of simple interop
- config package versioning requires migration orchestration outside codec/rules validation

## Verification Gate

For boundary-only changes:

- `cargo fmt --check`
- `cargo test -p rssr-application`
- `cargo test -p rssr-app`
- `cargo test -p rssr-cli`
- `cargo check --workspace`
- `git diff --check`

For infra or config exchange behavior:

- `cargo test -p rssr-infra`
- targeted config package codec/schema tests when payload structure changes
- wasm contract harness when browser storage or refresh/config contracts change

For browser-visible behavior:

- relevant static web or `rssr-web` smoke scripts
- browser feed/proxy smoke when refresh behavior or diagnostics changes

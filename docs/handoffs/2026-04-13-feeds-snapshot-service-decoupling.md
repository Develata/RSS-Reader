# Feeds Snapshot Service Decoupling

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：136a3e8
- 相关 commit：136a3e8
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

将 `FeedsSnapshotService` 从 `FeedService` 解耦，改为直接依赖 `FeedRepository::list_summaries()`，减少 application 层 service 之间的交错编排。

## 影响范围

- 模块：
  - `crates/rssr-application/src/feeds_snapshot_service.rs`
  - `crates/rssr-application/src/composition.rs`
- 平台：
  - desktop
  - Android
  - Web
  - CLI
- 额外影响：
  - application use case 边界

## 关键变更

### Feeds Snapshot Query Boundary

- `FeedsSnapshotService` 不再持有 `FeedService`。
- `load_snapshot()` 改为直接读取 `FeedRepository::list_summaries()`，然后在本 use case 内聚合：
  - `feeds`
  - `feed_count`
  - `entry_count`

### Composition Cleanup

- `AppUseCases::compose()` 不再把 `feed_service` 注入 `feeds_snapshot_service`。
- `import_export_service` 改为显式 clone `feed_repository`，避免在同一 compose 流程里提前 move 走仓储实例。

### Test Surface Simplification

- `feeds_snapshot_service` 单元测试删除不再需要的 `EntryRepository` stub。
- 测试现在只保留 snapshot query 真正依赖的 `FeedRepository` stub。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo test -p rssr-application`：通过
- `cargo test -p rssr-app`：通过
- `git diff --check`：通过

### 手工验收

- 未执行；本次为 application service 边界收敛，主要依赖单元测试与上层 app 测试覆盖。

## 结果

- 本次变更已验证，可合并。
- `FeedsSnapshotService` 现在是独立 query use case，不再借道 `FeedService`。

## 风险与后续事项

- CLI 的 `ListFeeds` 仍然直接访问 `feed_repository.list_feeds()`，说明 shell 到仓储的直连还没有完全收干净。
- `FeedService` 目前仍同时承载：
  - add/remove subscription 命令
  - list summaries 查询
- 下一步更值得做的是决定 `FeedService::list_feeds()` 是否继续保留；若目标是 application use case 更纯，建议把 CLI 列表查询也收敛到独立 use case，而不是继续让 shell 持有仓储。

## 给下一位 Agent 的备注

- 若继续 application 收敛，优先看 `crates/rssr-cli/src/main.rs` 里的 `CliServices::list_feeds()`。
- 这一步之后，`feeds_snapshot_service.rs` 已经不再需要 `FeedService` 参与，后续不要再把 query use case 重新套回 service-to-service 依赖。

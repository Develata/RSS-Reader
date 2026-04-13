# Feed Summaries Query Isolation

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：9b1e268
- 相关 commit：9b1e268
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

把 `StartupService` 和 `EntriesWorkspaceService` 里的 feed summaries 查询从 `FeedService` 中拆出，改为各自直接依赖 `FeedRepository::list_summaries()`，让 `FeedService` 退回订阅命令 use case。

## 影响范围

- 模块：
  - `crates/rssr-application/src/startup_service.rs`
  - `crates/rssr-application/src/entries_workspace_service.rs`
  - `crates/rssr-application/src/feed_service.rs`
  - `crates/rssr-application/src/composition.rs`
- 平台：
  - desktop
  - Android
  - Web
  - CLI
- 额外影响：
  - application use case 边界
  - service-to-service 查询依赖

## 关键变更

### Startup / Workspace Query Boundaries

- `StartupService` 不再持有 `FeedService`，改为直接持有 `FeedRepository`。
- `EntriesWorkspaceService` 不再持有 `FeedService`，改为直接持有 `FeedRepository`。
- 两者都直接调用 `list_summaries()`，不再通过中间 service 转发查询。

### Feed Service 收口

- `FeedService::list_feeds()` 已删除。
- `FeedService` 现在只保留：
  - `add_subscription`
  - `remove_subscription`

### Composition / Tests

- `AppUseCases::compose()` 改为向 `StartupService` 与 `EntriesWorkspaceService` 直接注入 `feed_repository`。
- 两个模块的测试 stub 删除不再需要的 `EntryRepository` / `FeedService` 组装。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo test -p rssr-application`：通过
- `cargo test -p rssr-app`：通过
- `cargo test -p rssr-cli`：通过
- `git diff --check`：通过

### 手工验收

- 未执行；本次为 application 层查询边界收敛，依赖模块测试与上层 app 测试。

## 结果

- 本次变更已验证，可合并。
- `FeedService` 已从“命令 + summaries 查询”收敛为更纯的订阅命令 use case。

## 风险与后续事项

- 现在 application 层里仍然存在多个“各自直接依赖 repository 的 query use case”，这是比跨 service 编排更干净的形态，但还没有形成统一命名体系。
- 如果继续收敛，下一步值得审查的是：
  - `feeds_snapshot_service`
  - `feed_catalog_service`
  - 是否要明确区分 `query` / `command` 服务命名

## 给下一位 Agent 的备注

- 若继续做 application use case 收敛，先看 `crates/rssr-application/src/composition.rs`，这里已经能看出：
  - 订阅命令 use case
  - feed full-entity query use case
  - feed summaries query use case
  的分离趋势。
- 下一步应该是命名和边界一致性审查，不是继续机械拆 helper。

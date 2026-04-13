# CLI Feed Catalog Service

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：a94856e
- 相关 commit：a94856e
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

去掉 CLI shell 对 `FeedRepository` 的直接依赖，新增独立 `FeedCatalogService` 作为“列出完整订阅实体”的 application use case。

## 影响范围

- 模块：
  - `crates/rssr-application/src/feed_catalog_service.rs`
  - `crates/rssr-application/src/composition.rs`
  - `crates/rssr-application/src/lib.rs`
  - `crates/rssr-cli/src/main.rs`
- 平台：
  - CLI
  - desktop
  - Android
  - Web
- 额外影响：
  - application use case 边界
  - shell 到仓储的依赖方向

## 关键变更

### Feed Catalog Query Use Case

- 新增 `FeedCatalogService`，只负责 `FeedRepository::list_feeds()`。
- 返回完整 `Feed` 实体，保留 CLI 当前需要的：
  - `folder`
  - `url`
  - `title`
  - `id`

### Composition Wiring

- `AppUseCases` 新增 `feed_catalog_service`。
- 组合层显式注入 `feed_repository` 到 `FeedCatalogService`，不借道 `FeedService`。

### CLI Dependency Cleanup

- `CliServices` 删除 `feed_repository` 字段。
- `list_feeds()` 改为走 `self.use_cases.feed_catalog_service.list_feeds()`。
- CLI 仍保持原来的输出形态，不改变用户可见行为。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo test -p rssr-application`：通过
- `cargo test -p rssr-cli`：通过
- `cargo check --workspace`：通过
- `git diff --check`：通过

### 手工验收

- 未执行；本次主要是壳层依赖方向调整，依赖 application/CLI 自动化验证。

## 结果

- 本次变更已验证，可合并。
- CLI 不再直接持有仓储，shell -> application use case -> repository 的依赖方向更一致。

## 风险与后续事项

- `FeedService` 目前仍混合了“订阅命令”和“feed summaries 查询”；后续若继续收敛，可考虑把 summary query 也拆成独立 use case。
- `StartupService` 和 `EntriesWorkspaceService` 还通过 `FeedService` 读取 summaries；这条边界虽然没有 shell 直连问题，但仍是 service-to-service 查询依赖。

## 给下一位 Agent 的备注

- 如果继续 application 收敛，下一步优先看：
  - `crates/rssr-application/src/startup_service.rs`
  - `crates/rssr-application/src/entries_workspace_service.rs`
- 这两个地方都只是读 feed summaries，适合对齐成独立 query use case，而不是继续挂在 `FeedService` 下。

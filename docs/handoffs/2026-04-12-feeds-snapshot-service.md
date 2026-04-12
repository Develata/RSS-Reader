# Feeds Snapshot Service Consolidation

- 日期：2026-04-12
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：8d10af1
- 相关 commit：8d10af1
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

将订阅管理页快照加载收敛到 application use case，移除 UI runtime 中的 `FeedService` / `EntryService` 跨服务编排，并避免为统计总文章数读取完整文章列表。

## 影响范围

- 模块：
  - `crates/rssr-application/src/feeds_snapshot_service.rs`
  - `crates/rssr-application/src/composition.rs`
  - `crates/rssr-application/src/lib.rs`
  - `crates/rssr-app/src/ui/runtime/feeds.rs`
  - `crates/rssr-app/src/ui/runtime/services.rs`
- 平台：
  - Linux
  - Web
  - 桌面端共享 application/runtime 路径
- 额外影响：
  - release UI regression 记录：`target/release-ui-regression/20260412-codex-feeds-snapshot-service/summary.md`

## 关键变更

### Application Use Case

- 新增 `FeedsSnapshotService` 和 `FeedsSnapshotOutcome`，统一承载订阅管理页快照加载。
- `entry_count` 改为从 `FeedSummary.entry_count` 汇总，不再通过 `EntryQuery::default()` 读取完整文章列表后取长度。
- 测试明确约束 feeds snapshot 不依赖 entry listing，避免后续重新引入跨 service 查询。

### UI Runtime

- `FeedsCommand::LoadSnapshot` 改为调用 `services.feeds().load_snapshot()`，runtime 只负责 outcome 到 `FeedsPageSnapshot` 的映射。
- 删除 `FeedsPort::list_feeds` 和 `FeedsPort::list_entries` 直通入口，减少 UI runtime 对基础 service 的直接暴露。

### Composition

- `AppUseCases` 注入 `feeds_snapshot_service`，与 `entries_list_service`、`entries_workspace_service`、`reader_service` 等 use case 并列。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo fmt --check`：通过
- `git diff --check`：通过
- `cargo test -p rssr-application`：通过，37 tests
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-cli`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo test --workspace`：通过
- `bash scripts/run_release_ui_regression.sh --no-serve --with-rssr-web --port 8351 --web-port 18851 --log-dir target/release-ui-regression/20260412-codex-feeds-snapshot-service`：通过

### 手工验收

- 未执行独立手工 UI 点击验收；本次依赖 release UI 自动门禁覆盖 feeds 页面和 rssr-web browser feed smoke。

## 结果

- 本次交付可合并；订阅管理页快照加载已进入 application 层。
- `rssr-web browser feed smoke` 本轮通过，未复现超时。
- feeds 快照不再为统计总文章数读取、排序并分配完整文章摘要列表。

## 风险与后续事项

- 下一步结构清洁重点是拆分 `crates/rssr-infra/src/application_adapters/browser/adapters.rs`，它目前集中承载多个 adapter 职责。
- settings 远端 pull 后重新 load settings 仍在 UI runtime 编排，可后续收敛为 settings sync use case。
- `cargo clippy --workspace --all-targets` 仍有既有测试 fixture dead_code warning 和 `Navigator` clone_on_copy warning；本次未处理。
- push 仍依赖 GitHub HTTPS 凭据；本地验证通过不代表远端已更新。

## 给下一位 Agent 的备注

- 阅读入口：`crates/rssr-application/src/feeds_snapshot_service.rs` 与 `crates/rssr-app/src/ui/runtime/feeds.rs`。
- 继续推进时，建议先拆 browser adapter 文件边界，再处理 settings sync 编排和小型 clippy warning。

# App State Cleanup Port Unification

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：7ec485a
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

继续推进 P2 边界债务清理，删除与 `AppStatePort` 语义重复的 `FeedRemovalCleanupPort`，让 subscription removal 和 config import removal 都通过同一 app-state cleanup port 清理 `last_opened_feed_id`。

## 影响范围

- 模块：
  - `crates/rssr-application/src/import_export_service.rs`
  - `crates/rssr-application/src/subscription_workflow.rs`
  - `crates/rssr-application/src/composition.rs`
  - `crates/rssr-application/src/lib.rs`
  - `crates/rssr-infra/src/application_adapters/non_refresh.rs`
  - `crates/rssr-infra/src/application_adapters/browser/adapters/app_state.rs`
  - `crates/rssr-infra/tests/test_config_exchange_contract_harness.rs`
  - `crates/rssr-infra/tests/wasm_config_exchange_contract_harness.rs`
  - `docs/design/application-use-case-consolidation-plan.md`
- 平台：
  - Linux
  - wasm32 / Web
  - Desktop / native
  - CLI
- 额外影响：
  - application composition boundary
  - config exchange cleanup tests

## 关键变更

### Port Boundary

- 删除 `FeedRemovalCleanupPort` trait 与 public re-export。
- `ImportExportService` 改为依赖 `Arc<dyn AppStatePort>` 执行 removed-feed app-state cleanup。
- `AppStateServicesPort` 约束从 `AppStateRepository + AppStatePort + FeedRemovalCleanupPort + Send + Sync` 收窄为 `AppStateRepository + AppStatePort + Send + Sync`。

### Constructors And Adapters

- `ImportExportService::new_with_feed_removal_cleanup(...)` 改名为 `new_with_app_state_cleanup(...)`。
- `ImportExportService::new_with_feed_removal_cleanup_and_clock(...)` 改名为 `new_with_app_state_cleanup_and_clock(...)`。
- SQLite 与 browser app-state adapters 移除重复的 `FeedRemovalCleanupPort` impl，仅保留 `AppStatePort` impl。
- tests / contract harness 的 recording cleanup stub 改为实现 `AppStatePort`。

### Design Docs

- 在 `docs/design/application-use-case-consolidation-plan.md` 记录 2026-04-13 的 app-state cleanup port sweep 结论。

## 验证与验收

### 自动化验证

- `cargo test -p rssr-application`：通过，43 passed
- `cargo test -p rssr-infra --test test_config_exchange_contract_harness`：通过，4 passed
- `cargo test -p rssr-infra --lib`：通过，17 passed
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo test -p rssr-cli`：通过，0 tests
- `cargo fmt --check`：通过
- `git diff --check`：通过

### 手工验收

- `rg` 确认 `crates/**/*.rs` 中已无 `FeedRemovalCleanupPort`、`new_with_feed_removal_cleanup`、`feed_removal_cleanup`、`RecordingFeedRemovalCleanup`、`NoopFeedRemovalCleanup` 残留：通过
- 静态代码复核：通过，removed-feed cleanup 仍调用 `clear_last_opened_feed_if_matches(feed_id)`，行为未改变

## 结果

- app-state cleanup 语义现在只有一个 application port：`AppStatePort`。
- application composition 不再要求同一个 adapter 重复实现两个等价 cleanup trait。

## 风险与后续事项

- 这是 application public API 层的命名清理；当前 workspace 调用点已全部更新，但如果外部 crate 直接依赖 `rssr-application` 的旧 constructor / trait 名称，需要同步改名。
- 下一步可继续处理 P3 的 `config_import_summary` 重复文案与结果汇总逻辑。

## 给下一位 Agent 的备注

- 入口文件：
  - `crates/rssr-application/src/import_export_service.rs`
  - `crates/rssr-application/src/composition.rs`
  - `crates/rssr-infra/src/application_adapters/non_refresh.rs`
  - `crates/rssr-infra/src/application_adapters/browser/adapters/app_state.rs`
- 当前同一 worktree 里还存在未提交的 auth 拆分与 fetch response classification 增量；本次 port unification 与这些文件互不重叠。

# refresh contract harness step1

- 日期：2026-04-09
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：aede229
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`implemented`

## 工作摘要

按 `docs/testing/contract-harness-rebuild-plan.md` 的阶段 1 落地一份新的 `refresh contract harness`，先覆盖当前主线中可直接在 host 环境执行的 SQLite / `RefreshService` contract。

## 影响范围

- 模块：
  - `crates/rssr-infra/tests/test_refresh_contract_harness.rs`
- 平台：
  - host / sqlite fixture

## 关键变更

- 新增 `test_refresh_contract_harness.rs`
- 使用 `ScriptedSource` 驱动共享 `RefreshService`
- 复用当前主线的：
  - `SqliteRefreshStore`
  - `SqliteFeedRepository`
  - `SqliteEntryRepository`
- 当前覆盖的 contract 面：
  - updated feed metadata + entries
  - not modified
  - failed refresh
  - `refresh_all` 聚合结果

## 已执行验证

- 待执行：
  - `cargo fmt --all`
  - `cargo check -p rssr-infra`
  - `cargo test -p rssr-infra --test test_refresh_contract_harness`
  - `git diff --check`

## 当前状态

- host / sqlite fixture 已落地。
- browser fixture 尚未进入这一轮实现。

## 风险与待跟进

- 当前 `rssr-infra::application_adapters::browser` 仅在 `wasm32` 下导出，因此旧分支那种“同一份 host harness 同时跑 sqlite + browser fixture”的结构不能直接照搬。
- browser fixture 要么：
  - 为 `rssr-infra` 补 wasm test 基座
  - 要么在下一轮单独设计 target-specific harness

## 相关文件

- `crates/rssr-infra/tests/test_refresh_contract_harness.rs`
- `docs/testing/contract-harness-rebuild-plan.md`

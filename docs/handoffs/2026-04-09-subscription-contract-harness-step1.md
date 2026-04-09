# subscription contract harness 第一步

- 日期：2026-04-09
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：c9c13fa
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

重建 `subscription contract harness` 的 host / sqlite baseline，先把新增订阅、URL 规范化去重、删除订阅时 entry 清理，以及 `last_opened_feed_id` 清理这几条共享契约钉到当前主线结构上。

## 影响范围

- 模块：
  - `crates/rssr-infra/tests/test_subscription_contract_harness.rs`
  - `docs/testing/contract-harness-rebuild-plan.md`
- 平台：
  - Linux
  - Web
  - Desktop
  - Android
- 额外影响：
  - docs
  - tests

## 关键变更

### sqlite subscription harness

- 新增 `test_subscription_contract_harness.rs`
- 直接基于：
  - `SqliteFeedRepository`
  - `SqliteEntryRepository`
  - `SqliteAppStateRepository`
  - `SqliteAppStateAdapter`
  - `SubscriptionWorkflow`
- 覆盖：
  - 新增订阅 URL 规范化与去重
  - 删除订阅时的软删除
  - `purge_entries = true` 时的 entry 清理
  - `last_opened_feed_id` 命中与不命中两种清理语义

### 计划进度

- 更新 `contract-harness-rebuild-plan.md`
- 将阶段 2 的当前进度推进到：
  - host / sqlite baseline 已完成
  - browser fixture 尚未开始

## 验证与验收

### 自动化验证

- `cargo fmt --all`：通过
- `cargo check -p rssr-infra`：通过
- `cargo test -p rssr-infra --test test_subscription_contract_harness`：通过
- `git diff --check`：通过

### 手工验收

- 未执行

## 结果

- `subscription contract harness` 现在已经有了和 `refresh contract harness` 对齐的 host / sqlite 基线
- 下一步可以开始设计 browser / wasm fixture，而不是继续回头补页面层回归

## 风险与后续事项

- 当前仍未覆盖 browser-state fixture
- `add_subscription_and_refresh` 这条组合路径目前仍主要依赖 application 层单元测试，不在本轮 host harness 覆盖面内

## 给下一位 Agent 的备注

- 先看 `crates/rssr-infra/tests/test_subscription_contract_harness.rs`
- 然后对照 `docs/testing/contract-harness-rebuild-plan.md` 里的阶段 2 说明继续补 browser fixture

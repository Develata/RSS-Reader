# subscription contract harness 第二步

- 日期：2026-04-09
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：a592d40
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

为 `subscription contract harness` 补上 browser / wasm fixture，让 `BrowserFeedRepository`、`BrowserEntryRepository`、`BrowserAppStateAdapter` 正式进入这条共享契约验证线。

## 影响范围

- 模块：
  - `crates/rssr-infra/tests/wasm_subscription_contract_harness.rs`
  - `docs/testing/contract-harness-rebuild-plan.md`
- 平台：
  - Web
  - Linux
- 额外影响：
  - docs
  - tests

## 关键变更

### browser / wasm subscription harness

- 新增 `wasm_subscription_contract_harness.rs`
- 直接基于：
  - `BrowserFeedRepository`
  - `BrowserEntryRepository`
  - `BrowserAppStateAdapter`
  - `SubscriptionWorkflow`
- 覆盖：
  - 浏览器状态下新增订阅 URL 规范化与去重
  - 删除订阅时的软删除
  - `purge_entries = true` 时 entry 清理
  - `last_opened_feed_id` 命中与不命中两种清理语义
  - localStorage 写回

### 计划进度

- 更新 `contract-harness-rebuild-plan.md`
- 将阶段 2 推进到：
  - host / sqlite baseline 已完成
  - browser / wasm baseline 已完成

## 验证与验收

### 自动化验证

- `cargo fmt --all`：通过
- `cargo check -p rssr-infra`：通过
- `cargo test -p rssr-infra --target wasm32-unknown-unknown --test wasm_subscription_contract_harness --no-run`：通过
- `git diff --check`：通过

### 手工验收

- 未执行
- 当前未在本机 WSL2 上真实运行 browser WebDriver，沿用 `environment-limitations.md` 中的 `env-limited` 约束

## 结果

- `subscription contract harness` 现在已经具备 host/sqlite 与 browser/wasm 两条 baseline
- 后续可以进入 `config exchange contract harness`，不必再回头补 subscription 主线契约

## 风险与后续事项

- browser / wasm 这条线当前仍以 `.wasm` 编译入口为主，真实浏览器执行依赖 CI runner
- 如果后续要把它纳入 CI，应参考 refresh harness 已经落下来的 runner 脚本模式

## 给下一位 Agent 的备注

- 先看 `crates/rssr-infra/tests/wasm_subscription_contract_harness.rs`
- 如需接 CI，参考 `scripts/run_wasm_refresh_contract_harness.sh`

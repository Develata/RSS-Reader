# config exchange contract harness

- 日期：2026-04-09
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：781bf1a
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

重建 `config exchange contract harness`，补齐 host / sqlite baseline 与 browser / wasm baseline，使 `ImportExportService` 这条共享契约在当前主线的两类 adapter 上都具备稳定测试入口。

## 影响范围

- 模块：
  - `crates/rssr-infra/tests/test_config_exchange_contract_harness.rs`
  - `crates/rssr-infra/tests/wasm_config_exchange_contract_harness.rs`
  - `docs/testing/contract-harness-rebuild-plan.md`
- 平台：
  - Linux
  - Web
- 额外影响：
  - docs
  - tests

## 关键变更

### host / sqlite baseline

- 新增 `test_config_exchange_contract_harness.rs`
- 直接基于：
  - `SqliteFeedRepository`
  - `SqliteEntryRepository`
  - `SqliteSettingsRepository`
  - `SqliteAppStateRepository`
  - `SqliteAppStateAdapter`
  - `ImportExportService`
- 覆盖：
  - JSON config roundtrip
  - 导入时清理被移除 feed 的 entries 与 `last_opened_feed_id`
  - OPML import 的 URL 规范化
  - remote push/pull 的契约

### browser / wasm baseline

- 新增 `wasm_config_exchange_contract_harness.rs`
- 直接基于：
  - `BrowserFeedRepository`
  - `BrowserEntryRepository`
  - `BrowserSettingsRepository`
  - `BrowserAppStateAdapter`
  - `BrowserOpmlCodec`
  - `ImportExportService`
- 覆盖：
  - browser state 导出 JSON
  - browser import 时清理被移除 feed 的 entries 与 `last_opened_feed_id`
  - remote pull roundtrip
  - localStorage 写回

### 计划进度

- 更新 `contract-harness-rebuild-plan.md`
- 三条 contract 线现在都已完成：
  - refresh
  - subscription
  - config exchange

## 验证与验收

### 自动化验证

- `cargo fmt --all`：通过
- `cargo check -p rssr-infra`：通过
- `cargo test -p rssr-infra --test test_config_exchange_contract_harness`：通过
- `cargo test -p rssr-infra --target wasm32-unknown-unknown --test wasm_config_exchange_contract_harness --no-run`：通过
- `git diff --check`：通过

### 手工验收

- 未执行
- browser WebDriver 真执行仍受 `environment-limitations.md` 中的 WSL2 约束影响

## 结果

- `zheye-mainline-stabilization` 中最核心的三条 contract harness 方向，已经全部在当前 `main` 上完成重建
- 当前剩余工作不再是“缺哪条 contract 线”，而是把这些 wasm harness 继续逐条接进可执行 CI runner

## 风险与后续事项

- `wasm_config_exchange_contract_harness` 当前仍是 `.wasm` 编译入口验证，真实浏览器执行要继续沿用 refresh harness 的 runner/CI 模式
- 若后续要继续收束，应优先做 config exchange 的 wasm runner 脚本与 CI job，而不是再增加新的 contract 主题

## 给下一位 Agent 的备注

- 先看 `crates/rssr-infra/tests/test_config_exchange_contract_harness.rs`
- 再看 `crates/rssr-infra/tests/wasm_config_exchange_contract_harness.rs`
- 如需接 browser runner / CI，直接复用 `scripts/run_wasm_refresh_contract_harness.sh` 的模式扩展

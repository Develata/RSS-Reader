# rssr-infra wasm 测试目标分流修复

- 日期：2026-04-14
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：1f0345d
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

修复 `cargo check -p rssr-infra --target wasm32-unknown-unknown --tests` 因原生集成测试误参与 wasm32 编译而失败的问题。

## 影响范围

- 模块：
  - `crates/rssr-infra/tests/`
- 平台：
  - Web / wasm32
  - Windows / 原生测试目标
- 额外影响：
  - 测试编译分流

## 关键变更

### wasm / native 测试目标分流

- 为依赖 `config_sync`、`db`、`parser`、`sqlx`、`NativeSqliteBackend` 等原生能力的 `rssr-infra` 集成测试文件补充 crate-level `#![cfg(not(target_arch = "wasm32"))]`。
- 保持 `wasm_*` 合约测试、浏览器状态测试和 `opml` 互操作测试继续参与 wasm 目标编译，不做无差别屏蔽。

### 修复策略

- 不修改 `rssr-infra` 运行时代码与模块导出条件，仅修正测试目标选择。
- 避免为 wasm 目标引入伪实现或额外 feature，保持当前模块边界与依赖关系不变。

## 验证与验收

### 自动化验证

- `cargo check -p rssr-infra --target wasm32-unknown-unknown --tests`：通过
- `cargo check -p rssr-infra --tests`：通过

### 手工验收

- 复现并检查失败日志，确认根因是原生测试在 wasm32 目标下误编译：通过
- 检查 `crates/rssr-infra/tests/` 中 wasm 专用测试文件仍保留 `#![cfg(target_arch = "wasm32")]` 分流：通过

## 结果

- 本次交付可合并，目标问题已修复。
- 用户现在可以直接执行 `cargo check -p rssr-infra --target wasm32-unknown-unknown --tests` 而不再被原生测试阻塞。

## 风险与后续事项

- 当前分流基于测试文件级 `cfg`，后续如果新增依赖原生后端的集成测试，仍需同步加上相同门控。
- 若未来希望让更多测试在 wasm 和 native 双端共享，建议进一步拆分纯逻辑测试与后端绑定测试。

## 给下一位 Agent 的备注

- 本次修改全部位于 `crates/rssr-infra/tests/`，入口可从新增了 `#![cfg(not(target_arch = "wasm32"))]` 的测试文件开始看。
- 若后续继续整理测试矩阵，优先检查 `wasm_*` 测试、`test_browser_state_seed_contracts.rs` 与 `test_opml_interop.rs` 是否仍覆盖预期的 wasm 可用能力。

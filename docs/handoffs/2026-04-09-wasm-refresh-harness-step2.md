# wasm refresh harness step2

- 日期：2026-04-09
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：8cf8b53
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`implemented`

## 工作摘要

把 `wasm_refresh_contract_harness` 从“仅有基座、能编译的入口”继续扩成覆盖更多 refresh contract 断言的浏览器侧测试文件。

## 影响范围

- 模块：
  - `crates/rssr-infra/tests/wasm_refresh_contract_harness.rs`
- 平台：
  - wasm32 / browser

## 关键变更

- 新增 browser-side contract 断言：
  - `RefreshCommit::Updated`
  - `RefreshCommit::Failed`
- 继续保留并复用已有断言：
  - `list_targets`
  - `RefreshCommit::NotModified`
- 每个测试都显式清理 browser localStorage，避免 `STORAGE_KEY` 相互污染。

## 已执行验证

- 待执行：
  - `cargo fmt --all`
  - `git diff --check`
  - `cargo check -p rssr-infra`
  - `cargo test -p rssr-infra --target wasm32-unknown-unknown --test wasm_refresh_contract_harness --no-run`

## 当前状态

- wasm/browser harness 的 contract 断言范围已经明显扩大。
- 当前机器没有：
  - `wasm-bindgen-test-runner`
  - `wasm-pack`
  - 可用 browser runner
  因此本轮只能确认 wasm test target 可编译，尚未在真实浏览器中执行。

## 风险与待跟进

- 下一步如果要让这组测试真正运行起来，需要补：
  - browser runner 工具链
  - 统一的执行命令
  - CI 对 wasm browser test 的支持

# wasm refresh harness base

- 日期：2026-04-09
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：e7a14ad
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`implemented`

## 工作摘要

为 browser / wasm contract harness 建立第一层测试基座：

- 为 `rssr-infra` 增加 wasm 测试依赖
- 新增一个独立的 `wasm_refresh_contract_harness` 入口
- 先覆盖 `BrowserRefreshStore` 的最小 contract 面

## 影响范围

- 模块：
  - `crates/rssr-infra/Cargo.toml`
  - `crates/rssr-infra/tests/wasm_refresh_contract_harness.rs`
  - `docs/testing/contract-harness-rebuild-plan.md`
- 平台：
  - wasm32 / browser

## 关键变更

- 新增 target-specific dev dependency：
  - `wasm-bindgen-test`
- 新增测试入口：
  - `crates/rssr-infra/tests/wasm_refresh_contract_harness.rs`
- 当前覆盖：
  - `BrowserRefreshStore::list_targets`
  - `BrowserRefreshStore::commit(NotModified)`
  - `PersistedState` 到 browser localStorage 的写回
- 在测试规划文档中补了：
  - host baseline 入口
  - wasm/browser harness 编译入口
  - 建议的 browser 实际执行入口

## 已执行验证

- 待执行：
  - `cargo fmt --all`
  - `git diff --check`
  - `cargo check -p rssr-infra`
  - `cargo test -p rssr-infra --target wasm32-unknown-unknown --test wasm_refresh_contract_harness --no-run`

## 当前状态

- browser / wasm harness 基座已落地
- 目前只先保证“能编译、入口存在、最小 contract 面有测试形状”
- 尚未在真实浏览器 runner 中执行

## 风险与待跟进

- 当前仓库还没有统一的 wasm browser test runner 约定
- 如果要在 CI 里真正跑起来，还需要决定：
  - 是否引入 `wasm-pack`
  - headless browser 选 Chrome 还是 Firefox
  - browser localStorage 清理策略如何标准化

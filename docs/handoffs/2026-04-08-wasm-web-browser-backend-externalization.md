# Web Browser Backend 外移到 rssr-infra

- 日期：2026-04-08
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：61ad9cf
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

把原先仍位于 `crates/rssr-app/src/bootstrap/web` 的 browser backend 正式外移到 `crates/rssr-infra/src/application_adapters/browser`，让 `rssr-app` 的 wasm 入口继续变薄，回到“装配共享 use case + 页面绑定”的职责。

## 影响范围

- 模块：
  - `crates/rssr-infra/src/application_adapters/browser`
  - `crates/rssr-infra/src/lib.rs`
  - `crates/rssr-app/src/bootstrap/web.rs`
  - `crates/rssr-app/src/bootstrap/web/mutations.rs`
  - `crates/rssr-app/src/bootstrap/web/refresh.rs`
- 平台：
  - Web
  - desktop
  - Android

## 关键变更

### browser backend 正式进入 infra

- 新增：
  - `crates/rssr-infra/src/application_adapters/browser/mod.rs`
  - `crates/rssr-infra/src/application_adapters/browser/adapters.rs`
  - `crates/rssr-infra/src/application_adapters/browser/config.rs`
  - `crates/rssr-infra/src/application_adapters/browser/feed.rs`
  - `crates/rssr-infra/src/application_adapters/browser/query.rs`
  - `crates/rssr-infra/src/application_adapters/browser/state.rs`

### rssr-app 的 Web 入口继续变薄

- `crates/rssr-app/src/bootstrap/web.rs` 改为直接装配：
  - browser repositories
  - browser refresh source/store
  - browser config exchange adapter
- 删除旧的本地实现文件：
  - `crates/rssr-app/src/bootstrap/web/adapters.rs`
  - `crates/rssr-app/src/bootstrap/web/config.rs`
  - `crates/rssr-app/src/bootstrap/web/feed.rs`
  - `crates/rssr-app/src/bootstrap/web/query.rs`
  - `crates/rssr-app/src/bootstrap/web/state.rs`

### wasm 编译边界收紧

- `rssr-infra` 现在：
  - 只在 `wasm32` 下暴露 `application_adapters::browser`
  - 在 `wasm32` 下不再编译：
    - `db`
    - `fetch`
    - `parser`
    - `config_sync`
    - native application adapters
- `rssr-infra` 的 wasm target 依赖去掉了 `sqlx`
- `rssr-app` 的 wasm target 正式依赖 `rssr-infra`

## 验证与验收

### 自动化验证

- `cargo fmt --all`：通过
- `cargo check -p rssr-infra`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `git diff --check`：通过

### 手工验收

- 本轮未单独做新的浏览器点击回归
- 依赖上一轮 Web 回归结论，并确认本轮改动未破坏宿主端和 wasm 编译

## 结果

- Web 端 browser backend 已经不再寄居在 `rssr-app`
- `rssr-app` 的 wasm 入口更接近装配层
- `rssr-infra` 开始正式承担 browser 适配器职责

## 风险与后续事项

- `application_adapters::browser` 仍然是 browser-only backend，并不意味着 Web 已与 native 共用同一存储实现
- 下一步适合继续做：
  - Web 实际回归
  - 评估 `mutations.rs` / `refresh.rs` 是否还要进一步收进 infra 或 application

## 给下一位 Agent 的备注

- 先看：
  - `crates/rssr-infra/src/application_adapters/browser/mod.rs`
  - `crates/rssr-infra/src/application_adapters/browser/adapters.rs`
  - `crates/rssr-app/src/bootstrap/web.rs`
- 如果后续再做 Web 收束，优先保持这个原则：
  - browser 特化可以存在
  - 但它应属于 `infra adapter`
  - 不应再回流到 `rssr-app` 入口层

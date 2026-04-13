# Shared Composition Builder

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：57ef326
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

按优先级继续推进 composition/bootstrap 收口，在 `rssr-infra` 新增共享 builder，把 `rssr-app` native、`rssr-app` web 和 `rssr-cli` 的重复 use-case 装配路径统一到同一组 helper。

## 影响范围

- 模块：
  - `crates/rssr-infra/src/composition.rs`
  - `crates/rssr-infra/src/lib.rs`
  - `crates/rssr-app/src/bootstrap/native.rs`
  - `crates/rssr-app/src/bootstrap/web.rs`
  - `crates/rssr-cli/src/main.rs`
- 平台：
  - Linux
  - desktop
  - Web
  - wasm32
  - CLI
- 额外影响：
  - bootstrap / composition wiring

## 关键变更

### Infra Shared Builder

- 新增 `crates/rssr-infra/src/composition.rs`。
- 提供 native / CLI 共享装配入口：
  - `compose_native_sqlite_use_cases(pool)`
  - 返回 `NativeSqliteComposition { use_cases, entry_repository }`
- 提供 web/browser 共享装配入口：
  - `compose_browser_use_cases(state, client, clock)`
- builder 内部统一组装：
  - concrete repositories
  - app-state adapter
  - refresh source/store
  - OPML codec
  - `AppUseCases::compose(...)`

### Native / Web / CLI Call Sites

- `rssr-app/src/bootstrap/native.rs`
  - 不再手工重复创建 SQLite repos + refresh source/store + `AppUseCases::compose(...)`
  - 改为调用 `compose_native_sqlite_use_cases(pool)`
- `rssr-app/src/bootstrap/web.rs`
  - 不再手工重复创建 browser repos/adapters + `AppUseCases::compose(...)`
  - 改为调用 `compose_browser_use_cases(state, client.clone(), Arc::new(BrowserClock))`
- `rssr-cli/src/main.rs`
  - 不再重复 native SQLite 组装路径
  - 改为调用 `compose_native_sqlite_use_cases(pool).use_cases`

## 验证与验收

### 自动化验证

- `cargo test -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo test -p rssr-cli`：通过
- `cargo fmt --check`：通过
- `git diff --check`：通过

### 手工验收

- 静态代码复核：通过
- 确认 native/web/CLI 三处入口仍保留各自 host 责任：
  - native 数据库定位与迁移
  - web `load_state()` warning logging / browser clock
  - CLI 参数解析与输出
- 共享 builder 只负责 use-case 装配，没有把 UI/runtime 或 host 生命周期逻辑错误下沉到 application。

## 结果

- native、web、CLI 的 use-case 组装路径已经统一到共享 builder，不再各自复制一套 concrete adapter 装配。
- 后续调整 refresh source/store、repo wiring 或 OPML codec 时，只需要改 shared builder，而不是三处并改。

## 风险与后续事项

- 当前只统一了“use-case 装配”，没有统一：
  - native 数据库 backend 初始化
  - web `load_state()` / warning logging
  - host capability 与 runtime 行为
- 下一步更值得继续的方向：
  - 转去收 `rssr-web/src/auth.rs`
  - 或回头继续清理 application port 语义重复（如 `AppStatePort` / `FeedRemovalCleanupPort`）

## 给下一位 Agent 的备注

- 先看：
  - `crates/rssr-infra/src/composition.rs`
  - `crates/rssr-app/src/bootstrap/native.rs`
  - `crates/rssr-app/src/bootstrap/web.rs`
  - `crates/rssr-cli/src/main.rs`
- 如果继续做 composition：
  - 先判断是否还需要统一 backend/bootstrap 初始化层
  - 否则就切到下一高价值目标 `rssr-web/src/auth.rs`

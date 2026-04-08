# Web 共享 Use Case 收束第一刀

- 日期：2026-04-08
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：dc6d9b6
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

把 Web 端“添加订阅 / 删除订阅 / 刷新订阅”的主流程从 `rssr-app` 私有实现，收束为通过 `rssr-application` 的共享 use case 执行，同时保留浏览器本地状态和浏览器抓取能力作为 platform adapter。

## 影响范围

- 模块：
  - `crates/rssr-app/src/bootstrap/web`
  - `crates/rssr-application`
  - `crates/rssr-infra/src/application_adapters`
- 平台：
  - Web
  - Android
- 额外影响：
  - N/A

## 关键变更

### Web browser adapter

- 新增 `crates/rssr-app/src/bootstrap/web/adapters.rs`
- 为浏览器本地状态实现：
  - `FeedRepository`
  - `EntryRepository`
  - `AppStatePort`
  - `FeedRefreshSourcePort`
  - `RefreshStorePort`
- 保留浏览器 `localStorage` 状态模型，不引入 SQLite 到 Web

### Web 装配层收束

- `crates/rssr-app/src/bootstrap/web.rs` 现在装配：
  - `FeedService`
  - `RefreshService`
  - `SubscriptionWorkflow`
- `add_subscription` / `remove_feed` / `refresh_all` / `refresh_feed` 改为走共享 use case
- 新增与 native 对齐的 refresh outcome 处理逻辑

### 旧 Web 私有流程清理

- 删除 `mutations.rs` 里已不再使用的私有 add/remove subscription 逻辑
- 删除 `refresh.rs` 里已不再使用的私有 refresh 实现
- 自动刷新调度继续保留在 Web 装配层，但内部改为调用共享 `refresh_all`

### wasm 兼容边界

- `rssr-application` 的 `FeedRefreshSourcePort` 按目标平台切换 async-trait 的 `Send` 约束
- `RefreshService::refresh_all` 在 wasm 下固定串行执行，在 native 下继续允许并发刷新
- `rssr-app` 补充 `async-trait` 依赖

## 验证与验收

### 自动化验证

- `cargo fmt --all`：通过
- `cargo check -p rssr-application`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo check -p rssr-app --target aarch64-linux-android`：通过

### 手工验收

- Web 添加/删除/刷新订阅：未执行
- Web 配置交换回归：未执行

## 结果

- 本次交付可继续在当前分支上推进
- Web 端共享 use case 收束已经开始落地，但配置交换和查询边界尚未继续外移

## 风险与后续事项

- 浏览器 adapter 目前仍放在 `rssr-app/src/bootstrap/web`，后续可以再评估是否移入 `rssr-infra`
- 当前只收了 add/remove/refresh 主流程，`exchange.rs` 和部分 browser-state 查询仍在 Web 层
- 如果继续第二刀，优先处理 Web 配置交换对 `ImportExportService` 的接入

## 给下一位 Agent 的备注

- 入口先看 `crates/rssr-app/src/bootstrap/web.rs`
- 然后看：
  - `crates/rssr-app/src/bootstrap/web/adapters.rs`
  - `crates/rssr-application/src/refresh_service.rs`
  - `crates/rssr-application/src/subscription_workflow.rs`
- 继续推进前，建议先补一轮 Web 端手工回归：
  - 添加订阅
  - 删除订阅
  - 单个刷新 / 刷新全部

# step2a wasm refresh 外移与 contract harness

- 日期：2026-04-08
- 作者 / Agent：Codex (GPT-5)
- 分支：refactor/wasm-refresh-extraction-step2a-v2
- 当前 HEAD：194bc17
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

完成 wasm/web refresh 的 step2a 最小落地：将 web 侧 `refresh_feed`/`refresh_all` 从私有编排切到共享 `RefreshService`，并补齐 refresh contract harness（SQLite + BrowserState 双 fixture）。

## 影响范围

- 模块：
  - crates/rssr-app/src/bootstrap/web.rs
  - crates/rssr-app/src/bootstrap/web/refresh.rs
  - crates/rssr-app/src/bootstrap/web/refresh_adapter.rs
  - crates/rssr-app/Cargo.toml
  - crates/rssr-application/Cargo.toml
  - crates/rssr-infra/tests/test_refresh_contract_harness.rs
  - Cargo.lock
- 平台：
  - Web
  - Desktop/CLI（仅编译与测试回归影响，无行为改造）
- 额外影响：
  - workflow（refresh contract 测试覆盖）

## 关键变更

### web refresh 接线

- `AppServices::refresh_feed` / `refresh_all` 改为调用共享 `RefreshService`。
- `web.rs` 保持薄装配：新增 `refresh_service` 字段与 outcome 处理函数，不把 adapter 细节塞入入口。
- 保留 `client` 字段，避免改动 import/export 路径的既有实现。

### web refresh adapter

- 新增 `web/refresh_adapter.rs`：
  - `BrowserFeedRefreshSource` 实现 `FeedRefreshSourcePort`。
  - `BrowserRefreshStore` 实现 `RefreshStorePort`。
  - `localStorage` 持久化细节通过 `SnapshotWriter` 留在 adapter 边界。
- source 侧使用 `spawn_local + oneshot` 桥接 wasm 非 `Send` 抓取 future，兼容共享端口的 `Send` 约束。

### web/refresh.rs 收缩

- 删除 web 私有 `refresh_feed` / `refresh_all` 业务编排。
- `refresh.rs` 仅保留 auto-refresh 调度薄层（定时触发 `services.refresh_all()`）。

### refresh contract harness

- 新增 `crates/rssr-infra/tests/test_refresh_contract_harness.rs`。
- 同一套断言覆盖 SQLite 与 BrowserState 双 fixture，覆盖：
  - 单 feed 刷新成功
  - 单 feed 刷新失败
  - NotModified
  - refresh_all 聚合语义
- BrowserState fixture 使用纯内存状态，不依赖真实 `web_sys`。

### 构建修补（为完成 wasm 校验）

- `rssr-application` 常规依赖补充 `tokio` 的 `rt` feature（`JoinSet` 所需）。
- `rssr-app` 增加 `async-trait` 依赖，支持 web adapter 实现端口。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo test -p rssr-application`：通过（17 passed）
- `cargo test -p rssr-infra --test test_application_refresh_store_adapter`：通过（1 passed）
- `cargo test -p rssr-infra --test test_refresh_contract_harness`：通过（4 passed）
- `cargo test -p rssr-app`：通过（13 passed）
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过

### 手工验收

- Web 页面手动刷新单 feed / 刷新全部：未执行
- Web auto-refresh 定时行为：未执行

## 结果

- 当前改动可继续进入下一阶段（step2b）前的小范围评审。
- 无 schema/migration 变更；UI 结构未改；import/export、add/remove、query 路径未改。

## 风险与后续事项

- `spawn_local + oneshot` 是 wasm source 侧的兼容桥接，后续若调整 application 端口的异步约束需重新评估。
- 本轮 contract harness 已覆盖 refresh 核心语义，但不替代 Web 端 CORS/proxy 手工路径验证。

## 给下一位 Agent 的备注

- step2a 入口看 `crates/rssr-app/src/bootstrap/web.rs` 与 `crates/rssr-app/src/bootstrap/web/refresh_adapter.rs`。
- step2b 若推进 config/exchange 外移，先复用现有 application 端口风格，不要回流到 `web.rs`。

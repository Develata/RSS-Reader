# step2c wasm/web add-remove workflow 对齐

- 日期：2026-04-08
- 作者 / Agent：Codex (GPT-5)
- 分支：refactor/wasm-config-exchange-extraction-step2b
- 当前 HEAD：b470676
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

完成 step2c：将 web add/remove 私有编排迁移到共享 `FeedService` / `SubscriptionWorkflow`，并新增 SQLite + BrowserState 双 fixture 的 add/remove contract harness。

## 影响范围

- 模块：
  - crates/rssr-app/src/bootstrap/web.rs
  - crates/rssr-app/src/bootstrap/web/mutations.rs
  - crates/rssr-app/src/bootstrap/web/subscription_adapter.rs
  - crates/rssr-infra/tests/test_subscription_contract_harness.rs
- 平台：
  - Web (wasm32)
  - Desktop/Linux/macOS（回归测试编译路径）
- 额外影响：
  - 测试 workflow（新增 subscription contract harness）

## 关键变更

### web add/remove 编排外移

- 新增 `web/subscription_adapter.rs`，在 BrowserState 边界实现：
  - `FeedRepository`
  - `EntryRepository`
  - `AppStatePort`
- `web.rs` 新增 `subscription_workflow` 装配，并将：
  - `AppServices::add_subscription` 改为 `SubscriptionWorkflow::add_subscription_and_refresh`
  - `AppServices::remove_feed` 改为 `SubscriptionWorkflow::remove_subscription`

### 删除旧私有编排

- 删除 `mutations.rs` 中旧的：
  - `add_subscription`
  - `remove_feed`
- 保留 `set_read` / `set_starred` / `save_settings` / `remember_last_opened_feed_id` 等与本轮无关职责。

### add/remove contract harness

- 新增 `test_subscription_contract_harness.rs`，SQLite fixture 与 BrowserState fixture 共用同一组断言。
- 覆盖语义：
  - URL normalize
  - re-activate deleted feed
  - remove + purge entries + soft delete
  - remove 后 clear last_opened

## 验证与验收

### 自动化验证

- `cargo test -p rssr-application`：通过
- `cargo test -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo test -p rssr-infra --test test_subscription_contract_harness`：通过

### 手工验收

- Web 页面添加/删除订阅流程：未执行（本轮以 contract + 编译回归为主）
- 与真实浏览器 localStorage 交互验证：未执行（本轮未引入新的 web_sys 依赖）

## 结果

- web add/remove 已对齐共享 `FeedService` / `SubscriptionWorkflow`，并移除旧私有编排路径。
- 可进入下一阶段一致性收敛与主线冻结前的更完整回归验证。

## 风险与后续事项

- BrowserState adapter 当前针对 `list_summaries` 返回空集合；当前 web 列表展示走 query 路径，不影响本轮目标，但后续若复用该方法需补语义。
- 建议下一阶段补一轮真实浏览器手工回归，确认 UI 交互链路与 contract 语义一致。

## 给下一位 Agent 的备注

- step2c 核心入口：`crates/rssr-app/src/bootstrap/web/subscription_adapter.rs`
- contract 入口：`crates/rssr-infra/tests/test_subscription_contract_harness.rs`

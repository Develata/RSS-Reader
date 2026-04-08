# step2b wasm/web config-exchange 外移

- 日期：2026-04-08
- 作者 / Agent：Codex (GPT-5)
- 分支：refactor/wasm-config-exchange-extraction-step2b
- 当前 HEAD：b470676
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

完成 step2b：将 web/wasm 配置交换主编排迁移到共享 `ImportExportService`，并补齐 SQLite + BrowserState 双 fixture 的最小 contract harness。

## 影响范围

- 模块：
  - crates/rssr-app/src/bootstrap/web.rs
  - crates/rssr-app/src/bootstrap/web/config.rs
  - crates/rssr-app/src/bootstrap/web/exchange.rs
  - crates/rssr-app/src/bootstrap/web/exchange_adapter.rs
  - crates/rssr-infra/tests/test_config_exchange_contract_harness.rs
- 平台：
  - Web (wasm32)
  - Desktop/Linux/macOS（测试编译路径）
- 额外影响：
  - 测试 workflow（新增 config-exchange contract harness）

## 关键变更

### Web config/exchange 接线外移

- 新增 `web/exchange_adapter.rs`，提供 BrowserState 侧 `FeedRepository` / `EntryRepository` / `SettingsRepository` / `FeedRemovalCleanupPort` / `OpmlCodecPort` / `RemoteConfigStore` 适配。
- `AppServices` 新增并注入共享 `ImportExportService`，web 配置交换入口改为调用共享 service。
- `web/exchange.rs` 收缩为薄包装，仅保留 UI 入口调用、参数组装与错误翻译。

### config.rs 必要裁剪

- 删除 web 本地 `validate_config_package` 与 `import_field`，避免与共享应用层规则重复。
- 保留 `validate_settings`、`remote_url`、OPML 编解码，维持 web 侧必要边界职责。

### config-exchange contract harness

- 新增 `test_config_exchange_contract_harness.rs`。
- 同一套断言覆盖 SQLite fixture 与 BrowserState fixture：
  - JSON 导入导出 roundtrip
  - OPML 导入导出
  - remote pull 删除 feed 时的 cleanup（含 last-opened 清理）
  - 关键设置校验边界（`refresh_interval_minutes >= 1`）
- remote store 与 BrowserState snapshot writer 均为内存 stub，不依赖真实网络和 `web_sys`。

## 验证与验收

### 自动化验证

- `cargo test -p rssr-application`：通过
- `cargo test -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo test -p rssr-infra --test test_config_exchange_contract_harness`：通过

### 手工验收

- Web 页面手工导入导出路径：未执行（本轮以 contract + 编译校验为主）
- 远端 WebDAV 真实环境联调：未执行（本轮约束要求不依赖真实网络）

## 结果

- step2b 目标范围内改动已完成，且未触碰 refresh / add-remove 主路径 / query.rs / UI 结构 / schema/migrations。
- 当前交付可继续进入下一步整合。

## 风险与后续事项

- BrowserState adapter 对损坏 URL 的清理语义目前是“在 `list_feeds` 时惰性清理”，建议后续结合更完整一致性测试补充回归断言。
- remote push/pull 的真实 WebDAV 兼容性仍需在集成环境补一轮端到端验证。

## 给下一位 Agent 的备注

- 先看 `crates/rssr-app/src/bootstrap/web/exchange_adapter.rs`，这是本轮 web config-exchange 外移的核心入口。
- 合同测试入口在 `crates/rssr-infra/tests/test_config_exchange_contract_harness.rs`，下一阶段可在此基础上继续扩展 add/remove 一致性断言。

# Web 配置交换共享 Use Case 收束第二刀

- 日期：2026-04-08
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：5069bdd
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

把 Web 端的 JSON 配置导入导出、OPML 导入导出、WebDAV push/pull 从 `rssr-app` 私有实现切到共享 `ImportExportService`，同时补齐浏览器侧配置交换 adapter。

## 影响范围

- 模块：
  - `crates/rssr-app/src/bootstrap/web`
  - `crates/rssr-application/src/import_export_service`
  - `crates/rssr-infra/src/application_adapters`
- 平台：
  - Web
  - Android
- 额外影响：
  - N/A

## 关键变更

### Browser exchange adapter

- `crates/rssr-app/src/bootstrap/web/adapters.rs` 新增：
  - `BrowserSettingsRepository`
  - `BrowserOpmlCodec`
  - `BrowserRemoteConfigStore`
- `BrowserAppStateAdapter` 同时实现 `FeedRemovalCleanupPort`

### Web 配置交换装配

- `crates/rssr-app/src/bootstrap/web.rs` 新增 `ImportExportService`
- Web 的：
  - `export_config_json`
  - `import_config_json`
  - `export_opml`
  - `import_opml`
  - `push_remote_config`
  - `pull_remote_config`
  现在都走共享 service

### 旧 Web 私有交换逻辑清理

- `crates/rssr-app/src/bootstrap/web/exchange.rs` 改成薄 wrapper
- 删除原先在 Web 层直接维护的 JSON/OPML/WebDAV 主流程
- `crates/rssr-app/src/bootstrap/web/config.rs` 清理掉已不再使用的私有配置导入 helper

### wasm 兼容边界

- `RemoteConfigStore` 和 `FeedRemovalCleanupPort` 按目标平台切换 async-trait 的 `Send` 约束
- 保持浏览器 reqwest/WebDAV 路径在 wasm 下可编译

## 验证与验收

### 自动化验证

- `cargo fmt --all`：通过
- `cargo check -p rssr-application`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo check -p rssr-app --target aarch64-linux-android`：通过
- `git diff --check`：通过

### 手工验收

- Web JSON 导入导出：未执行
- Web OPML 导入导出：未执行
- WebDAV push/pull：未执行

## 结果

- Web 端配置交换主流程已与 native 共用 `ImportExportService`
- `bootstrap/web` 的私有业务编排继续变薄

## 风险与后续事项

- 目前仍缺一轮浏览器实际回归，重点是：
  - JSON 导入导出
  - OPML 导入导出
  - WebDAV push/pull
- browser adapter 仍位于 `rssr-app/src/bootstrap/web`，后续仍可评估是否正式外移到 `rssr-infra`

## 给下一位 Agent 的备注

- 入口先看：
  - `crates/rssr-app/src/bootstrap/web.rs`
  - `crates/rssr-app/src/bootstrap/web/adapters.rs`
  - `crates/rssr-app/src/bootstrap/web/exchange.rs`
- 然后看共享 service：
  - `crates/rssr-application/src/import_export_service.rs`

# application-usecases-step1-patches

- 日期：2026-04-08
- 作者 / Agent：Codex
- 分支：refactor/application-usecases-step1
- 当前 HEAD：194bc17
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

在 `application-usecases-step1` 第一刀基础上完成 3 个最小修补：导入删除路径补齐 app-state cleanup、刷新 outcome 收紧、infra application adapter 按 refresh/non-refresh 最小拆分。

## 影响范围

- 模块：
  - `crates/rssr-application`
  - `crates/rssr-infra`
  - `crates/rssr-app/src/bootstrap/native.rs`
- 平台：
  - macOS / Linux / Windows desktop
  - CLI
- 额外影响：
  - `docs/handoffs`

## 关键变更

### cleanup 语义统一

- `ImportExportService` 新增 `FeedRemovalCleanupPort`（单一窄方法）与 `new_with_feed_removal_cleanup(...)` 构造入口。
- `ImportExportService::new(...)` 保持兼容并默认 no-op cleanup（CLI/测试不需要 app-state 的入口可继续复用）。
- 导入配置删除 feed 改为统一调用 `remove_feed_with_cleanup`，remote pull 复用同一路径。
- native 装配改为注入真实 `SqliteAppStateAdapter` 到 `ImportExportService`；`SubscriptionWorkflow` 与导入服务共享同一 adapter。

### refresh outcome 收紧

- `RefreshFeedResult::Updated` 从 `entries: Vec<ParsedEntryData>` 调整为 `localization_entries: Vec<RefreshLocalizedEntry>`。
- 新增 `RefreshLocalizedEntry`，仅保留图片本地化必需字段：`dedup_key/url/title/content_html/content_text`。
- `RefreshService` 内部新增集中构造函数，仅输出含 `content_html` 的本地化候选。
- native 背景图片本地化改消费精简 DTO；CLI 行为保持不变。

### adapter 文件最小拆分

- `application_adapters.rs` 改为薄门面，仅 `mod + pub use`。
- refresh 相关实现迁到 `application_adapters/refresh.rs`。
- app-state/opml/remote-config 相关实现迁到 `application_adapters/non_refresh.rs`。

## 验证与验收

### 自动化验证

- `cargo test -p rssr-application`：通过
- `cargo test -p rssr-infra`：沙箱内除 `test_webdav_local_roundtrip` 外通过；该测试因本地端口绑定权限失败
- `cargo test -p rssr-infra --test test_webdav_local_roundtrip`（提权后）：通过
- `cargo test --workspace`（提权后）：通过
- `cargo run -p rssr-cli -- --help`：通过
- `cargo fmt`：通过

### 手工验收

- desktop 添加/删除订阅与导入远端配置交互回归：未执行
- CLI 除 `--help` 外命令级手工回归：未执行

## 结果

- 三个最小修补项已全部落地并通过自动化验证。
- 第一刀重构结果可进入冻结评审窗口，无需立即进入 wasm adapter 外移。

## 风险与后续事项

- cleanup 仍是“删除成功后再清理 app-state”的顺序语义，与现有 `remove_subscription` 一致，未引入事务化联动。
- wasm/web 路径保持原状，本次未变更。

## 给下一位 Agent 的备注

- cleanup 入口优先看 `crates/rssr-application/src/import_export_service.rs` 与 `crates/rssr-infra/src/application_adapters/non_refresh.rs`。
- refresh outcome 变更优先看 `crates/rssr-application/src/refresh_service.rs` 与 `crates/rssr-app/src/bootstrap/native.rs`。
- adapter 结构入口为 `crates/rssr-infra/src/application_adapters.rs`。

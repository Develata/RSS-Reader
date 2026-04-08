# application-usecases-step1

- 日期：2026-04-07
- 作者 / Agent：Codex
- 分支：refactor/application-usecases-step1
- 当前 HEAD：194bc17
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

完成第一刀 application use case 收束：把 native / CLI 中重复的刷新、订阅管理、配置交换主流程收进 `rssr-application`，并保留 native 专属正文图片本地化在入口层后处理。

## 影响范围

- 模块：
  - `crates/rssr-application`
  - `crates/rssr-infra`
  - `crates/rssr-cli`
  - `crates/rssr-app/src/bootstrap/native.rs`
- 平台：
  - macOS / Linux / Windows desktop
  - CLI
- 额外影响：
  - `docs/handoffs`

## 关键变更

### application use case

- 扩展 `FeedService`，让其负责添加订阅 URL 规范化、删除订阅和可选文章清理。
- 新增 `RefreshService`，统一单 feed / 全量刷新流程，并返回显式 outcome。
- 新增 `SubscriptionWorkflow`，只负责 add+optional refresh 与 remove+app-state cleanup 编排。
- 扩展 `ImportExportService`，统一 JSON 配置交换、OPML 导入导出和远端配置 push/pull。

### infra adapter

- 新增 `application_adapters.rs`，落地 `FeedRefreshSourcePort`、`RefreshStorePort`、`AppStatePort`、`OpmlCodecPort` 和 `RemoteConfigStore` 的 infra 实现。
- 新增 refresh store adapter 集成测试，验证刷新落库会写入 feed metadata、fetch state 和 entries。

### 入口收薄

- `rssr-cli` 删除自有 refresh / OPML / WebDAV / remove cleanup 主流程，改为只调 application service / workflow。
- native `bootstrap/native.rs` 删除重复 refresh / OPML / WebDAV / last_opened_feed_id cleanup 主流程，只保留正文图片本地化后处理和桌面端装配。

## 验证与验收

### 自动化验证

- `cargo test -p rssr-application`：通过
- `cargo test -p rssr-infra`：沙箱内仅 `test_webdav_local_roundtrip` 因本地端口绑定受限失败；其余通过
- `cargo test -p rssr-infra --test test_webdav_local_roundtrip`（提权后）：通过
- `cargo test -p rssr-cli --no-run`：通过
- `cargo test -p rssr-app --no-run`：通过
- `cargo test --workspace`（提权后）：通过
- `cargo run -p rssr-cli -- --help`：通过
- `cargo fmt`：通过

### 手工验收

- 桌面端 `rssr-app` 手工验证添加订阅 / 删除订阅 / 刷新 / 导入导出：未执行
- CLI 手工命令级回归（除 `--help` 外）：未执行

## 结果

- 第一刀重构已完成到“可继续集成 / 可继续手工回归”的状态。
- native / CLI 的核心刷新、订阅管理、配置交换主流程已收束进 application；wasm/web 仍保留原实现。

## 风险与后续事项

- `bootstrap/web/*` 仍保留旧的本地状态实现与刷新 / 配置交换逻辑，本轮刻意未动。
- `RefreshService` 的 outcome 现在会携带更新条目，主要用于 native 正文图片本地化；后续如果 wasm 也接这层，可能需要再审视 outcome 形状。
- workspace 全量测试在默认沙箱里仍会受本地 WebDAV 回环测试影响；完整验证需要允许本地端口绑定。

## 给下一位 Agent 的备注

- 继续推进时先看 `crates/rssr-application/src/refresh_service.rs`、`crates/rssr-application/src/subscription_workflow.rs`、`crates/rssr-infra/src/application_adapters.rs`。
- 如果下一轮开始处理 wasm adapter 外移，优先复用本次新增的 application port，不要把 web 刷新逻辑再直接抄回入口层。

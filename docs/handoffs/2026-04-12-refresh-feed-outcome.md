# Refresh Feed Outcome

- 日期：2026-04-12
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：e14dc35
- 相关 commit：e14dc35
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

把 feeds 页单订阅刷新从纯 error/ok 改成结构化执行结果，避免 feed-level 刷新失败已写回 `fetch_error` 后 UI 不 reload。

## 影响范围

- 模块：
  - `crates/rssr-app/src/bootstrap.rs`
  - `crates/rssr-app/src/bootstrap/native.rs`
  - `crates/rssr-app/src/bootstrap/web.rs`
  - `crates/rssr-app/src/ui/runtime/services.rs`
  - `crates/rssr-app/src/ui/runtime/feeds.rs`
- 平台：
  - Linux
  - Web
  - wasm32 / native single-feed refresh path
- 额外影响：
  - release UI regression 记录：`target/release-ui-regression/20260412-codex-refresh-feed-outcome/summary.md`

## 关键变更

### Structured Host Outcome

- 新增 `RefreshFeedExecutionOutcome { failure_message: Option<String> }`
- `RefreshPort::refresh_feed()` 的返回值从 `anyhow::Result<()>` 改为 `anyhow::Result<RefreshFeedExecutionOutcome>`

### Native / Web Bootstrap

- feed-level refresh failure 不再被 host 层升级为 error
- native / web 都保留失败日志，并返回 `failure_message`
- native 仍在 updated outcome 上触发 image localization worker

### UI Runtime

- feeds runtime 在单订阅刷新返回后无论是否存在 feed-level failure，都会发送 `BumpReload`
- 失败时显示 `刷新订阅失败：...` 且 tone 为 `error`
- fatal error 仍走 `Err` 分支，不 reload

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo test --workspace`：通过
- `git diff --check`：通过
- `bash scripts/run_release_ui_regression.sh --no-serve --with-rssr-web --port 8359 --web-port 18859 --log-dir target/release-ui-regression/20260412-codex-refresh-feed-outcome`：通过

### 手工验收

- 未执行独立手工 UI 点击验收；本次依赖 workspace 自动化和 release UI 门禁覆盖 single-feed refresh / browser smoke 路径。

## 结果

- 本次交付可合并；单订阅刷新失败后页面会 reload，以展示已写回的错误状态。
- `rssr-web browser feed smoke` 本轮通过。

## 风险与后续事项

- 这次只改变 host capability 到 UI runtime 的结果表达，不改变 application refresh service 的提交语义。
- 如果继续做 feeds 页治理，下一步更适合审查 remove/import 的确认态是否还能更集中表达，而不是继续改 refresh 结果。

## 给下一位 Agent 的备注

- 阅读入口：`crates/rssr-app/src/bootstrap.rs`、`crates/rssr-app/src/ui/runtime/feeds.rs`
- `RefreshAllExecutionOutcome` 和 `RefreshFeedExecutionOutcome` 是并列的 host execution outcome，区分 fatal error 与 feed-level failure。

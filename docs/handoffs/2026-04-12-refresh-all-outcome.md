# Refresh All Outcome

- 日期：2026-04-12
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：4c08402
- 相关 commit：4c08402
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

把 feeds 页“全部刷新”从纯 `Result<()>` 改成结构化执行结果，避免部分订阅刷新失败时吞掉已经成功写回的数据 reload。

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
  - wasm32 / native refresh-all path
- 额外影响：
  - release UI regression 记录：`target/release-ui-regression/20260412-codex-refresh-all-outcome/summary.md`

## 关键变更

### Structured Host Outcome

- 新增 `RefreshAllExecutionOutcome { failure_message: Option<String> }`
- `RefreshPort::refresh_all()` 的返回值从 `anyhow::Result<()>` 改为 `anyhow::Result<RefreshAllExecutionOutcome>`

### Native / Web Bootstrap

- native / web 的 `handle_refresh_all_outcome()` 仍保留逐 feed 日志输出
- 不再把“部分订阅刷新失败”升级成 host-level error
- 改为返回：
  - `failure_message = None`：全部成功或未变化
  - `failure_message = Some(...)`：部分失败，但刷新流程已完成且可能已有成功写回

### UI Runtime

- feeds runtime 在 `RefreshAll` 成功返回后无论是否存在部分失败，都会发送 `BumpReload`
- 当存在部分失败时，状态消息改为：
  - `刷新完成，但部分订阅失败：...`
  - tone 为 `error`
- 只有真正的 fatal error（例如 capability / service 调用失败）才走 `刷新失败：...` 且不 reload

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo test --workspace`：通过
- `git diff --check`：通过
- `bash scripts/run_release_ui_regression.sh --no-serve --with-rssr-web --port 8358 --web-port 18858 --log-dir target/release-ui-regression/20260412-codex-refresh-all-outcome`：通过

### 手工验收

- 未执行独立手工 UI 点击验收；本次依赖 workspace 自动化和 release UI 门禁覆盖 refresh-all / browser smoke 路径。

## 结果

- 本次交付可合并；feeds 页在部分订阅刷新失败时不会丢失已成功写回的数据 reload。
- `rssr-web browser feed smoke` 本轮通过。

## 风险与后续事项

- 这次只修正 `refresh_all` 的结果表达和 UI reload 策略，单 feed refresh 仍保持现有语义。
- 若后续继续做 feeds 页治理，可考虑把 refresh/import/remove 的页面级结果消息进一步结构化，而不是继续由 runtime 手工拼文案。

## 给下一位 Agent 的备注

- 阅读入口：`crates/rssr-app/src/bootstrap.rs`、`crates/rssr-app/src/ui/runtime/feeds.rs`
- 若继续沿结果建模方向推进，优先看哪些 runtime 分支还会因为 host 结果过于扁平而丢失“部分成功”的信息。

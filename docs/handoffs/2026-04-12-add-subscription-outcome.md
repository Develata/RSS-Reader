# Add Subscription Outcome

- 日期：2026-04-12
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：f40664c
- 相关 commit：f40664c
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

把 feeds 页“订阅已保存但首次刷新失败”的判定从字符串匹配改成结构化 outcome，移除 runtime 对错误文案的隐式协议依赖。

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
  - wasm32 / native add subscription bootstrap path
- 额外影响：
  - release UI regression 记录：`target/release-ui-regression/20260412-codex-add-subscription-outcome/summary.md`

## 关键变更

### Structured Host Outcome

- 新增 `AddSubscriptionOutcome`：
  - `SavedAndRefreshed`
  - `SavedRefreshFailed { message }`
- `RefreshPort::add_subscription()` 的返回值从 `anyhow::Result<()>` 改为 `anyhow::Result<AddSubscriptionOutcome>`。

### Native / Web Bootstrap

- native 和 web 的 `RefreshCapability::add_subscription()` 现在：
  - 订阅保存失败时返回 error
  - 首次刷新成功时返回 `SavedAndRefreshed`
  - 首次刷新失败但订阅已保存时返回 `SavedRefreshFailed { message }`
- 不再把“已保存但首刷失败”编码成 `"首次刷新订阅失败"` 字符串前缀。

### UI Runtime

- `FeedsPort::add_subscription()` 改为返回 `AddSubscriptionOutcome`
- feeds runtime 删除：
  - `err.to_string().contains("首次刷新订阅失败")`
- 改为按结构化 outcome 渲染成功/部分失败状态消息和 reload intent

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo test --workspace`：通过
- `git diff --check`：通过
- `bash scripts/run_release_ui_regression.sh --no-serve --with-rssr-web --port 8357 --web-port 18857 --log-dir target/release-ui-regression/20260412-codex-add-subscription-outcome`：通过

### 手工验收

- 未执行独立手工 UI 点击验收；本次依赖 workspace 自动化和 release UI 门禁覆盖 add subscription / browser smoke 路径。

## 结果

- 本次交付可合并；feeds add flow 不再依赖错误字符串协议。
- `rssr-web browser feed smoke` 本轮通过。

## 风险与后续事项

- 这次清理的是 host capability 与 runtime 之间的字符串耦合，不涉及 refresh service 或 subscription workflow 的核心语义。
- feeds runtime 仍承载较多页面文案和确认态逻辑；后续若继续收敛，优先看 import/export/remove 的页面 workflow，而不是回到字符串协议。

## 给下一位 Agent 的备注

- 阅读入口：`crates/rssr-app/src/bootstrap.rs`、`crates/rssr-app/src/ui/runtime/feeds.rs`
- 若继续做 `ui/runtime` 横向治理，优先找类似“通过 error string 推断业务语义”的点；这类问题的优先级高于继续增加形式上的 service 包装。

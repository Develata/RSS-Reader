# Settings Sync Service

- 日期：2026-04-12
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：1263ac8
- 相关 commit：1263ac8
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

把 settings 页面里“pull remote config 后再 reload settings”的组合流程收敛到 application 层，减少 UI runtime 对 host capability 和 settings service 的交错编排。

## 影响范围

- 模块：
  - `crates/rssr-application/src/settings_sync_service.rs`
  - `crates/rssr-application/src/lib.rs`
  - `crates/rssr-application/src/composition.rs`
  - `crates/rssr-app/src/ui/runtime/services.rs`
  - `crates/rssr-app/src/ui/runtime/settings.rs`
- 平台：
  - Linux
  - Web
  - wasm32 browser local state / remote config path
- 额外影响：
  - release UI regression 记录：`target/release-ui-regression/20260412-codex-settings-sync-service/summary.md`

## 关键变更

### Application Use Case

- 新增 `SettingsSyncService`，统一承接 `RemoteConfigPullOutcome` 的后续处理。
- 新增 `AppliedRemoteConfigOutcome`，用 `NotFound` / `Imported { import, settings }` 明确表达导入后是否需要重建 settings 快照。
- `AppUseCases` 注入 `settings_sync_service`，使 settings sync 成为 application 层可组合能力。

### UI Runtime Boundary

- `SettingsPort` 新增 `pull_remote_config_and_load_settings()`，内部先调用 host remote config capability，再调用 `settings_sync_service.apply_remote_pull()`。
- settings runtime 删除本地 `pull -> if found -> load_settings` 编排，只保留 outcome 到页面 intent / 文案的映射。
- `PushConfig`、普通 `Load`、`SaveAppearance` 路径保持不变。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo check -p rssr-application`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo test -p rssr-application`：通过
- `cargo fmt --check`：通过
- `git diff --check`：通过
- `cargo check -p rssr-cli`：通过
- `cargo test --workspace`：通过
- `bash scripts/run_release_ui_regression.sh --no-serve --with-rssr-web --port 8354 --web-port 18854 --log-dir target/release-ui-regression/20260412-codex-settings-sync-service`：通过

### 手工验收

- 未执行独立手工 UI 点击验收；本次依赖 workspace 合同测试和 release UI 自动门禁覆盖 remote config / browser smoke 路径。

## 结果

- 本次交付可合并；settings 页面不再在 UI runtime 手工组合 remote pull 和 settings reload。
- `rssr-web browser feed smoke` 本轮通过，未复现超时。

## 风险与后续事项

- 远端下载能力本身仍在 host capability，当前收敛的是 pull 后状态恢复，不是整条 remote config 基础设施。
- `cargo test` 期间仍有既有 `test_browser_state_seed_contracts` fixture dead_code warning。
- 如果继续清理 settings 相关边界，下一步更适合审查 push/pull 文案、错误分类和 endpoint/path 校验是否应继续下沉。

## 给下一位 Agent 的备注

- 阅读入口：`crates/rssr-application/src/settings_sync_service.rs` 和 `crates/rssr-app/src/ui/runtime/settings.rs`。
- 如果要继续沿这条线推进，优先看 `RemoteConfigPort` 是否值得演化为 application port，而不是回退到 UI runtime 编排。

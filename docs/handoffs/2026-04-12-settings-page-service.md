# Settings Page Service

- 日期：2026-04-12
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：60c831f
- 相关 commit：60c831f
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

把 settings 页的读取、保存和远端 pull 后设置恢复统一收敛到 application 层 `SettingsPageService`，减少 UI runtime 对通用 `SettingsService` / `SettingsSyncService` 的直接编排。

## 影响范围

- 模块：
  - `crates/rssr-application/src/settings_page_service.rs`
  - `crates/rssr-application/src/lib.rs`
  - `crates/rssr-application/src/composition.rs`
  - `crates/rssr-app/src/ui/runtime/services.rs`
- 平台：
  - Linux
  - Web
  - wasm32 settings page / remote config path
- 额外影响：
  - release UI regression 记录：`target/release-ui-regression/20260412-codex-settings-page-service/summary.md`

## 关键变更

### Application Use Case

- 新增 `SettingsPageService`。
- 新增：
  - `SettingsPageSnapshot`
  - `SaveSettingsAppearanceOutcome`
- `SettingsPageService` 统一承接：
  - `load()`
  - `save_appearance()`
  - `apply_remote_pull()`

### Composition Boundary

- `AppUseCases` 注入 `settings_page_service`。
- `SettingsPageService` 基于现有 `SettingsService` 和 `SettingsSyncService` 组合，不改底层设置校验规则，也不改 remote pull 的导入语义。

### UI Runtime Boundary

- `SettingsPort::load_settings()` 改为走 `settings_page_service.load()`
- `SettingsPort::save_settings()` 改为走 `settings_page_service.save_appearance()`
- `SettingsPort::pull_remote_config_and_load_settings()` 改为走 `settings_page_service.apply_remote_pull()`

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo check -p rssr-application`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo fmt --check`：通过
- `git diff --check`：通过
- `cargo test -p rssr-application`：通过
- `cargo test --workspace`：通过
- `bash scripts/run_release_ui_regression.sh --no-serve --with-rssr-web --port 8356 --web-port 18856 --log-dir target/release-ui-regression/20260412-codex-settings-page-service`：通过

### 手工验收

- 未执行独立手工 UI 点击验收；本次依赖 application 单测、workspace 自动化和 release UI 门禁覆盖 settings / remote config / browser smoke 路径。

## 结果

- 本次交付可合并；settings 页入口现在是统一的页面级 application service。
- `rssr-web browser feed smoke` 本轮通过。

## 风险与后续事项

- 本次收敛的是 settings 页面 use case 边界，不涉及 host remote config capability 下沉。
- `settings_service` 与 `settings_sync_service` 仍保留为更底层能力；当前没有继续压缩它们到单一服务，以免把通用能力和页面能力重新混回去。
- 如果继续推进壳层/application 收敛，下一步更适合检查 feeds 页或 shell/settings 之外是否仍有跨 service 组合残留。

## 给下一位 Agent 的备注

- 阅读入口：`crates/rssr-application/src/settings_page_service.rs`、`crates/rssr-app/src/ui/runtime/services.rs`
- 如果要继续做边界审查，优先看 `ui/runtime` 中是否还有“拿两个以上 use case/host capability 组合成一个页面结果”的点。

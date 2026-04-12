# Shell Service

- 日期：2026-04-12
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：e82faf0
- 相关 commit：e82faf0
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

把 authenticated shell 初始化里的设置加载收敛到 application 层 `ShellService`，减少 UI runtime 对 `settings_service` 的直接编排；同时清理 `Navigator` 的无意义 `clone`。

## 影响范围

- 模块：
  - `crates/rssr-application/src/shell_service.rs`
  - `crates/rssr-application/src/lib.rs`
  - `crates/rssr-application/src/composition.rs`
  - `crates/rssr-app/src/ui/runtime/services.rs`
  - `crates/rssr-app/src/ui/runtime/shell.rs`
  - `crates/rssr-app/src/ui/shell.rs`
- 平台：
  - Linux
  - Web
  - wasm32 browser shell bootstrap path
- 额外影响：
  - release UI regression 记录：`target/release-ui-regression/20260412-codex-shell-service/summary.md`

## 关键变更

### Application Use Case

- 新增 `ShellService`，统一承接 authenticated shell 所需的 settings 快照读取。
- 新增 `AuthenticatedShellSnapshot`，明确 shell 初始化 use case 的返回结构。
- `AppUseCases` 注入 `shell_service`，让 shell 初始化不再直接依赖通用 `settings_service`。

### UI Runtime Boundary

- `ShellPort` 把 `load_settings()` 收敛为 `load_authenticated_shell()`。
- shell runtime 删除对通用 settings 读取的直通依赖，只保留：
  - 加载 authenticated shell 快照
  - 启动 host auto refresh capability
  - 映射成 `UiIntent::AuthenticatedShellLoaded`

### Warning Cleanup

- `crates/rssr-app/src/ui/shell.rs` 去掉 `Navigator` 的无意义 `clone()`，清理既有 `clippy` warning。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo check -p rssr-application`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo fmt --check`：通过
- `git diff --check`：通过
- `cargo clippy --workspace --all-targets`：通过，仍有既有 `test_browser_state_seed_contracts` fixture dead_code warning
- `cargo test --workspace`：通过
- `bash scripts/run_release_ui_regression.sh --no-serve --with-rssr-web --port 8355 --web-port 18855 --log-dir target/release-ui-regression/20260412-codex-shell-service`：通过

### 手工验收

- 未执行独立手工 UI 点击验收；本次依赖 workspace 自动化和 release UI 门禁覆盖 shell/bootstrap/web smoke 路径。

## 结果

- 本次交付可合并；shell 入口开始走页面级 application use case。
- `rssr-web browser feed smoke` 本轮通过。

## 风险与后续事项

- auto refresh 启动仍在 host capability 层，这是合理的环境能力边界；当前收敛的是 shell 设置快照读取，不是整条 shell 启动流程。
- `SettingsPort::load_settings` / `save_settings` 仍是直通 `SettingsService`；如果继续沿页面 use case 收敛，下一步更适合审查 settings 页面是否需要完整 bootstrap/save outcome，而不是继续包单个 getter。
- `cargo clippy` 仍剩 browser state seed fixture 的 dead_code warning。

## 给下一位 Agent 的备注

- 阅读入口：`crates/rssr-application/src/shell_service.rs`、`crates/rssr-app/src/ui/runtime/shell.rs`。
- 如果继续推进“页面/壳层 use case 化”，优先看 settings save/load 是否值得合并成稳定的 page workflow，而不是回退到跨 service 编排。

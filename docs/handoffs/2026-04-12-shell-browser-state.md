# Shell Browser State

- 日期：2026-04-12
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：81f96bf
- 相关 commit：81f96bf
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

审查 `ui/shell.rs` 的浏览器 API 使用后，确认其属于 shell 表现层状态与登录完成后的浏览器跳转，并拆出独立 helper，降低 shell 主文件职责混杂。

## 影响范围

- 模块：
  - `crates/rssr-app/src/ui/shell_browser.rs`
  - `crates/rssr-app/src/ui/shell.rs`
  - `crates/rssr-app/src/ui/mod.rs`
- 平台：
  - Linux
  - Web
  - shell navigation / search / web auth transition path
- 额外影响：
  - release UI regression 记录：`target/release-ui-regression/20260412-codex-shell-browser-state/summary.md`

## 关键变更

### Boundary Decision

- `rssr-entry-search` 和 `rssr-nav-hidden` 是 shell 表现层记忆，不进入 application use case 或 host capability。
- 登录完成后的 `window.location().reload()` 是 Web auth 交互收尾行为，仍留在 UI shell 侧，但移出 shell 主状态文件。

### File Split

- 新增 `shell_browser.rs`，集中放置 shell 浏览器 helper：
  - `initial_entry_search()`
  - `remember_entry_search()`
  - `initial_nav_hidden()`
  - `remember_nav_hidden()`
  - `complete_web_auth_transition()`
- `shell.rs` 保留 shell state、nav facade、auth gate shell 和 runtime bus 逻辑。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo test -p rssr-app`：通过，22 个 app 测试通过
- `cargo test --workspace`：通过
- `git diff --check`：通过
- `bash scripts/run_release_ui_regression.sh --no-serve --with-rssr-web --port 8364 --web-port 18864 --log-dir target/release-ui-regression/20260412-codex-shell-browser-state`：通过

### 手工验收

- 未执行独立手工 shell 搜索、导航折叠或 Web auth 登录验收；本次依赖 native/wasm 编译、workspace 自动化和 release UI 门禁覆盖。

## 结果

- 本次交付可合并；`ui/shell.rs` 不再直接承载 localStorage 与 Web reload helper 实现。
- `rssr-web browser feed smoke` 本轮通过，未复现超时。

## 风险与后续事项

- 这次是行为保持型拆分，没有改变 shell 搜索、导航折叠或登录完成后 reload 语义。
- 后续可继续审查 `pages/settings_page/themes/theme_io.rs`，该文件包含主题导入导出的浏览器文件 API；初步判断它属于 settings/theme UI 交互能力，适合先拆 page helper，而不是进入 application service。

## 给下一位 Agent 的备注

- 阅读入口：`crates/rssr-app/src/ui/shell_browser.rs` 与 `crates/rssr-app/src/ui/shell.rs`
- 当前判断：shell 搜索与导航折叠记忆是 UI shell 表现状态，不是核心应用状态。

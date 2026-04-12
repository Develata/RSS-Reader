# Theme File IO

- 日期：2026-04-12
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：00691aa
- 相关 commit：00691aa
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

审查 settings/theme 的文件导入导出逻辑后，确认其属于主题设置页交互能力，并把具体浏览器 / rfd 文件 API 从页面 orchestration 中拆出。

## 影响范围

- 模块：
  - `crates/rssr-app/src/pages/settings_page/themes/theme_file_io.rs`
  - `crates/rssr-app/src/pages/settings_page/themes/theme_io.rs`
  - `crates/rssr-app/src/pages/settings_page/themes/lab.rs`
  - `crates/rssr-app/src/pages/settings_page/themes/mod.rs`
- 平台：
  - Linux
  - Web
  - Android compile path for unsupported native file picker / saver branches
- 额外影响：
  - release UI regression 记录：`target/release-ui-regression/20260412-codex-theme-file-io/summary.md`

## 关键变更

### Boundary Decision

- 主题 CSS 文件导入/导出是 settings/theme UI 交互能力，不进入 application service。
- 具体文件 API，包括 Web `web_sys`/Blob/download anchor 和 native `rfd::AsyncFileDialog`，集中放入 page-local helper。

### File Split

- 新增 `theme_file_io.rs`，承载：
  - native CSS 文件读取
  - Web 隐藏文件输入触发
  - native CSS 文件保存
  - Web CSS Blob 下载
- `theme_io.rs` 保留页面状态反馈、异步 spawn、导入后应用 CSS、导出结果提示。
- `lab.rs` 的 Web 文件选择触发改为直接引用 `theme_file_io::trigger_css_file_input_in_browser()`。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo test -p rssr-app`：通过，22 个 app 测试通过
- `cargo test --workspace`：通过
- `git diff --check`：通过
- `bash scripts/run_release_ui_regression.sh --no-serve --with-rssr-web --port 8365 --web-port 18865 --log-dir target/release-ui-regression/20260412-codex-theme-file-io`：通过

### 手工验收

- 未执行独立手工主题文件导入/导出验收；本次依赖 native/wasm 编译、workspace 自动化和 release UI 门禁覆盖。

## 结果

- 本次交付可合并；主题文件 API 绑定被收敛到 page-local file IO helper，页面 orchestration 更清晰。
- `rssr-web browser feed smoke` 本轮通过，未复现超时。

## 风险与后续事项

- 这次是行为保持型拆分，没有改变主题 CSS 校验、应用、保存或导出文件名。
- 后续若要进一步治理 settings 页，可审查 remote sync session 的确认态生命周期是否与 feeds 页确认态规则一致。

## 给下一位 Agent 的备注

- 阅读入口：`crates/rssr-app/src/pages/settings_page/themes/theme_file_io.rs` 与 `crates/rssr-app/src/pages/settings_page/themes/theme_io.rs`
- 当前判断：主题文件导入/导出不属于核心 application use case；它是 settings/theme 页面交互能力。

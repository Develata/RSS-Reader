# feeds page paste fallback

- 日期：2026-04-09
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：4d0d270
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`implemented`

## 工作摘要

把 `zheye-mainline-stabilization` 中仍有价值的“新增订阅输入框粘贴兜底”按当前主线结构手动移植到订阅页输入框。

## 影响范围

- 模块：
  - `crates/rssr-app/src/pages/feeds_page_sections/compose.rs`
- 平台：
  - Web
  - desktop
  - Android

## 关键变更

- 为 `feed-url-input` 新增 `Cmd/Ctrl+V` 最小兜底。
- 命中粘贴快捷键时，优先通过 `navigator.clipboard.readText()` 读取剪贴板文本。
- 成功读取后直接写回 `feed_url` signal。
- 读取失败时，不引回旧的页面直调模式，而是继续通过 `FeedsPageBindings::set_status_error(...)` 显示错误状态。
- 若平台不提供 `navigator.clipboard.readText()`，则保持静默回退，不阻断现有默认输入行为。

## 验证与验收

### 自动化验证

- `cargo fmt --all`
- `cargo check -p rssr-app`
- `cargo check -p rssr-app --target wasm32-unknown-unknown`
- `cargo check -p rssr-app --target aarch64-linux-android`
- `git diff --check`

### 手工验收

- 待执行：
  - desktop 订阅输入框 `Cmd/Ctrl+V` 粘贴可回填
  - Web 订阅输入框 `Cmd/Ctrl+V` 不引入新的 console error

## 当前状态

- 代码已落地，待本轮验证通过后提交。

## 风险与待跟进

- 当前兜底只覆盖新增订阅输入框，不扩展到所有输入控件。
- 某些浏览器或 WebView 若不开放剪贴板权限，将静默退回原生输入行为；后续如需更强的一致性，可再评估是否抽共享输入策略。

## 相关文件

- `crates/rssr-app/src/pages/feeds_page_sections/compose.rs`


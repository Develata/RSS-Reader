# Entry Browser Interactions

- 日期：2026-04-12
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：e7e8bf3
- 相关 commit：e7e8bf3
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

审查 entries 页残留的浏览器 / DOM 交互代码后，确认其属于交互壳层行为，并从 controls 渲染文件拆出独立 helper，减少组件文件职责混杂。

## 影响范围

- 模块：
  - `crates/rssr-app/src/pages/entries_page/browser_interactions.rs`
  - `crates/rssr-app/src/pages/entries_page/controls.rs`
  - `crates/rssr-app/src/pages/entries_page/mod.rs`
  - `crates/rssr-app/src/pages/entries_page/session.rs`
- 平台：
  - Linux
  - Web
  - entries page controls / directory navigation path
- 额外影响：
  - release UI regression 记录：`target/release-ui-regression/20260412-codex-entry-browser-interactions/summary.md`

## 关键变更

### Boundary Decision

- `scroll_to_entry_group()` 是目录按钮触发的视图滚动行为，属于 entries 页交互壳层，不迁入 application service 或 host capability。
- `initial_entry_controls_hidden()` / `remember_entry_controls_hidden()` 是页面控件折叠状态的浏览器本地表现记忆，不进入应用核心状态模型。

### File Split

- 新增 `browser_interactions.rs`，集中放置 entries 页浏览器 / DOM helper：
  - 控件折叠初始值读取
  - 控件折叠状态记忆
  - 目录锚点滚动
- `controls.rs` 保留组件渲染和事件绑定。
- `session.rs` 从 `browser_interactions` 调用折叠状态记忆。
- `mod.rs` 从 `browser_interactions` 读取初始折叠状态。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo test -p rssr-app`：通过，22 个 app 测试通过
- `cargo test --workspace`：通过
- `git diff --check`：通过
- `bash scripts/run_release_ui_regression.sh --no-serve --with-rssr-web --port 8363 --web-port 18863 --log-dir target/release-ui-regression/20260412-codex-entry-browser-interactions`：通过

### 手工验收

- 未执行独立手工目录滚动点击验收；本次依赖 native/wasm 编译、workspace 自动化和 release UI 门禁覆盖。

## 结果

- 本次交付可合并；entries controls 渲染文件不再直接承载 localStorage 与 DOM scroll helper 实现。
- `rssr-web browser feed smoke` 本轮通过，未复现超时。

## 风险与后续事项

- 这次是行为保持型拆分，没有改变目录滚动脚本或控件折叠记忆语义。
- 后续如果继续审查直接浏览器 API，建议优先区分“交互壳层表现行为”和“平台能力”。前者可留在页面 helper，后者才应走 host capability。

## 给下一位 Agent 的备注

- 阅读入口：`crates/rssr-app/src/pages/entries_page/browser_interactions.rs`
- 当前判断：entries 目录滚动不是 application use case，也不是 runtime service；它是页面交互壳层 helper。

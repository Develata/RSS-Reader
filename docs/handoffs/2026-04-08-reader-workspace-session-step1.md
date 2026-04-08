# 2026-04-08 Reader Workspace Session Step 1

## Summary
- 将阅读页重构为局部 workspace/session 结构。
- 为阅读页补齐 `state`、`intent`、`reducer`、`bindings`、`effect`、`runtime` 与 support helper。
- 让文章加载、已读切换、收藏切换统一走 `effect -> runtime -> intent -> reducer -> state` 链路。
- 清理文章页旧的命令/分发残留文件，避免项目内同时存在两套过时入口。

## Modules
- `crates/rssr-app/src/pages/reader_page.rs`
- `crates/rssr-app/src/pages/reader_page_state.rs`
- `crates/rssr-app/src/pages/reader_page_intent.rs`
- `crates/rssr-app/src/pages/reader_page_reducer.rs`
- `crates/rssr-app/src/pages/reader_page_bindings.rs`
- `crates/rssr-app/src/pages/reader_page_effect.rs`
- `crates/rssr-app/src/pages/reader_page_runtime.rs`
- `crates/rssr-app/src/pages/reader_page_support.rs`
- `crates/rssr-app/src/hooks/use_reader_shortcuts.rs`
- `crates/rssr-app/src/pages.rs`

## What Changed
- 阅读页页面壳改为围绕单一 `ReaderPageState` 渲染。
- 原先直接在页面与快捷键里调用 service 的逻辑，收进 `ReaderPageEffect` 与 `execute_reader_page_effect(...)`。
- `use_reader_shortcuts` 现在消费阅读页 state/bindings，而不是直接操纵页面局部信号。
- 阅读页正文选择、HTML 清洗、时间格式化等 support helper 从主页面文件抽离。
- 删除 `entries_page_commands.rs` 与 `entries_page_dispatch.rs` 这两份已脱离主链路的旧文件。

## Acceptance
- `cargo fmt --all`
- `cargo check -p rssr-app`
- `cargo check -p rssr-app --target wasm32-unknown-unknown`
- `cargo check -p rssr-app --target aarch64-linux-android`
- `git diff --check`
- Chrome MCP 回归：
  - 打开阅读页成功
  - 底部栏“已读/未读”切换成功
  - 底部栏“收藏”切换成功
  - 快捷键 `F` 切换收藏成功
  - console 无新增 `error/warn`

## Notes
- 当前阅读页已经具备局部 session 骨架，但还没有显式 `reader_page_session` 壳类型。
- 如果继续推进 headless active interface，下一步更自然的是评估是否把阅读页初始化与导航行为继续收束到显式 session 层。

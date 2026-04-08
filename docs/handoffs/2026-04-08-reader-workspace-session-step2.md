# 2026-04-08 Reader Workspace Session Step 2

## Summary
- 为阅读页补上显式 `ReaderPageSession` 壳层。
- 页面组件与快捷键不再直接调用 runtime，而是统一经由 session 发起加载与动作。
- 阅读页现在具备更稳定的局部工作台入口，便于后续继续沿 headless active interface 推进。

## Modules
- `crates/rssr-app/src/pages/reader_page.rs`
- `crates/rssr-app/src/pages/reader_page_session.rs`
- `crates/rssr-app/src/hooks/use_reader_shortcuts.rs`
- `crates/rssr-app/src/pages.rs`

## What Changed
- 新增 `ReaderPageSession`，集中承担读取快照、加载当前文章、切换已读、切换收藏、推导前后导航目标。
- `ReaderPage` 现在通过 session 获取 snapshot、reload tick 与底部栏动作，而不是直接拼 runtime 调用。
- `use_reader_shortcuts` 现在只依赖 `ReaderPageSession`，不再直接依赖 `ReaderPageEffect` / `ReaderPageBindings` / `execute_reader_page_effect(...)`。

## Acceptance
- `cargo fmt --all`
- `cargo check -p rssr-app`
- `cargo check -p rssr-app --target wasm32-unknown-unknown`
- `cargo check -p rssr-app --target aarch64-linux-android`
- `git diff --check`
- Chrome MCP 回归：
  - 阅读页打开正常
  - 快捷键 `M` 切换已读正常
  - 底部栏按钮状态正常更新
  - console 无新增 `error/warn`

## Notes
- 到这一步，阅读页已经具备：
  - `state`
  - `intent`
  - `reducer`
  - `bindings`
  - `effect`
  - `runtime`
  - `session`
- 再继续在阅读页内部加层，边际收益已经开始下降。
- 更自然的下一步是评估是否把同样的局部 workspace/session 模式迁移到设置页，而不是继续细拆阅读页内部结构。

# 2026-04-08 Entries Session Explicitization

## Summary

- 为文章页新增显式 `EntriesPageSession`。
- 页面壳、控制区和文章卡片动作现在都围绕 session 工作。
- 文章页继续从“局部 workspace 组件集合”推进到“显式局部 session 壳”。

## Why

- 上一轮盘点确认文章页已经具备 `state / intent / reducer / queries / effect / runtime / presenter`，但仍缺一个显式 session 入口。
- 页面壳里还散着多条 `use_resource/use_effect` 驱动链和局部动作接线。
- 如果不补 session，文章页虽然模块已拆散，但还不算真正完成 phase 1 收口。

## Changes

### Added

- `crates/rssr-app/src/pages/entries_page_session.rs`

### Updated

- `crates/rssr-app/src/pages/entries_page.rs`
- `crates/rssr-app/src/pages/entries_page_controls.rs`
- `crates/rssr-app/src/pages/entries_page_cards.rs`
- `crates/rssr-app/src/pages.rs`

## What Moved Into Session

- 记住当前订阅作用域
- 加载文章页偏好
- 加载订阅列表
- 加载文章列表
- 保存文章页浏览偏好
- 切换已读
- 切换收藏
- 派发本地 `EntriesPageIntent`

## Resulting Boundary

- `entries_page.rs` 更接近 view shell：
  - 只负责路由作用域、布局、`use_resource/use_effect` 挂载和渲染
- `EntriesPageSession` 成为页面壳的唯一动作入口
- 控制区与卡片区不再各自直连 reducer/runtime

## Verification

- `cargo fmt --all`
- `cargo check -p rssr-app`
- `cargo check -p rssr-app --target wasm32-unknown-unknown`
- `cargo check -p rssr-app --target aarch64-linux-android`
- `git diff --check`

## Follow-up

- 文章页的 `use_resource/use_effect` 现在已经全部围绕 session 工作。
- 如果继续推进 headless 主线，下一步应审视是否要把页面壳的资源触发再进一步收进一个 session-owned hook，而不是继续拆更多 helper。

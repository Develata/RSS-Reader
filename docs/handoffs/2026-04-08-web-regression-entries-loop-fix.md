# Web 回归阻塞修复：文章页资源循环

- 日期：2026-04-08
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：7f648bc
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

在 Chrome MCP 回归中定位到一个真实前端阻塞：进入文章页后，`EntriesPage` 的文章加载 resource 同时依赖 `feeds()` 又在内部执行 `feeds.set(...)`，导致浏览器主线程持续重跑，表现为页面进入后 Chrome MCP 的截图、快照、脚本执行全部超时。

## 影响范围

- 模块：
  - `crates/rssr-app/src/pages/entries_page.rs`
- 平台：
  - Web
  - desktop
  - Android
- 额外影响：
  - 文章页首屏加载稳定性

## 关键变更

### 拆开订阅列表与文章列表加载

- 新增单独的 `use_resource` 负责：
  - `list_feeds`
  - 更新 `feeds`
- 原先的文章列表 `use_resource` 只负责：
  - `list_entries`
  - 更新 `entries`
  - 更新状态文案

### 消除自触发重跑

- 删除“文章列表 resource 内部再次 `feeds.set(...)`”这条路径
- 保留 `feeds()` 作为筛选映射输入，但不再由同一 resource 回写，避免持续重跑

## 验证与验收

### 自动化验证

- `cargo fmt --all`：pending
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：pending

### 手工验收

- Chrome MCP 打开文章页：pending
- Chrome MCP 截图 / 快照恢复：pending

## 结果

- 已修复一处明确的页面主线程自激活循环
- 这条问题足以解释进入 app 后 Chrome MCP 全面超时的现象

## 风险与后续事项

- 修复后仍需重新打 Web bundle 并做一次真实回归
- 如果 Chrome MCP 仍超时，再继续检查：
  - `SettingsPage` / `FeedsPage` 是否存在类似 signal-resource 自触发
  - `AppServices::shared()` 之后的初始化是否还有隐性重复保存

## 给下一位 Agent 的备注

- 先从 `crates/rssr-app/src/pages/entries_page.rs` 看这次修复
- 然后重新做 Web 浏览器回归，优先确认：
  - 文章页能正常进入
  - Chrome MCP 能恢复截图 / 快照 /脚本执行

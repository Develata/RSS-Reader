# 文章页工作台内核第一阶段

- 日期：2026-04-08
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：8972474
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

把文章页从“多组 signal/resource/effect 直接堆在页面主文件里”的形态推进到局部工作台内核第一阶段：新增状态、意图、规约器、presenter、查询、effect 与 runtime 模块，并让主页面更接近 view shell。

## 影响范围

- 模块：
  - `crates/rssr-app/src/pages/entries_page.rs`
  - `crates/rssr-app/src/pages/entries_page_bindings.rs`
  - `crates/rssr-app/src/pages/entries_page_controls.rs`
  - `crates/rssr-app/src/pages/entries_page_state.rs`
  - `crates/rssr-app/src/pages/entries_page_intent.rs`
  - `crates/rssr-app/src/pages/entries_page_reducer.rs`
  - `crates/rssr-app/src/pages/entries_page_presenter.rs`
  - `crates/rssr-app/src/pages/entries_page_queries.rs`
  - `crates/rssr-app/src/pages/entries_page_effect.rs`
  - `crates/rssr-app/src/pages/entries_page_runtime.rs`
  - `crates/rssr-app/src/pages.rs`
- 平台：
  - Web
  - Windows / macOS / Linux desktop
  - Android
- 额外影响：
  - docs

## 关键变更

### 状态与意图层

- 新增 `EntriesPageState`，统一承载文章页原始状态：
  - entries / feeds
  - filters
  - grouping
  - controls hidden
  - directory expansion
  - status
  - reload tick
- 新增 `EntriesPageIntent`，统一表达本地状态变化与加载结果。
- 新增 `entries_page_reducer`，通过 `dispatch_entries_page_intent(...)` 统一更新页面状态。

### Presenter 与查询层

- 新增 `EntriesPagePresenter`，把页面派生逻辑从主文件移出：
  - visible entries
  - archived count
  - source filter options
  - grouped entry trees
  - directory models
  - top nav items
- 新增 `entries_page_queries`，把页面主文件里的服务查询抽成显式函数：
  - `remember_last_opened_feed`
  - `load_entries_page_preferences`
  - `load_entries_page_feeds`
  - `load_entries_page_entries`

### Effect 与 runtime

- 新增 `EntriesPageEffect`，把文章页副作用统一建模为显式 effect：
  - 加载偏好
  - 加载订阅
  - 加载文章
  - 记住最后打开订阅
  - 标记已读
  - 切换收藏
  - 保存浏览偏好
- 新增 `entries_page_runtime`，统一执行 effect，并把结果回灌为页面 intent。
- 原有 `entries_page_commands.rs` / `entries_page_dispatch.rs` 已移除，卡片动作和偏好保存改走 effect/runtime。

### 页面壳与绑定

- `entries_page.rs` 改为围绕单一 `Signal<EntriesPageState>` 组装页面。
- `EntriesPageBindings` 不再直接持有多个独立 signal，而是统一把 runtime 结果写回页面状态。
- `entries_page_controls.rs` 不再直接依赖多组信号；交互改为通过 intent 驱动状态变化。
- 目录展开/收起也进入同一条 reducer 流。

## 验证与验收

### 自动化验证

- `cargo fmt --all`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo check -p rssr-app --target aarch64-linux-android`：通过
- `git diff --check`：通过

### 手工验收

- Chrome MCP：阅读页返回文章页：通过
- Chrome MCP：文章页默认收起控制区、目录正常显示：通过
- Chrome MCP：展开筛选与组织：通过
- Chrome MCP：点击“标已读”后卡片状态更新：通过
- Chrome MCP：effect/runtime 改造后文章页 Console 无新增 error / warn：通过
- Chrome MCP：Console 无新增 error / warn：通过

## 结果

- 本次交付可继续推进，不需要回滚。
- 文章页现在已经具备局部工作台内核的完整基本骨架：state / intent / reducer / presenter / effect / runtime。

## 风险与后续事项

- 当前异步加载仍然由 `use_resource` 触发，但其副作用已经统一收敛到 effect/runtime；下一步如果继续推进，可以再评估是否把触发层也抽成更明确的 session 壳。
- 页面本地状态变化目前是 “intent -> reducer”，副作用是 “spawn/use_resource -> effect/runtime -> intent”；这已经清楚，但还不是最极致的单入口 session 对象。
- 下一步最值得继续做的是：
  - 评估是否需要引入 `entries_page_session`，进一步统一触发层
  - 再决定是否把阅读页也按同样模式收成工作台内核

## 给下一位 Agent 的备注

- 先看：
  - `crates/rssr-app/src/pages/entries_page.rs`
  - `crates/rssr-app/src/pages/entries_page_state.rs`
  - `crates/rssr-app/src/pages/entries_page_presenter.rs`
  - `crates/rssr-app/src/pages/entries_page_reducer.rs`
- 这轮没有修改用户当前打开的 `docs/design/headless-active-interface.md`，如果工作区里它仍显示为 modified，需要先确认那是不是用户本地未提交改动。

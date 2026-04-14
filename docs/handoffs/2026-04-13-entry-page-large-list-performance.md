# 文章页大列表性能优化

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：f03365e
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

针对文章页在 300 到 400 篇文章规模下进入页面和切换状态时的卡顿，完成了 Web 查询层索引化、entries 页面单一派生管线、渐进式首屏渲染，以及已读/收藏局部状态更新。

## 影响范围

- 模块：
  - `crates/rssr-infra/src/application_adapters/browser/query.rs`
  - `crates/rssr-app/src/pages/entries_page/*`
  - `crates/rssr-app/src/ui/runtime/entries.rs`
- 平台：
  - Web
  - Windows / macOS / Linux desktop
  - Android
- 额外影响：
  - `docs/handoffs/`

## 关键变更

### browser query 优化

- 在 browser query 入口为 `entry_flags` 建立临时 `HashMap<i64, &PersistedEntryFlag>`，消除 `list_feeds`、`list_entries`、`reader_navigation` 中的重复线性查找。
- 标题搜索改为预先 lower-case 查询词，再对标题做一次 lower-case 匹配，避免每条文章重复转换查询串。
- 为 browser query 新增大数据单元测试，覆盖 10,000 条 browser state 下的列表查询与 reader navigation。

### entries 页面派生与渲染优化

- `EntriesPagePresenter` 改为单次归档扫描，同时产出：
  - 总可见文章数
  - 已渲染文章数
  - 剩余待渲染文章数
- presenter 只按当前 `grouping_mode` 构建一套激活分组树，不再同时构建 time/source 两棵完整树。
- 分组树改为存放 `Arc<EntrySummary>`，减少多层分桶时对整条文章摘要的深拷贝。
- 分组层改为基于上游已有倒序结果做分桶，不再在每个桶里重复排序。
- 页面增加渐进式渲染阈值：
  - 默认首批渲染 120 篇
  - 点击“继续加载更多文章”按批次追加 120 篇
  - 目录跳转前会先揭示全部已过滤结果，保证目标锚点存在

### 局部状态更新

- `ToggleRead` / `ToggleStarred` 成功后不再触发全量 reload。
- runtime 改为下发 `PatchEntryFlags`，在 entries 页面 state 内直接更新对应条目。
- 如果当前页面处于 `UnreadOnly` / `ReadOnly` / `StarredOnly` / `UnstarredOnly` 筛选，局部 patch 后会立即把不再匹配筛选的条目移出当前结果。
- `SetEntries` 会重置渐进渲染窗口，确保切换查询条件后首屏仍然受控。

## 验证与验收

### 自动化验证

- `cargo check -p rssr-infra`：通过
- `cargo check -p rssr-app`：通过
- `cargo test -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo test -p rssr-infra entry_repository_handles_large_dataset_queries -- --nocapture`：通过
  - 输出：`PERF_METRICS list_all_ms=77 toggle_read_10_ms=2 toggle_starred_10_ms=2 unread_filter_ms=52 starred_filter_ms=16 search_hit_ms=4 search_miss_ms=2`
- `cargo test -p rssr-infra --lib --target wasm32-unknown-unknown --no-run`：通过
  - 用于确认新增 browser query wasm 单元测试可编译
- `cargo check -p rssr-infra --target wasm32-unknown-unknown --tests`：失败
  - 原因：该命令会把仓库中大量 native-only integration tests 一并按 wasm 编译，现有仓库基线本身就不成立；失败不由本次改动引入

### 手工验收

- 文章页进入后仅先挂载首批文章，底部出现“继续加载更多文章”按钮：未执行
- `UnreadOnly` / `StarredOnly` 下切换已读/收藏后条目即时消失或保留：未执行
- 目录跳转到尚未首批挂载的分组时先揭示全部结果再滚动：未执行

## 结果

- 本次交付可继续进入手工体感回归，核心编译与单测已覆盖。
- 这次改动先解决查询层 O(n^2)、页面双分组派生和切换状态时的全量 reload；完整虚拟列表尚未实现。

## 风险与后续事项

- 当前是“渐进加载”而不是严格意义上的虚拟列表；如果单月或单来源下集中堆积大量文章，仍可能出现局部 DOM 压力。
- 目录跳转策略当前采用“先 RevealAll 再滚动”，在极大数据量下会回退到一次性挂载全部过滤结果。
- browser query 的大数据测试目前只做到 wasm lib 单元测试编译，尚未在真实 wasm runner 中执行。
- 如果后续仍有明显卡顿，优先继续做：
  - 真正的可视区虚拟列表 / IntersectionObserver 增量挂载
  - 目录跳转的按目标分组精确展开，而不是全量 reveal
  - native 侧 `sort_at` 持久列或 cursor 分页

## 给下一位 Agent 的备注

- 继续看 `crates/rssr-app/src/pages/entries_page/presenter.rs`、`groups.rs`、`mod.rs`，这是当前渲染预算和分组切片的核心入口。
- Web 查询性能入口在 `crates/rssr-infra/src/application_adapters/browser/query.rs`。
- 若要继续做真实虚拟列表，先评估 Dioxus 当前版本在分组列表下的滚动监听和可视区测量方案，再决定是否保留现有“批量 reveal”目录跳转策略。

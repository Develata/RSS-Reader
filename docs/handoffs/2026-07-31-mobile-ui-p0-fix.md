# 移动端 UI P0 修复：极端内容溢出、按钮粘连、分页样式

- 日期：2026-07-31
- 作者 / Agent：WorkBuddy (Kimi-K3)
- 分支：main
- 当前 HEAD：7c7a03d
- 相关 commit：7c7a03d（fix）；docs commit 见后续 log
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

按 `docs/mobile-ui-audit-2026-07.md` 的 P0 批次实施纯增量 CSS 修复，消除移动端三个 live 缺陷：长 token 标题吹胀列表、删除/刷新按钮 0px 粘连、分页无样式。39 行新增、0 行删除。

## 影响范围

- 模块：
  - `assets/styles/workspaces.css`（+14 行）
  - `assets/styles/entries.css`（+25 行）
- 平台：
  - Android / 移动 Web 为主修复面；桌面端同步获益（feed-card-actions 按钮间距、分页间距）
- 额外影响：
  - N/A

## 关键变更

### 溢出防护（audit P0-1，对应 claude-code #1）

- `[data-layout="feed-list"]` / `[data-layout="entry-list"]`：补 `grid-template-columns: minmax(0, 1fr)`，列轨不再按最宽卡片 max-content 吹胀。
- `[data-slot="feed-card-title"]` / `[data-slot="entry-card-title"]`：补 `min-width: 0; overflow-wrap: anywhere`（与 reader-body / feed-card-url 既有 guard 对齐）。
- 实测验收取代过程中发现同类未防护容器并一并补齐（均为同形增量）：
  - `[data-slot="entry-group-title"]` / `[data-slot="entry-group-meta"]`（分组标题可显示订阅名）；
  - `[data-layout="entry-filters-source-chip"]`（筛选面板订阅源 chip，补 `min-width:0; max-width:100%; overflow-wrap:anywhere`）；
  - `[data-slot="feed-card-meta"]` / `[data-slot="entry-card-meta"]`（卡片 meta 行显示订阅名/错误信息）。

### 按钮粘连（audit P0-2，对应 claude-code #2）

- 新增 `[data-layout="feed-card-actions"]`：`display: flex; flex-wrap: wrap; gap: 12px; margin-top: 14px`，完全镜像既有 `entry-card-actions` 待遇。

### 分页样式（audit P0-3，对应 claude-code #6）

- 新增 `[data-layout="entry-pagination"]`（grid, gap 10px, margin 16px 0）、`[data-layout="entry-pagination-actions"]`（flex, align center, gap 10px 14px）、`[data-slot="entry-pagination-status"]`（muted, 0.92rem）。

## 验证与验收

### 自动化验证

- `cargo test -p rssr-app`：通过（67 unit + 2 theme contract + 2 token contract，全绿）
- `dx build --platform web --package rssr-app`：通过

### 手工验收（Chrome DevTools MCP，360×800 mobile/touch，注入极端标题数据）

- /entries：`scrollWidth` 709px → **360px**；标已读/收藏按钮 688px 飞出 → **318.7px 完全在视口内**；hashtag 标题折行 3 行：通过
- /feeds：`scrollWidth` 631px → **360px**；刷新/删除垂直间距 0px → **12px**；hashtag 订阅标题在胶囊内折行：通过
- 分页段间间隙 0/0.1px → **14px**：通过
- 内联截图确认（修复前后对比）：通过

## 结果

- 可合并；不改变任何正常内容的渲染（`overflow-wrap: anywhere` 仅在需要时断词），桌面端仅按钮/分页间距改善。
- 用户可见：移动端列表在任意标题内容下不再横向溢出，删除按钮与刷新按钮有明确间隔。

## 风险与后续事项

- 剩余已知缺口（audit 报告 P1/P2，未动）：`[data-nav]` 作用域收敛、小触控目标提升、触屏键盘提示、概览卡分级、non-wasm 控件持久化、safe-area、Amethyst Glass 定位。
- 观察项（out-of-scope，未验证因果）：滚动时顶部 sticky 导航的半透明区域会让下方文字隐约透出，如需处理归入 P1 视觉批。
- 建议把本次实测断言（360×800 + 极端标题注入 → scrollWidth=360、按钮间距 ≥10px）固化进 `scripts/run_static_web_small_viewport_smoke.sh`。

## 给下一位 Agent 的备注

- 入口：`docs/mobile-ui-audit-2026-07.md`（审计）+ 本文件（P0 实施）。
- 极端值复现方法：localStorage `rssr-web-state-v1` 注入长 token 标题/订阅名（详见审计报告附录）。
- 文本节点溢出对 `getBoundingClientRect` 扫描不可见，需用 `scrollWidth > clientWidth` 递归定位——本次 card meta 就是这样漏网又被抓到的。

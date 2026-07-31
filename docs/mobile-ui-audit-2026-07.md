# RSS-Reader 移动端 UI 审计（Vibe-Coding 七类「丑」框架）

- 日期：2026-07-31
- 审计基线：commit `c397826`（工作树干净），debug web 构建（`dx build --platform web`）
- 性质：audit-only，未修改任何代码
- 前置输入：Develata 提供的「Vibe Coding 为什么丑」7 类框架讲解 + claude-code 的 7 项移动端审计结论

## 结论

1. claude-code 的 7 项发现在 360×800 Android 模拟环境下**全部复现**：6 项为 live 缺陷，1 项（safe-area）为 latent，将在 targetSdk 升至 35 时激活。
2. 按 7 类框架归因，移动端问题集中于 **04 脆弱感**（容器未验证极端值）与 **06 各论各的**（`[data-nav]` 作用域失控导致同一组件族两种形态）；另新发现 5 项框架内问题（键盘提示泄漏、概览卡等权、订阅列表沉底、主题动词堆叠、紫玻璃预设）。
3. 修复建议分三批：P0 为纯增量 CSS（无回归面），P1 为 `[data-nav]` 作用域收敛（可见修正，需决策），P2 为信息组织与平台治理。

## 验证环境与方法

- 服务：`scripts/run_web_spa_regression_server.sh --debug --skip-build`（注意：Windows 下必须设 `RSSR_REPO_ROOT=E:\gitclone\RSS-Reader`，否则 `__codex` helper 因 Git Bash 路径直译报 `E:\e\gitclone\...` 找不到 fixture）。
- 数据：`__codex/setup-local-auth?seed=reader-demo`，另向 localStorage 注入 6 条极端标题条目（base64 串 / merge commit hash / 德语复合词 / hashtag / 两条正常中文标题）与 `entries_page_size=3` 以强制分页。
- 浏览器：Chrome DevTools MCP，视口 360×800、mobile、touch，UA 为 Pixel 8 / Android 14。
- 取证方式：全部几何数据为 `getBoundingClientRect` + `getComputedStyle` 实测；截图经 MCP 内联确认（MCP 的 workspace roots 限制导致截图无法落盘，不影响数据效力）。
- 现场清理：注入的订阅标题与主题修改已恢复；测试条目仅存于一次性浏览器 profile 的 localStorage。

## A. claude-code 七项清单复验

| # | 发现 | 状态 | 本次实测数据（360×800） | 框架归类 |
|---|------|------|--------------------------|----------|
| 1 | entry-list 溢出传染 | ✅ live 复现 | hashtag 标题 max-content 681px → grid 列 688px → `innerWidth`/`docScrollW` 被撑至 709px；正常中文标题与 688px 宽的「标已读/收藏」按钮一并被拖出屏幕 | 04 脆弱感 |
| 2 | 删除/刷新按钮 0px 粘连 | ✅ live 复现 | `feed-card-actions`: `display:block; gap:normal`；刷新底 1903.3px / 删除顶 1903.4px，间距恰为 0 | 04 脆弱感 + 触控安全 |
| 3 | `.button`+`[data-nav]` 冲突压小触控目标 | ✅ live 复现 | 返回上一页/同订阅上下篇：36px、14.08px；导航链接 36px；收起 × 28×28；展开控件钮 28px；归档复选框 13×13。对照组纯 `.button` 为 44–45.3px | 06 各论各的 + 触控 |
| 4 | 订阅卡片标题小于自身 URL | ✅ live 复现 | 标题 14.08px + 999px 胶囊边框（继承 `[data-nav]`），URL 14.72px；截图确认标题渲染为 chip | 06 各论各的 |
| 5 | `viewport-fit=cover` 无 safe-area | ✅ latent 确认 | live DOM 中存在两个 viewport meta（index.html 无 viewport-fit；app.rs:53 注入含 `viewport-fit=cover`）；全部样式表 0 处 `env(safe-area-inset-*)`；reader-bottom-bar `bottom:10px`。`target_sdk=34` 时未激活 | 03 跳脱感（平台规范） |
| 6 | 分页无样式 | ✅ live 复现 | 分页容器 `display:block; gap:normal`；「上一页(84×45.3) → 第 1 / 2 页 → 下一页」间隙实测 0 / 0.1px；顶部与底部各重复一份 | 06 各论各的 |
| 7 | 展开控件垂直成本 + 持久化 no-op | ✅ live 复现（略差） | 展开后首条标题 y=1837px = **2.30 屏**（claude-code 测 1.96 屏）；概览 344px（4 张等权卡）+ 筛选 401px + 组织栏 220px；`remember_entry_controls_hidden`/`initial_entry_controls_hidden` 经源码确认在 non-wasm 为 no-op/恒 true（browser_interactions.rs），Android 原生端每次启动需重新展开 | 05 信息缺组织 |

### 对第 1 项的补充（新证据）

同一缺口不止影响文章标题：将订阅标题改为 hashtag 长串后，**/feeds 布局被撑至 631px**（`feed-card-title` 572.5px）。根因相同——`workspaces.css:215` 的 `[data-slot="feed-card-title"], [data-slot="entry-card-title"]` 均无 `min-width:0`/`overflow-wrap` 防护，而 `[data-slot="feed-card-url"]`（workspaces.css:254）与 `[data-layout="reader-body"]`（reader.css:50）早已有。两个列表的 grid 隐式单列按 max-content 撑宽，一处极端值传染全部同列卡片。

### 对第 2 项的缓解说明

删除订阅有内联两步确认（`feeds_page/facade.rs` 的 `is_delete_pending_for` → `data-state="confirm"`），误触不会立即生效；但 0px 粘连在触屏上仍是明确的误触诱因，且 destructive 按钮位于下沿，是单手握持拇指热区。

## B. 按七类框架的新发现

| 类别 | 发现 | 严重度 |
|------|------|--------|
| 01 诡异感（违背习惯） | 阅读页底部栏在触屏设备显示键盘快捷键提示「已读（M）」「收藏（F）」——桌面中心主义泄漏，Android 无 M/F 键 | 低 |
| 03 跳脱感（原生规范） | **健康项**：全 app 无自创弹窗，删除采用内联两步确认而非自定义 modal | — |
| 05 信息缺组织 | (a) 概览四卡等权：「当前结果 6 / 每页数量 3 / 归档文章 0 / 当前组织 按时间」——后两者是控件状态复述，与核心指标同权同形；(b) /feeds 移动端顺序：新增订阅 → 配置包 JSON 输入区 → OPML 输入区 → **已保存订阅沉底**（首卡标题 y≈1635px，约 2 屏），最高频的列表被两个大输入区压在最后 | 中 |
| 06 各论各的 | 主题管理三套并行 UI（预设下拉 / 快捷按钮 / 画廊卡），共 7 个动词（载入所选主题、导入主题文件、应用当前 CSS、导出当前 CSS、使用这套主题、移除这套主题、清空 CSS），语义重叠、命名不一 | 低-中 |
| 07 纯丑 | 内置预设 **Amethyst Glass** 即博主案例原型：`#8b5cf6→#6d28d9` 紫渐变按钮、`#e0c3fc→#8ec5fc` 视口渐变、玻璃拟态、全文件 16 处渐变。opt-in 非 live 缺陷，实现也算克制（blur≤12px、reduced-motion 支持），但与项目自身 design-taste 准则冲突 | 低（治理项） |

## C. 修复优先级建议

**P0 — 纯增量 CSS，无回归面（对应 claude-code 的 1/2/6）：**

1. `[data-slot="entry-card-title"]`、`[data-slot="feed-card-title"]` 补 `min-width: 0; overflow-wrap: anywhere;`，与 reader-body 的既有防护对齐；`[data-layout="entry-list"]`/`[data-layout="feed-list"]` 的列轨显式 `minmax(0, 1fr)` 或给卡片 `min-width:0`。
2. 补写 `[data-layout="feed-card-actions"]` 规则：`display:grid; gap:10px; margin-top:12px`，对齐 `entry-card-actions` 既有待遇。
3. 分页容器改为 grid/flex + gap，并与列表加分隔（margin/border-top）。

**P1 — 可见修正，需 Develata 决策（对应 3/4，含新发现）：**

4. `[data-nav]` 作用域收敛（如限定到 `[data-layout="app-nav-links"]` 内或拆独立 class），停止命中 `.button` 与卡片标题；返回/同订阅导航恢复 44px token 尺寸。
5. 触控目标提升：归档复选框整行 label 可点、收起 ×、展开控件钮 ≥44px（Material 建议 48dp）。
6. 触屏设备隐藏键盘快捷键提示（`@media (hover: none)` 或 `(pointer: coarse)`）。

**P2 — 信息组织与平台治理：**

7. 概览卡分级：「当前结果」作主卡，「每页数量/当前组织」降级为一行 meta 文本；/feeds 已保存订阅上移，或配置交换两个输入区默认折叠。
8. non-wasm 端为 `remember_entry_controls_hidden` 补持久化（设置项或平台存储）。
9. Amethyst Glass 定位复议（降级为实验室主题或移除）；升 targetSdk 35 前补 `env(safe-area-inset-*)`，重点 reader-bottom-bar。

## 附注

- 移动端 console 干净：仅有预期内的开发环境噪音（example.com 种子源的 CORS、dx dev-server WebSocket）。
- 桌面端布局吸收了部分问题（如宽视口下 grid 吹胀不明显），移动端是该 UI 的最弱面；建议后续把 360×800 纳入 UI 回归的固定视口（`scripts/run_static_web_small_viewport_smoke.sh` 已有雏形，可把本报告的实测断言固化进去）。

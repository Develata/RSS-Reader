# 7 类「丑」框架 UI 审计与确认项修复（移动端优先）

- 日期：2026-08-01
- 作者 / Agent：opencode (k3-256k)
- 分支：main
- 当前实现 commit：`48b15e4e8549ffe6e0cc9c4eed707d9eeb00f55c`
- 相关 commit：`48b15e4e8549ffe6e0cc9c4eed707d9eeb00f55c`
- 相关 tag / release：`v0.1.14`（已发布）
- 状态：`released in v0.1.14`

## 工作摘要

按「Vibe Coding 7 类丑」框架（诡异感 / 出乎意料 / 跳脱感 / 脆弱感 / 信息缺组织 / 各论各的 / 纯丑）对全应用做了一轮移动端优先的实测审计（dx web + Chrome 360×800/touch/dpr3 仿真），确认 5 项缺陷并全部修复；其余候选项实测后证伪或降级为记录项。

## 影响范围

- 模块：
  - `assets/styles/`（reader / shell / entries / workspaces 四个样式面）
  - `crates/rssr-app/src/pages/feeds_page/sections/`（saved、config_exchange）
  - `crates/rssr-app/src/pages/settings_page/`（preferences、sync）
- 平台：
  - Web / Windows / macOS / Android（样式与页面层变更，四端同构生效；实测宿主为 Web 仿真）
- 额外影响：
  - 本审计的浏览器实测方法见「给下一位 Agent 的备注」

## 审计方法

- 宿主：`dx serve --platform web -p rssr-app`（端口 8899）+ Chrome DevTools 设备仿真 360×800、dpr3、mobile、touch；桌面抽查 1280×800。
- 种子数据：按 `rssr-infra/.../browser/state/models.rs` 的序列化结构直接向 localStorage 注入（键 `rssr-web-state-v1` / `-entry-content-v1` / `-entry-flags-v1` / `-app-state-v2`），含 3 订阅 12 文章，覆盖上批实测的 5 类极端标题（base64 / merge-hash / 德语复合词 / hashtag / URL 标题）+ 超长订阅名 + 故障源 + 宽表格正文；`entries_page_size` 设为 5 以触发分页。
- 判定：截图 + computed-style/boundingBox 几何双取证；溢出判定用 `documentElement.clientWidth` 与逐元素宽度比对（**不能**用 `innerWidth`/`scrollWidth` 互减，移动仿真下二者同源失真）。
- 主题：浅色/深色双模式对比度抽测（muted 文本：浅色 5.68:1、深色 9.37:1，均 ≥4.5:1）。

## 七类审计结论（确认项 → 修复；证伪/记录项 → 见风险节）

| 类 | 确认缺陷 | 实测证据 | 处置 |
|---|---|---|---|
| 04 脆弱感 | 阅读页标题无溢出保护 | base64 标题把整页撑到 scrollWidth 1244（360 视口），唯一超宽元素即 `reader-title`（1223px, overflow-wrap:normal） | 已修 F1 |
| 01/03 确认反馈断裂 | 三处二次确认的说明文案只出现在页顶 banner | 点第 3 张卡「删除订阅」：banner absTop 460、按钮 absTop 2120，banner 距视口 -1283px 不可见 | 已修 F2 |
| 04 脆弱感 | `stat-card-value` / `entry-overview-value` / `reader-meta` 无 overflow 保护 | CSS 级确认（实测值小未触发），与 F1 同构 | 已修 F3 |
| 03 跳脱感（触控） | `.icon-link-button` 40×40 < 44px 触控规范 | 设置页页头实测 40×40 | 已修 F4 |
| 02/07 纯丑 | 字号缩放 min 属性 f32 伪影 | spinbutton valuemin 实测渲染 `0.800000011920929`（dioxus 属性插值经 f64） | 已修 F5 |

证伪项（实测通过，未动）：文章列表极端标题换行（上批 P0 修复有效，0 溢出）、卡片操作钮 45.3px/16px、feed 卡按钮间距 12px、分页按钮 45.3px+10px 间距、底部栏触屏隐藏快捷键提示（display:none 生效）、宽表格正文（td 继承 reader-body 的 anywhere 换行）、阅读页底栏 safe-area 补偿、概览卡分级（primary 整行 78px）、feeds 页章节顺序、设置页输入控件 52.7–54px/16px。

## 关键变更

### F1 阅读页标题/元信息溢出防护（`reader.css`）

- `[data-slot="reader-title"]` 与 `[data-slot="reader-meta"]` 增加 `overflow-wrap: anywhere`，对齐 `workspaces.css:220` 卡片标题的既有防护。
- 实测 before→after：标题 1223px→319px（4 行换行），页面 scrollWidth 1244→360；桌面 1280 抽查同标题 983px 容器内换行，无回归。

### F2 二次确认内联提示（新槽位 `confirm-hint`）

- 新增 `[data-slot="confirm-hint"]` 样式（danger 色、0.9rem、anywhere 换行，`workspaces.css`）。
- 三处 pending 状态在**操作按钮旁**渲染内联提示，页顶 banner 保留不动：
  - 删除订阅（`saved.rs`）：「再次点击「确认删除」将删除订阅：{title}」，实测提示—按钮距离 1660px→71px。
  - 覆盖导入配置（`config_exchange.rs`），实测距离 10px。
  - WebDAV 下载覆盖（`sync/mod.rs`），实测距离 24px。
- 不采用「自动滚动到 banner」方案：会把用户带离确认按钮，比现状更差。

### F3 数值槽位溢出防护（`shell.css` / `entries.css`）

- `[data-slot="stat-card-value"]`、`[data-slot="entry-overview-value"]` 增加 `overflow-wrap: anywhere`（additive，实测值小未触发，防极端计数）。

### F4 触控目标（`shell.css`）

- `.icon-link-button` 40×40 → 44×44；桌面/移动端同规则，桌面抽查无回归。

### F5 字号缩放 f32 属性伪影（`preferences.rs`）

- `min`/`max`/`value` 改为 `format!(...)` 预格式化（Rust f32 Display 走最短往返表示），避免 dioxus 属性插值经 f64 展开精度。
- 实测 before→after：min `0.800000011920929`→`0.8`，max/value 正常。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo clippy --workspace --all-targets`：通过（0 警告）
- `cargo test --workspace`：通过（rssr-app 68+2+2，其余 crate 全绿，exit=0）
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- dx web 热重建：通过（浏览器实测即基于修复后构建）

### 手工验收

- 360×800/touch/dpr3 逐项复测 F1–F5 before→after：通过（数据见上）
- 深色/浅色双主题关键页 + 对比度抽测：通过
- 桌面 1280×800 抽查（阅读页、设置页、图标钮、目录 rail、快捷键提示显示）：通过
- console error：0（本轮实测期间无应用层错误；feed 刷新失败 WARN 为 dev server 无代理的预期行为）

## 结果

- 已随 `v0.1.14` 发布；实现 commit：`48b15e4e8549ffe6e0cc9c4eed707d9eeb00f55c`，tag commit：`e0ed5844604404fb27bfc80ff50641d5220f4b09`。
- 用户影响：阅读页不再被单条极端标题拖入横向滚动；三处破坏性二次确认在按钮旁可见；设置页数值属性干净。

## 风险与后续事项

- **C15 主题管理三套动词/路径**：`v0.1.14` 已完成保守方案，所有应用入口收敛为“应用”语义；激进撤掉快捷按钮行仍未执行。
- **C1 阅读页返回按钮位置**（01 类，低）：现为标题下方全宽按钮（absTop 191），非顶部左上惯例位；Android 有 `use_mobile_back_navigation` 兜底，是否调整属设计决策。
- **C5 目录 chips 横滑无溢出指示**：`v0.1.14` 已增加随滚动终点离散消失的右缘渐隐，并固化长/短目录 smoke。
- **来源筛选 chip 极端订阅名 180.7px 高**：`v0.1.14` 已改为单行省略，并通过 `title` 与 `aria-label` 保留完整标签信息。
- **C7 Android 主题文件导出**：`v0.1.14` 已接入系统 SAF 保存器；Android 主题文件导入仍未实现。
- **C20 safe-area 观察项**：`v0.1.14` 已完成第一方 sticky/固定交互元素的样式核查，并在 API 37 的 API 35+ edge-to-edge 分支验证；精确 API 35 设备复跑仍为观察项。
- 上批遗留建议「360×800 断言固化进 smoke 脚本」已在 `v0.1.14` 完成，并纳入 release 聚合入口。

## 给下一位 Agent 的备注

- 入口：本文件 + 上批 `docs/handoffs/2026-07-*` 移动审计记录；样式面 `assets/styles/`，确认槽位 `data-slot="confirm-hint"`。
- **实测坑 1**：Chrome DevTools MCP 的移动仿真会在部分导航/点击后静默失效（`innerWidth` 回到真实窗口值）；对策是每次测量前重新 `emulate` 并用 `documentElement.clientWidth` 自检，严重时就地开新标签页重来。
- **实测坑 2**：移动仿真下 `window.innerWidth`/`scrollWidth` 会同步失真（1244≡1244），溢出判定必须用 `clientWidth` + 逐元素 `getBoundingClientRect`。
- **实测坑 3**：localStorage 种子里 `OffsetDateTime` 的真实序列化格式是 `2026-08-01 17:20:58.311 +00:00:00`（非 RFC3339）；格式错误会被判损坏并备份为 `*-corrupt-*` 键后用空状态启动——静默失败，排查时先看有没有 corrupt 键。
- 若要继续做 C15，先读本文件「风险与后续事项」的归并方案选项，再动 `settings_page/themes/presets.rs`。

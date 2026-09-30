# UI 优化建议复核

- 日期：2026-09-30
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：dbe68e1
- 相关 commit：pending
- 相关 tag / release：N/A（未发布）
- 状态：`validated`（仅完成源码核查与指定色对数值核算）

## 工作摘要

用户提供一份包含 P1–P11 的 UI 评审，询问优化建议是否值得实施。本次对照当前源码、项目设计契约和 W3C 官方说明复核，不实施应用修改。原报告的主要焦点缺口、导航结构和部分请求状态描述已不符合当前代码；建议保留小范围的状态呈现与可访问性改进，避免按旧报告重复修改。

## 影响范围

- 模块：核查 `assets/styles/`、内置主题、`rssr-app` 页面/组件及刷新调用路径。
- 平台：这些源码由 Web、Desktop、Android 共享；本次未做平台运行验收。
- 额外影响：仅新增本交接记录，无源码、样式、依赖、workflow 或发布变更。

## 关键变更

### 评审结论

| 原编号 | 当前证据与建议 |
| --- | --- |
| P1 焦点 | `shell.css:116,249`、`entries.css:77,258`、`reader.css:215` 已覆盖搜索、图标按钮、筛选折叠、来源选择行、阅读底栏。来源 checkbox 已可见。文章标题链接没有自定义焦点规则，但也未清除浏览器默认轮廓；缺少自定义规则不等于实际焦点不可见。应做残余覆盖验收，不按原报告全面重补。 |
| P2 禁用态 | `.button` 确实缺少统一 `:disabled` 外观，hover/active 也未排除禁用按钮。值得补齐；保存与添加订阅已有忙碌文案，因此“与可用状态完全相同”不够准确。 |
| P3 标题 | 主要页面与阅读标题仍用 h2，升为 h1 值得做，并检查剩余标题层级。h2 同样是可被辅助技术识别的标题；缺少 h1 不能直接推导为页面标题不可读取或单独构成 WCAG 不合格。 |
| P4 品牌 | `components/app_nav.rs:37` 的 R 是 Read / 首页与重复点击刷新入口，README 和主题契约已有明确说明。换为品牌 SVG 是产品选择，当前没有必须改的缺陷证据。 |
| P5 图标 | 导航 SVG 和 R 已有 `aria-hidden`，阅读底栏四个图标尚无。可以隐藏装饰图标，但收藏按钮固定文案为“收藏”，状态目前只通过星形字符和 `data-state` 表达；应先配套 `aria-pressed` 或明确变化的操作文案，避免隐藏图标时丢失状态。 |
| P6 断点 | Atlas 的 960px 控制 280px 侧栏退出，基础 720/480px 控制其它布局细节，不同职责可以有不同断点。没有据此确认布局故障；应记录职责并覆盖临界宽度，而非强制同值。 |
| P7 primary | 默认 `.button` 承担 primary 样式是正常变体设计。可在既有主题接口文档说明“缺省即 primary”，无需复制规则或添加空规则。 |
| P8 checkbox | 当前无 `accent-color`。保留原生控件并补主题色是低成本改进；系统默认外观依平台而异，不能笼统称所有平台都是蓝色。 |
| P9 加载/刷新 | 添加订阅已有 disabled、aria-busy 与忙碌文案。刷新全部经 `FeedsPageSession` 进入 `AppShellState::manual_refresh`，同步 begin gate 拦截重复事件，host 的 RefreshFlight 复用进行中刷新；导航已有忙碌状态和 status live region。订阅页“刷新全部”按钮自身缺少忙碌反馈，可复用 shell 状态补齐。通用 StatusBanner 当前为普通 p，应优先考虑动态状态通知，加载动画为可选呈现。 |
| P10 快捷键 | `reader_page/mod.rs:52` 已解释局部焦点容器用于隔离导航搜索框，`tabindex=0` 是键盘进入该范围的入口。可改进名称、焦点呈现与提示，不建议仅为少一个 Tab 停靠点迁到 document。全局 M/F 还会增加字符快捷键和事件生命周期约束。 |
| P11 对比度 | 值得在现有主题作者契约中补充正文、状态、控件和焦点的要求，并按实际合成后的前景/背景验收。不能只检查 muted/bg 两个变量，也不能把没有文档当成已经不达标。 |

### 建议实施顺序

1. 阅读收藏按钮状态语义与装饰图标、必要的动态状态播报、通用禁用态。
2. 页面主标题与后续标题层级、复选框主题色、订阅页刷新按钮忙碌反馈。
3. 在既有主题作者文档补默认 primary、断点职责和对比度要求；验收现有焦点规则和实际主题背景组合。
4. 暂不实施品牌替换、统一所有断点或全局快捷键迁移。

## 验证与验收

### 自动化验证

- `git status --short`：核查开始时工作区干净；结束时仅本记录未跟踪。
- `git log -1 --format='%h %s'`、`git branch --show-current`：当前分支 main，HEAD dbe68e1。
- `rg -n 'focus|disabled|accent-color|h1|h2|h3|960|720|480|reader-shortcut|entry-filters-source-chip|entry-card-title' ...` 与 `nl -ba` / `sed -n`：核对当前选择器、页面标签、导航与快捷键代码。
- `rg -n 'refresh_all|RefreshAll|is_refreshing|single.flight|try_start|is_adding|aria_pressed|aria-pressed|role:.*status|aria_live|aria-live' crates/rssr-app/src crates/rssr-app/tests` 与相关实现阅读：追踪添加与刷新状态、重复触发拦截及现有通知区域。只读代码，未运行这些测试。
- `python3 - <<'PY' ... PY`：读取默认与 Newsprint 的首组 `--muted` / `--bg`，按 sRGB 线性化、相对亮度权重 0.2126/0.7152/0.0722 及 `(Lmax+0.05)/(Lmin+0.05)` 核算，结果如下。
  - 默认 `#6f6257` / `#f4efe6`：5.1497:1。
  - Newsprint `#6f6253` / `#ece1cf`：4.5767:1。
  - 两组不透明色对均超过普通文本 4.5:1；未计算实际渐变、透明叠加或整套主题的全部组合。
- `git diff --check`、新记录格式和引用路径检查：通过。
- 未运行 Rust 构建或测试：本次仅评估建议，没有可执行代码变更。

### 手工验收

- 未运行浏览器、屏幕阅读器、桌面或 Android 实机验收。本次不能确认原生默认焦点在每个平台的实际可见性，也不声称整站达到 WCAG 2.2 AA。
- 已核对 W3C 官方说明：
  - [Focus Visible](https://www.w3.org/WAI/WCAG22/Understanding/focus-visible.html)：允许平台默认焦点，不以有无自定义 CSS 为判断标准。
  - [Headings and Labels](https://www.w3.org/WAI/WCAG22/Understanding/headings-and-labels.html)：需区分描述性标题与结构语义，不将没有 h1 简化为无法读标题。
  - [Button Pattern](https://www.w3.org/WAI/ARIA/apg/patterns/button/)：固定名称的切换按钮可通过 aria-pressed 表达状态。
  - [Status Messages](https://www.w3.org/WAI/WCAG22/Understanding/status-messages.html)：动态状态需能被辅助技术发现，不只依赖视觉动画。
  - [Character Key Shortcuts](https://www.w3.org/WAI/WCAG22/Understanding/character-key-shortcuts.html)：字符快捷键应可关闭、重映射或仅在相应组件获焦时生效。
  - [Contrast Minimum](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html) 与 [Non-text Contrast](https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html)：普通文本 4.5:1、大文本 3:1；必要的控件/状态视觉信息对相邻颜色 3:1，注意各自例外与适用范围。

## 结果

- 已完成建议价值与现状复核；应用行为和样式未修改。
- 本记录 commit: pending；无 tag、push 或 release。
- 原报告不能直接作为当前修复清单，也不足以支持“补齐键盘侧后 WCAG 2.2 AA 基本没有硬伤”的结论。

## 风险与后续事项

- 未来实施需验证默认及四套内置主题的亮/暗/跟随系统、Tab/Shift+Tab、状态切换和缩放/窄屏；不同 WebView 的实际表现仍需覆盖。
- 不为了消除代码表象差异而收紧合法主题布局能力或扩大快捷键作用域。
- 对比度数值仅代表已列出的两组不透明 token，不能代替实际页面验收。

## 给下一位 Agent 的备注

- 从 `components/app_nav.rs`、`pages/reader_page/mod.rs`、`components/status_banner.rs` 和六个基础样式文件查看当前实现，不再依赖旧报告的 app.rs 导航行号。
- 刷新状态复用 `ui/shell.rs` / `ui/shell_state.rs`，避免引入与现有门控脱节的页面局部 busy 标记。
- 主题契约统一维护在 `docs/design/theme-author-selector-reference.md`，保持 Rust 行为与 CSS 呈现的既有职责。

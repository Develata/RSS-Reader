# 2026-07-20 默认 CSS token 化与暗色模式修复

## 工作摘要与背景

用户要求：设计并优化前端默认 CSS，并落实「所有按钮与布局样式都可自定义」的
既有架构承诺（Rust 控制行为，CSS 控制呈现）。

审计发现三处契约破损：

1. `tokens.css` 之外约 70 处硬编码颜色 / 圆角 / 阴影（暖色边线
   `rgba(62,42,27,…)`、面板 `rgba(255,25x,24x,…)`、accent 派生态
   `rgba(176,74,47,…)`、按钮文字与阴影），导致覆写文档变量基本无法换肤——
   内置主题 `midnight-ledger.css` 需要 144 行具体选择器重绘即为此症状。
2. `.theme-light` 在 `.app-shell` 上重复声明整套调色板，因继承就近原则压过
   用户主题在 `:root` 的变量覆写，亮色模式下用户主题变量失效。
3. 暗色模式结构性破损：`.theme-dark` 只换变量，而面板 / 卡片 / 输入框背景
   全是硬编码亮色 → 亮底浅字。

## 方案

两层 token 体系（`assets/styles/tokens.css`）：

- 第 1 层：原 11 个文档化变量，名称与语义不变（向后兼容硬约束）。
- 第 2 层：约 45 个语义 token——表面（panel/reader/card/veil/chip/tint/input）、
  边线（`--line-soft`）、强调态（`--accent-soft/-line/-line-faint`、焦点环）、
  状态色（info/error/success/danger）、阴影（soft/lift/card/float/inset）、
  圆角（panel/card/card-sm/control/pill）、按钮全套（`--button-*`）、
  布局尺寸（`--shell-max-width`、`--rail-width`、`--reader-measure`）、
  动效（`--transition-quick`）、视口底色（`--app-bg`、`--app-bg-dark`）。
- 亮色字面量默认值复现原视觉；`.theme-dark` 与 `.theme-system`（媒体查询）
  整套覆写第 1+2 层 → 暗色模式一次性修复。
- `@supports (color-mix)` 块在末尾把强调态从 `--accent` 派生，主题作者只改
  `--accent` 即可让 hover / 选中 / 焦点环整体跟随；不支持的引擎回落字面量。
- 删除 `.theme-light` 的调色板重声明（`:root` 即亮色默认），用户主题在
  `:root` 覆写变量在亮色模式下真正生效。
- `body:has(.theme-dark)` 渐进增强：暗色时整视口换底色并设置
  `color-scheme: dark`（滚动条 / 表单控件跟随）；不支持 `:has` 的引擎回落为
  旧行为（仅 shell 列变暗）。

五个默认样式表（shell / workspaces / entries / reader / responsive）全部改为
仅消费 token：**选择器与特异性零变更**，内置主题与用户 CSS 的覆写优先级
保持不变。附带清理：合并 `entries.css` 中重复的 `entry-organize-bar` 块；
相近的硬编码 alpha 值收敛到共享 token（视觉差异不可辨）；补齐
`.status-banner[data-state="success"]`（文档已声明该状态但无样式）与
`.button:focus-visible` 焦点环、`.text-input` 的 `color: var(--ink)`
（暗色模式必需）。

## 受影响模块与平台

- `assets/styles/*.css`（6 个文件，全部平台共用：Web / Windows / macOS /
  Linux / Android，经 `include_str!` 内嵌，无 Rust 代码变更）
- `docs/design/theme-author-selector-reference.md`：「可用 CSS 变量」章节
  重写为两层 token 文档 + 覆写建议
- 新增根 `CLAUDE.md`（同会话另一任务：Claude Code 仓库指引）

## 验证

- `cargo test -p rssr-app` → 50 passed（含 reader HTML 正规化等）
- `tests/test_builtin_theme_contracts.rs` → 2 passed（内置主题契约不变）
- 六个 CSS 文件花括号配平检查 → 全部平衡
- `grep` 确认 `tokens.css` 之外硬编码颜色为 0
- 因磁盘余量偏低（40GB），未跑 wasm / Android target 与浏览器回归；CSS 为
  `include_str!` 内容级变更，不影响各 target 编译

## 当前状态、风险、待跟进

- commit: pending
- 风险：字面量收敛处存在不可辨级别的视觉微差（alpha 0.66→0.7 等）；
  `color-mix` / `:has` 在旧 WebKitGTK 上回落为字面量 / 旧行为，无功能损失
- 待跟进：
  1. 跑一轮浏览器视觉回归（`scripts/run_release_ui_regression.sh` 或
     `run_static_web_reader_theme_matrix.sh`），重点核对暗色模式与四个内置主题
  2. 内置主题可后续瘦身（大量具体选择器重绘已因 token 化变得多余），
     属可选优化，不阻塞
  3. 如接受本方案，`docs/testing/global-browser-regression.md` 按惯例更新

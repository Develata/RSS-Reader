# 紧凑顶部导航与文章工作区间距

- 日期：2026-10-26
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 当前代码 HEAD：`6eed78bd8a25488c00b1583eff7dcfdc9485c3e3`
- 相关 commit：`6eed78bd`（样式）、`6df57ad3`（初始验收记录）
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

按用户提供的比例参考缩小 Home / Search / Subscribe / Settings 导航的纵向占位和按钮背景，将空间留给文章。继续在 E 盘当前阅读分支工作，不迁入 v0.2 主线。

## 影响范围

- 模块：`assets/styles/shell.css`、`assets/styles/reading-workspace.css`，以及 Newsprint、Amethyst Glass、Atlas Sidebar 内置主题 CSS。
- 平台：共享前端样式影响 Web、Windows 和 Android；本轮实际运行验证使用已安装的 Google Chrome。
- 无 Rust 逻辑、SQL、持久化数据结构或导航命令变更；保留现有配色、页面左右留白和正文排版。

## 关键变更

- 默认导航高度由 62px 降至 46px；导航与主视图间距由 14px 降至 6px。相同 1440px 宽窗口下，文章主面板顶部由 y=100 变为 y=76，总计腾出 24px。
- 导航按钮使用内缩 6px 的伪元素绘制 32×32px 背景；真实点击范围仍为 44×44px，SVG 仍为 20×20px，Home 字形大小不变。收起箭头仍为 12×20px。
- 悬停、按下、当前页面高亮和 Home 刷新背景作用于内缩表面；键盘聚焦轮廓及 Reader 的刷新提示标记保留。
- 减少图标之间重复间距；收起按钮通过静态纵向位置跟随首行内边距，保持窄窗口搜索换行时的首行对齐。
- 文章面板最小高度和目录吸顶位置相应调整；当前 Atlas Sidebar 桌面主题的目录按独立列定位，不再预留顶部横向导航的高度。
- 内置主题收紧各自覆盖的导航纵向内边距。已有自定义 CSS 或已保存的主题 CSS 快照不自动重写；这些覆盖样式仍可决定自己的留白。

## 验证与验收

### 自动化验证

- `dx build --platform web --package rssr-app --locked`：通过。工具仍输出已有 dx 0.7.10 / Dioxus 0.7.9 版本提示，最终构建成功、退出码 0。
- `cargo test -p rssr-app --locked --test test_builtin_theme_contracts --test test_default_style_token_contract`：4/4 通过。
- Chrome + Playwright 定向检查：60/60 通过。覆盖默认及四套当前内置主题；1440、360、320px 阅读页导航和搜索；展开/收起、Escape 焦点恢复、订阅/设置/Home 切换、键盘焦点、当前页高亮、点击背景外侧的透明按钮区域。
- `node scripts/browser/rssr_small_viewport_assertions.mjs --cdp-base http://127.0.0.1:9254 --static-base http://127.0.0.1:8115 --directory-only true --artifact-dir target/nav-compact-20261026/directory`：144/144 通过，控制台错误 0。覆盖既有目录定位、跟随、浏览和选择保留等交互约定。
- `git diff --check`：通过。

### 视觉验收

- Chrome 1440×810（16:9）与 360×800 明暗主题截图：已查看；导航紧凑、图标未缩小、左右无溢出，文章面板与目录对齐。
- 五套当前主题的 1440 / 360px 布局尺寸记录；Atlas 目录独立列对齐；默认主题滚动后目录与导航保持间隔：通过。
- 截图使用隔离浏览器上下文的目录测试数据，不是用户订阅数据。最终展示截图清除了检查时输入的 Alpha 搜索条件，并等待主题切换动画结束。
- 本轮未重新打包或在 Windows / Android 原生宿主启动验证；Chrome 窄窗口检查不等同于手机实机验收。

## 结果与证据

- 代码改动仅含 5 份 CSS，提交为 `6eed78bd`；代码和初始验收记录已推送，`git ls-remote` 核对远端阅读分支为 `6df57ad34065fbf5de4dd41ea413be9883a519ea`。本次状态更新继续随文档提交推送，不涉及 v0.2 主线。
- 本地证据目录：`target/nav-compact-20261026/`（忽略的构建/验收产物，不纳入 Git）。
- `before.json` / `after.json`：默认布局实测；`layout-cases.json`：主题宽窄窗口尺寸；`navigation-checks.json`：60 项定向检查；`directory/assertions.json`：144 项目录回归。
- 最终预览：`home-light-1440x810.png`、`home-dark-1440x810.png`、`home-light-360.png`、`home-dark-360.png`；构建与测试日志为 `build.log`、`contracts.log`、`directory.log`。

## 风险与后续事项

- 自定义主题可以自行覆盖导航尺寸；本轮验证当前内置版本，不声称任意历史 CSS 都得到相同 46px 高度。
- 如后续要求原生安装包验收，需重新构建这次 CSS 所在版本；上一轮 SQL / 目录修复的原生安装包不包含本次样式。
- 保留工作区中任务外的 SpecKit 删除、旧 handoff 修改及未跟踪设计资料。
- C 盘旧基线工作树的清理阻塞属于此前任务，不通过本次样式工作绕过主工作区保护。

## 给下一位 Agent 的备注

- 在 `E:\gitclone\RSS-Reader` 操作；不要因应用仍报告旧 cwd 而修改 C 盘旧基线。
- CSS 的 32px 仅指可见背景；保持真实 44px 点击区域及图标大小是本次明确约定。
- 浏览器使用 Chrome 或 Thorium，遵循用户偏好。

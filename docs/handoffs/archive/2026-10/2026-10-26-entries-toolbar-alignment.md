# 文章工具栏对齐与目录间距微调

- 日期：2026-10-26
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 当前 HEAD：`1b3025d1fbe5f07e91ad55e52ca02111a7d1bc3d`（修改前）
- 相关 commit：与本记录同批提交，记录生成时 `pending`；可用 `git log -1 -- docs/handoffs/2026-10-26-entries-toolbar-alignment.md` 定位。
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

按用户截图要求微调实际文章列表：标题略上移，同排控件内容略下移，缩小文章与目录间距。核对窄屏时同时修复工具栏额外换行导致目录入口覆盖来源按钮的问题。

## 影响范围

- 模块：`assets/styles/reading-workspace.css`。
- 平台：共享 Web / 桌面 / Android 样式；本轮实测为 Windows 上 Google Chrome 的 Web 构建。
- 额外影响：仅本交接文档；不改 Rust、数据、业务语义、依赖或迁移。

## 关键变更

- “文章”标题视觉上移 2px；数量和全选框下移 3px；同排按钮内容通过内边距下移约 3px。按钮外部位置和已有点击大小不变。
- 限定偏移选择器到工具栏控件，来源弹窗的关闭图标不受影响。全选、图标和来源按钮保持 44px 高；分组控件保持内部 40px、整体 44px。
- `--workspace-panel-gap` 从 12px 改为 8px；保留页面外侧留白、目录宽度及顶部对齐。
- 手机目录入口改为相对工具栏底部定位。原先相对主面板固定 `top: 116px`，在 320px 窗口多次换行后与来源按钮重叠；现在随工具栏末行移动。
- 本次未修改 Penpot；历史草模中的 12px 间距不代表本次实际代码的最新值。

## 验证与验收

### 自动化验证

- `dx build --platform web --package rssr-app --locked`：通过。仍有原有 dx 0.7.10 / Dioxus 0.7.9 版本提示，本次未调整工具链。
- `cargo test -p rssr-app --locked --test test_builtin_theme_contracts --test test_default_style_token_contract`：4 项通过。
- `git diff --check -- assets/styles/reading-workspace.css docs/handoffs/2026-10-26-entries-toolbar-alignment.md`：通过。
- 已安装 Google Chrome，经 Playwright 操作真实页面：1440×810、760×900、360×800、320×740，共 40 项断言通过。覆盖无横向溢出、8px 间距、既有点击高度、可见控件无重叠、来源弹窗适配、弹窗关闭图标、选中计数、批量按钮启用及目录开关保留勾选。
- 使用隔离浏览器上下文和 `directory-contract` 样例（72 篇文章、6 个来源），不使用用户数据。

### 视觉核对

- 已检查实际截图：桌面标题/同排图标对齐、双面板 8px 间距、760px 换行，以及 360px / 320px 手机布局。
- 1440px 实测：标题 y 从 93.5 降为 91.5；数量 y 从 104.5 增为 107.5；图标 y 从 104 增为 107；工具栏保持 y=92、高 44px。
- 证据位于忽略目录 `target/toolbar-alignment-20261026/`：`browser-checks.json`、`before.json`、`desktop.png`、`toolbar.png`、`light-*.png`、构建与测试日志。

## 结果

- 已完成本次小范围样式调整，验证通过；沿用用户既有授权提交并推送当前阅读分支，不迁入 v0.2 主线。
- 单篇阅读页、导航栏、订阅页及文章选择/筛选语义保持既有实现。

## 风险与后续事项

- 未重新生成或安装 Windows / Android 原生包；手机尺寸 Chrome 检查不等于 Android 实机验收。
- 未重跑完整目录行为矩阵或工作区全部 Rust 测试，因未改变相关逻辑；仅验证受布局影响的入口和选择保持。

## 给下一位 Agent 的备注

- 工作目录为 `E:\gitclone\RSS-Reader`，未修改或清理 C 盘旧基线。
- 仅提交此 CSS 与本交接记录，保留任务外的删除、修改和未跟踪文件。
- 桌面截图基于测试文章，不代表用户订阅内容。

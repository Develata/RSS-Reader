# 默认纯色背景与暗色登录文字修正

- 日期：2026-10-26
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 当前 HEAD：修改基于 `a9a10de2`
- 相关 commit：本记录所在提交（提交前为 `pending`）
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

用户反馈登录页面外围背景出现灰黑光晕，希望使用纯色底色。本轮移除默认视口背景的装饰渐变，沿用暖米色 / 深灰配色；视觉检查另发现并修正了暗色登录标题和提示文字的颜色继承问题。

## 影响范围

- 模块：`assets/styles/tokens.css`、`assets/styles/reader.css`。
- 平台：共享样式影响 Web、Windows / Linux / macOS 桌面及 Android；本轮实际运行验证为 Google Chrome 桌面和移动视口。
- 文档：更新主题 token 说明；没有改动登录逻辑、文章交互、数据结构、依赖或数据库。
- 工作区：只在 `E:\gitclone\RSS-Reader` 工作；未操作 C 盘旧基线，未迁入 v0.2 主线。

## 关键变更

### 页面背景

- `--app-bg` 从两层放射渐变加纵向渐变改为纯暖米色 `#f4efe6`。
- `--app-bg-dark` 从两层渐变改为纯深灰 `#141414`。
- 保留卡片表面、细边线和轻阴影；公开 token 名称及自定义主题覆写能力不变。
- 修改前 Chrome 可复现原有渐变，但没有复现用户截图中同等程度的黑色光晕；其具体渲染环境成因仍未确定，不宣称已定位到浏览器或显卡缺陷。

### 暗色登录文字

- 登录门位于 `.app-shell` 外，原先继承 `body` 按浅色 token 计算的文字颜色，即使登录门已切换暗色 token，标题及状态提示仍显示深色文字。
- 在 `[data-layout="web-auth-shell"]` 重新声明 `color: var(--ink)`，让文字与系统配色同步；未更改认证行为。

## 验证与验收

### 自动化验证

- `dx build --platform web --package rssr-app --locked`：最终构建通过。既有 dx 0.7.10 / Dioxus 0.7.9 版本提示仍存在，本轮未改工具链。
- `cargo test -p rssr-app --locked --test test_builtin_theme_contracts --test test_default_style_token_contract`：最终 4 / 4 通过。
- `git diff --check`（本轮路径）：通过。
- 真实 Google Chrome：11 个默认背景状态通过背景颜色、无背景图、无水平溢出检查；另通过 Newsprint 覆写底纹检查。
- 登录页：1200×934 和 390×844，分别检查系统浅色 / 暗色，标题文字颜色与主题一致。
- 文章页：1440×900 和 390×844，分别检查系统浅色 / 暗色。
- 设置页：通过实际导航、选择和保存操作验证显式浅色、显式深色、恢复跟随系统。目录测试 fixture 的归档月数原为 0，会被现有设置校验拒绝；只在隔离测试会话中调整为 1200 后完成保存，未修改 fixture 或校验规则。
- 隔离浏览器内填写本地测试账号并完成首次设置，成功进入应用。无未捕获页面错误。

### 视觉验收

- 已查看桌面及手机视口截图，纯色背景均匀，卡片与背景仍有层次；暗色登录标题 / 提示可读。
- 本轮截图及记录：`target/solid-background-20261026/`，包含 `before-login.png`、`before-style.json`、最终 `login-*.png`、`entries-*.png`、`browser-checks.json`、`build-final.log`、`theme-tests-final.log`。
- 这是实际 Web 渲染截图，不是 Penpot 草模；390px 为 Chrome 移动视口，不代表 Android 安装包验收。

## 结果

- 默认登录页及主应用底色使用纯色；暗色登录文字继承已修正。
- 已关闭本轮创建的隔离 Chrome 会话与 8118 预览服务。
- 按既有授权提交并推送当前阅读分支；不打 tag、不发布安装包、不迁入主线。

## 风险与后续事项

- 未重新构建 / 安装 Windows、Android 原生安装包；包内样式需在后续构建中更新。
- 自定义主题可以继续主动提供渐变或底纹，本轮仅调整默认背景。
- 用户截图里黑晕的具体成因未在 Chrome 重现；已直接移除默认页面生成渐变光晕的图层。

## 给下一位 Agent 的备注

- 保留工作区内其他 agent 的 Penpot 文档修改，以及既有无关 dirty / deleted / untracked 内容。
- 后续登录页 Penpot 提案落地是另一项工作；本轮没有将登录表单重新设计。

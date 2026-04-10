# 2026-04-10 交接汇总

- 日期：2026-04-10
- 作者 / Agent：Codex
- 分支：`main`
- 状态：`active`
- commit：`pending`

## 工作摘要与背景

- 继续推进 `headless active interface + CSS 完全分离 + infra`。
- 本轮重点不是继续大面积改页面逻辑，而是：
  - 继续识别残留的 class 驱动视觉规则
  - 固化一条本地可复用的 web 静态构建 + SPA fallback 回归路径
  - 把 `app-nav`、`entry-directory-rail`、`reader-page`、`web-auth` 这几块壳层从 class 驱动进一步收成稳定语义接口

## 受影响模块与平台

- 前端样式与设计文档
  - `assets/styles/*`
  - `docs/design/*`
- 回归脚本
  - `scripts/run_web_spa_regression_server.sh`
- 本地浏览器回归
  - Web / Chrome MCP

## 关键变更

### 1. 新增本地 SPA fallback 回归脚本

- 新增：
  - [run_web_spa_regression_server.sh](/home/develata/gitclone/RSS-Reader/scripts/run_web_spa_regression_server.sh)
  - [web-spa-regression-server.md](/home/develata/gitclone/RSS-Reader/docs/design/web-spa-regression-server.md)
- 作用：
  - 读取 `target/dx/rssr-app/<profile>/web/public`
  - 为未知路径自动回退到 `index.html`
  - 让 `/entries`、`/feeds`、`/settings` 的本地静态构建回归不再落到 404

### 2. CSS 完全分离基线检查继续收口

- 更新：
  - [css-separation-baseline-checklist.md](/home/develata/gitclone/RSS-Reader/docs/design/css-separation-baseline-checklist.md)
  - [README.md](/home/develata/gitclone/RSS-Reader/docs/design/README.md)
- `app-nav` 已继续迁移：
  - 导航壳、显隐按钮、品牌区、链接区、搜索区都已改成 `data-layout` / `data-slot` / `data-nav`
  - 页面结构中不再保留只为样式服务的 `app-nav*` class
- `entry-directory-rail` / `entry-top-directory` 已继续迁移：
  - 目录栏容器、导航树、section/children/grandchildren、toggle、顶部目录 chip 都已改成 `data-layout`
  - 目录标题、条目标题、条目元信息已统一到 `data-slot`
  - 月份/日期目录跳转已补 `data-nav="entry-directory-*"`
  - 来源目录展开态已补 `data-state="expanded|collapsed"`
- `reader-page` 外壳已继续迁移：
  - 阅读页根、header、toolbar、meta block、body、pagination、bottom bar 都已改成 `data-layout`
  - 标题、meta、底部栏图标/标签已统一到 `data-slot`
  - 同订阅上下文分页已补 `data-context="feed"`
- `web-auth` 本地门禁壳已继续迁移：
  - shell、card、brand、form 都已改成 `data-layout`
  - 品牌 mark / name、title、intro、note 都已统一到 `data-slot`
  - 页面结构中不再保留只为样式服务的 `web-auth-*` class
- `entry-filters` 筛选布局已继续迁移：
  - 根容器、toggle、sources、source-grid 都已补 `data-layout`
  - sources label 已补 `data-slot="entry-filters-sources-label"`
  - `.page` / `.page-header` 当前保留为通用壳类，不继续作为高优先级槽化目标
- 当前剩余最值得继续专项审计的区域已收敛为：
  - 通用布局 class 与允许保留的内容岛边界

### 3. 主题作者与前端接口文档正式对齐到语义接口

- 更新：
  - [theme-author-selector-reference.md](/home/develata/gitclone/RSS-Reader/docs/design/theme-author-selector-reference.md)
  - [frontend-command-reference.md](/home/develata/gitclone/RSS-Reader/docs/design/frontend-command-reference.md)
  - [ui-shell-bus-page-facade.md](/home/develata/gitclone/RSS-Reader/docs/design/ui-shell-bus-page-facade.md)
- 重点：
  - 删掉过时的 `app-nav*`、`reader-page*`、`entry-filters*`、`web-auth*` 契约示例
  - 把主题作者文档改成以 `data-page / data-layout / data-slot / data-nav / data-action / data-field / data-state` 为主
  - 把 `UiCommand / UiRuntime` 的实现路径说明对齐到当前目录结构：
    - `ui/commands/mod.rs`
    - `ui/runtime/mod.rs`
  - 把前端接口说明从“旧 class 列表”收口成“语义接口优先 + 少量通用 class 保留”

## 已执行的验证 / 验收

- 脚本可执行权限：
  - `chmod +x scripts/run_web_spa_regression_server.sh`
- SPA fallback 回归脚本：
  - `bash scripts/run_web_spa_regression_server.sh --skip-build --port 8092`
- 路径回退检查：
  - `curl -I http://127.0.0.1:8092/feeds`
  - `curl -I http://127.0.0.1:8092/settings`
  - `curl -I http://127.0.0.1:8092/assets/rssr-app-*.js`
- Chrome MCP 回归：
  - 直接访问 `http://127.0.0.1:8092/feeds`
  - 直接访问 `http://127.0.0.1:8092/settings`
  - 已确认不再是静态服务器 404，而是正确回退到 SPA 入口页
- 编译检查：
  - `cargo fmt --all`
  - `cargo check -p rssr-app`
  - `git diff --check`
- 文档一致性复查：
  - `rg -n "\\.app-nav|\\.reader-page|\\.reader-header|\\.reader-toolbar|\\.entry-filters|\\.web-auth-|\\.is-active|\\.is-disabled|\\.button\\.secondary|\\.button\\.danger|\\.button\\.danger-outline" docs/design`
  - `rg -n "commands\\.rs|runtime\\.rs" docs/design`
- 语义接口 grep：
  - `rg -n "app-nav__|entry-directory-rail__|entry-top-directory__" assets/styles crates/rssr-app/src -g'*.css' -g'*.rs'`
- 阅读页接口 grep：
  - `rg -n "\\.reader-page\\b|\\.reader-header\\b|\\.reader-toolbar\\b|\\.reader-meta-block\\b|\\.reader-title\\b|\\.reader-meta\\b|\\.reader-pagination\\b|\\.reader-bottom-bar\\b" assets/styles crates/rssr-app/src -g'*.css' -g'*.rs'`
- 门禁壳接口 grep：
  - `rg -n "class:\\s*\\\"web-auth|\\.web-auth-" assets/styles crates/rssr-app/src -g'*.css' -g'*.rs'`
- 筛选布局接口 grep：
  - `rg -n "\\.entry-filters\\b|\\.entry-filters__toggle\\b|\\.entry-filters__sources\\b|\\.entry-filters__source-grid\\b" assets/styles crates/rssr-app/src -g'*.css' -g'*.rs'`
- 导航壳浏览器回归：
  - `bash scripts/run_web_spa_regression_server.sh --skip-build --port 8093`
  - 完成本地登录后检查：
    - `http://127.0.0.1:8093/entries`
    - `http://127.0.0.1:8093/feeds`
    - `http://127.0.0.1:8093/settings`
  - 已确认导航壳、搜索框、导航链接正常显示，console 无新错误

## 当前状态、风险、待跟进项

- 当前工作区未提交。
- 这轮没有继续扩大 CSS 迁移面，而是：
  - 固化回归脚本
  - 把 `app-nav`、`entry-directory-rail`、`reader-page`、`web-auth` 从 class 驱动进一步迁到语义接口
  - 把下一批 class 驱动密集区整理进执行清单
  - 把主题作者与前端公开接口文档正式收口到当前实现
- 下一步最自然的是：
  - 复查剩余通用布局 class 是否还值得继续槽化
  - 或把这轮语义接口迁移和文档收口一起提交

## 相关 commit / worktree 状态

- 已有基线提交：
  - `be2b7dd` `refactor: add semantic layout interfaces for css`
- 当前 worktree：
  - 设计文档更新
  - 新增 SPA fallback 回归脚本
  - 新增对应设计文档
  - `app-nav` / `entry-directory-rail` / `reader-page` / `web-auth` 语义接口迁移

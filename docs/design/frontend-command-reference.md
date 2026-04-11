# 前端命令与界面接口清单

## 目的

这份文档记录 RSS-Reader 当前对外公开的前端命令面、导航语义和稳定界面接口。

它回答的是“现在有哪些命令和接口可以长期依赖”，而不是“为什么这样设计”。功能边界与设计原则本身见：

- [功能设计哲学](./functional-design-philosophy.md)
- [Headless Active Interface 设计目标](./headless-active-interface.md)
- [UI Shell / Bus / Page Facade 边界](./ui-shell-bus-page-facade.md)

---

## 命令边界

前端命令只覆盖以下四类能力：

- 订阅
- 阅读
- 基本设置
- 基础配置交换

超出这四类的命令，不应进入当前前端命令面。

同时，命令面应持续朝以下目标演进：

- 命令先于视图定义
- UI 只是命令的一个触发器
- CLI、快捷键与未来命令面板复用同一命令语义

---

## 当前 UI 命令面

> 说明：下面列的是当前已经对外暴露的 UI 语义面；后续 headless 重构会把它们逐步迁移为统
> 一的 Rust 命令层，但不应改变这些公开语义。

> 当前实现补充：这些公开语义现在已经不只是 DOM 标记，而是开始映射到 `rssr-app/src/ui` 中的
> `UiCommand / UiRuntime / UiIntent`。页面继续负责默认语义壳，当前实现版边界见：
>
> - [UI Shell / Bus / Page Facade 边界](./ui-shell-bus-page-facade.md)

### 订阅相关

- 添加订阅
- 删除订阅
- 刷新单个订阅
- 刷新全部订阅

### 阅读相关

- 标记已读 / 未读
- 切换收藏
- 按标题搜索
- 按来源浏览 / 分组
- 按时间浏览 / 分组
- 查看归档文章
- 仅未读筛选
- 仅收藏筛选
- 返回上一页
- 上一篇未读 / 下一篇未读
- 上一篇同订阅文章 / 下一篇同订阅文章

### 设置与主题相关

- 保存设置
- 应用当前 CSS
- 导入主题文件
- 导出当前 CSS
- 载入所选主题
- 清空当前自定义 CSS
- 上传 WebDAV 配置
- 下载 WebDAV 配置

### 配置交换相关

- 导出配置包
- 导入配置包
- 导出 OPML
- 导入 OPML

---

## 当前 CLI 命令面

当前 `rssr-cli` 与 UI 共用同一套应用服务语义。

长期目标不是删除 CLI，而是让 CLI 变成同一命令面的命令行外壳。

### 订阅相关

- `rssr-cli list-feeds`
- `rssr-cli add-feed <url>`
- `rssr-cli remove-feed <feed-id>`
- `rssr-cli refresh --all`
- `rssr-cli refresh --feed-id <id>`

### 配置交换相关

- `rssr-cli export-config [--output <path>]`
- `rssr-cli import-config <file>`
- `rssr-cli export-opml [--output <path>]`
- `rssr-cli import-opml <file>`
- `rssr-cli push-webdav <endpoint> <remote-path>`
- `rssr-cli pull-webdav <endpoint> <remote-path>`

### 设置相关

- `rssr-cli show-settings`
- `rssr-cli save-settings ...`

---

## 页面级接口

页面级作用域应长期保持稳定：

- `data-page="feeds"`
- `data-page="entries"`
- `data-page="reader"`
- `data-page="settings"`

用途：

- 限定样式作用域
- 让用户 CSS 和 AI 生成 CSS 避免跨页污染
- 允许视图壳在不改行为逻辑的情况下自由重排

---

## 导航接口

导航语义标记应长期保持稳定：

- `data-nav="feeds"`
- `data-nav="entries"`
- `data-nav="settings"`
- `data-nav="back"`
- `data-nav="feed-entries"`
- `data-nav="previous-feed-entry"`
- `data-nav="next-feed-entry"`
- `data-nav="previous-unread-entry"`
- `data-nav="next-unread-entry"`

这些标记只表达导航语义，不承载业务副作用。

长期要求：

- 导航必须可被视图壳、命令面板和未来快捷键系统复用
- 导航语义不应依赖某个具体按钮位置

---

## 命令接口

命令按钮应暴露稳定的 `data-action`：

- `data-action="add-feed"`
- `data-action="remove-feed"`
- `data-action="refresh-feed"`
- `data-action="refresh-all"`
- `data-action="export-config"`
- `data-action="import-config"`
- `data-action="export-opml"`
- `data-action="import-opml"`
- `data-action="save-settings"`
- `data-action="push-webdav"`
- `data-action="pull-webdav"`
- `data-action="mark-read"`
- `data-action="toggle-starred"`
- `data-action="group-by-source"`
- `data-action="group-by-time"`
- `data-action="toggle-archived"`
- `data-action="apply-custom-css"`
- `data-action="export-custom-css-file"`
- `data-action="import-custom-css-file"`
- `data-action="apply-selected-theme"`
- `data-action="apply-theme-preset"`
- `data-action="remove-theme-preset"`
- `data-action="clear-custom-css"`
- `data-action="open-github-repo"`
- `data-action="show-top-nav"`
- `data-action="hide-top-nav"`

如果未来需要新增命令，应优先保持这个命名风格：

- 使用短语义英文
- 使用 kebab-case
- 一个动作只表达一个清晰业务语义

## Headless 命令面迁移要求

后续重构中，`data-action` 的职责应逐步收敛为：

- 公开语义标记
- CSS / AI / 自动化可依赖的稳定选择器
- 与统一 Rust 命令定义的一对一映射

而不是：

- 业务逻辑本体
- 页面私有临时点击逻辑
- DOM 结构的替代命名
- 容器或展示位本身的标签

推荐最终形成以下命令族：

- Feed commands
- Entry commands
- Settings commands
- Config exchange commands
- Navigation commands
- UI shell commands

这组命令族的目标，是让同一语义能够被：

- GUI
- CLI
- 快捷键
- 命令面板
- 未来的表格 / 工作台视图

共同复用。

---

## 稳定界面接口

当前应优先依赖的不是 page 私有 class，而是：

- `data-page`
- `data-layout`
- `data-slot`
- `data-nav`
- `data-action`
- `data-field`
- `data-state`
- `data-variant`
- `data-density`

通用 class 仍然保留为公开界面接口，但范围应收敛在真正通用的壳和组件上：

- `.app-shell`
- `.app-header`
- `.page`
- `.status-banner`
- `.button`
- `.text-input`
- `.text-area`
- `.select-input`
- `.field-label`
- `[data-slot="reader-body-html"]`

不再建议把以下 page-specific class 当作长期契约：

- `.app-nav*`
- `.reader-page*`
- `.entry-filters*`
- `.entry-directory-*`
- `.web-auth-*`

这些区域现在都应优先通过语义接口消费。

---

## 字段接口

设置页中的输入字段不应伪装成命令。它们应暴露稳定的 `data-field`，而把真正触发副作用的
按钮继续保留为 `data-action`。

当前已经稳定公开的字段接口包括：

- `data-field="entry-search"`
- `data-field="search-title"`
- `data-field="read-filter-unread"`
- `data-field="read-filter-read"`
- `data-field="starred-filter-starred"`
- `data-field="starred-filter-unstarred"`
- `data-field="entry-source-filter"`
- `data-field="entry-grouping-mode"`
- `data-field="show-archived"`
- `data-field="feed-url-input"`
- `data-field="config-text"`
- `data-field="opml-text"`
- `data-field="theme-mode"`
- `data-field="list-density"`
- `data-field="startup-view"`
- `data-field="refresh-interval"`
- `data-field="archive-after-months"`
- `data-field="reader-font-scale"`
- `data-field="preset-theme-select"`
- `data-field="custom-css"`
- `data-field="webdav-endpoint"`
- `data-field="webdav-remote-path"`

---

## 状态接口

当某个节点需要暴露稳定的默认状态，而这个状态不应依赖 class 名、按钮文案或 DOM 层级时，
应优先使用 `data-state`。

当前已经开始使用的状态语义包括：

- `data-state="expanded" | "collapsed"`
- `data-state="info" | "error" | "success"`
- `data-state="pending" | "idle"`
- `data-state="confirm" | "idle"`
- `data-state="active" | "inactive"`
- `data-state="available" | "unavailable"`
- `data-state="read" | "unread"`
- `data-state="starred" | "unstarred"`
- `data-state="html" | "text"`
- `data-state="empty" | "populated"`

`data-state` 的职责应保持为：

- 默认状态语义
- CSS / 自动化可依赖的稳定状态面
- facade 或 shell 已经明确投影出来的可读状态

而不是：

- 替代 class 的视觉命名
- 临时 DOM hack
- 页面私有且不可复用的内部标记
- `data-field="archive-after-months"`
- `data-field="reader-font-scale"`
- `data-field="custom-css"`
- `data-field="preset-theme-select"`
- `data-field="webdav-endpoint"`
- `data-field="webdav-remote-path"`

其它已稳定的字段接口：

- `data-field="feed-url-input"`
- `data-field="config-text"`
- `data-field="opml-text"`
- `data-field="search-title"`

字段接口用于：

- 自动化定位输入控件
- 用户 CSS 和极端重排时保留语义锚点
- 区分“持续输入值”和“触发一次动作”

---

## 布局与槽位接口

容器、画廊、卡片壳、列表包裹层等展示位不应再使用 `data-action`。

它们应优先使用：

- `data-layout`
- `data-slot`
- `data-page`
- `data-theme-preset`
- 其它明确描述载荷的属性

当前已经稳定公开的布局/槽位接口包括：

- `data-layout="app-nav-shell"`
- `data-layout="app-nav-links"`
- `data-layout="app-nav-search"`
- `data-layout="web-auth-shell"`
- `data-layout="page-header"`
- `data-layout="page-section-header"`
- `data-layout="stats-grid"`
- `data-layout="feed-workbench-single"`
- `data-layout="exchange-grid"`
- `data-layout="settings-grid"`
- `data-layout="entries-layout"`
- `data-layout="entry-groups"`
- `data-layout="entry-directory-rail"`
- `data-layout="entry-top-directory"`
- `data-layout="entry-filters"`
- `data-layout="reader-page"`
- `data-layout="reader-header"`
- `data-layout="reader-toolbar"`
- `data-layout="reader-body"`
- `data-layout="reader-bottom-bar"`
- `data-slot="page-header-actions"`
- `data-slot="page-title"`
- `data-slot="page-intro"`
- `data-slot="feed-card-title"`
- `data-slot="feed-card-meta"`
- `data-slot="entry-card-title"`
- `data-slot="entry-card-meta"`
- `data-slot="entry-directory-title"`
- `data-slot="entry-directory-meta"`
- `data-slot="reader-title"`
- `data-slot="reader-meta"`
- `data-slot="reader-bottom-bar-label"`
- `data-slot="theme-card-title"`
- `data-slot="theme-card-swatches"`
- `data-slot="theme-card-swatch"`

它们的职责是：

- 暴露稳定结构语义
- 给 CSS / AI / 自动化提供不依赖 DOM 层级的锚点
- 让 page facade 和 UI shell 能继续退化成默认语义壳

---

## 使用建议

如果你要做这些事，建议这样选文档：

- 想判断某个功能该不该加：
  - 先看 [功能设计哲学](./functional-design-philosophy.md)
- 想确认某个按钮、页面或导航接口是否可长期依赖：
  - 看这份清单
- 想理解这些接口以后如何被提升为真正的 headless 命令面：
  - 看 [Headless Active Interface 设计目标](./headless-active-interface.md)
- 想写主题或让 AI 生成 CSS：
  - 再看 [主题作者选择器参考](./theme-author-selector-reference.md)

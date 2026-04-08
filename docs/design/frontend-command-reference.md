# 前端命令与界面接口清单

## 目的

这份文档记录 RSS-Reader 当前对外公开的前端命令面、导航语义和稳定界面接口。

它回答的是“现在有哪些命令和接口可以长期依赖”，而不是“为什么这样设计”。功能边界与设计原则本身见：

- [功能设计哲学](./functional-design-philosophy.md)
- [Headless Active Interface 设计目标](./headless-active-interface.md)

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
- `data-action="filter-unread"`
- `data-action="filter-starred"`
- `data-action="filter-read"`
- `data-action="filter-unstarred"`
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

## 稳定组件接口

以下 class 应尽量作为公开界面接口长期保持稳定：

- `.app-shell`
- `.app-header`
- `.app-nav`
- `.app-nav-shell`
- `.page`
- `.page-section`
- `.feed-list`
- `.feed-card`
- `.entry-list`
- `.entry-card`
- `.reader-toolbar`
- `.reader-body`
- `.settings-panel`
- `.status-banner`
- `.button`
- `.text-input`
- `.text-area`
- `.select-input`

---

## 字段接口

设置页中的输入字段不应伪装成命令。它们应暴露稳定的 `data-field`，而把真正触发副作用的
按钮继续保留为 `data-action`。

当前已稳定的设置页字段接口：

- `data-field="theme-mode"`
- `data-field="list-density"`
- `data-field="startup-view"`
- `data-field="refresh-interval"`
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

## 容器与展示位边界

容器、画廊、卡片壳、列表包裹层等展示位不应再使用 `data-action`。

它们可以使用：

- 稳定 class
- `data-page`
- `data-theme-preset`
- 其它明确描述数据载荷的 attribute

但不应伪装成“动作接口”。

---

## 状态接口

状态应通过 class 或 data attribute 明确暴露，而不是让主题去猜测逻辑：

- `.is-active`
- `.is-loading`
- `.is-disabled`
- `.is-read`
- `.is-starred`
- `.has-error`
- `data-state="loading|ready|error"`

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

# 前端命令与界面接口清单

## 目的

这份文档记录 RSS-Reader 当前对外公开的前端命令面、导航语义和稳定界面接口。

它回答的是“现在有哪些命令和接口可以长期依赖”，而不是“为什么这样设计”。功能边界与设计原则本身见：

- [功能设计哲学](./functional-design-philosophy.md)

---

## 命令边界

前端命令只覆盖以下四类能力：

- 订阅
- 阅读
- 基本设置
- 基础配置交换

超出这四类的命令，不应进入当前前端命令面。

---

## 当前 UI 命令面

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

---

## 导航接口

导航语义标记应长期保持稳定：

- `data-nav="feeds"`
- `data-nav="entries"`
- `data-nav="settings"`
- `data-nav="feed-entries"`

这些标记只表达导航语义，不承载业务副作用。

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
- `data-action="search-title"`
- `data-action="group-by-source"`
- `data-action="group-by-time"`
- `data-action="toggle-archived"`
- `data-action="filter-unread"`
- `data-action="filter-starred"`

如果未来需要新增命令，应优先保持这个命名风格：

- 使用短语义英文
- 使用 kebab-case
- 一个动作只表达一个清晰业务语义

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
- 想写主题或让 AI 生成 CSS：
  - 再看 [主题作者选择器参考](./theme-author-selector-reference.md)

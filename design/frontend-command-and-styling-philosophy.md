# 前端命令与样式设计哲学

## 背景

RSS Reader 的界面需要同时满足两个目标：

1. 允许用户高度自定义页面样式和布局。
2. 保持业务行为稳定、可测试、可跨平台复用。

为了同时满足这两个目标，本项目采用如下设计哲学：

- **Rust 控制行为，CSS 控制呈现。**
- **CLI 与前端共享同一套应用服务，而不是互相依赖。**
- **默认样式开箱即用，自定义样式后加载覆盖。**
- **前端对外暴露稳定的样式接口，不把内部 DOM 结构当作公开 API。**

---

## 核心原则

### 1. 行为与样式严格分离

- Rust 负责命令执行、数据读写、页面导航、状态流转和错误处理。
- CSS 只负责布局、视觉层级、间距、字体、颜色、显示顺序和可见性。
- CSS 不参与业务逻辑，不决定命令是否存在，也不决定命令如何执行。

这意味着：

- “添加 feed”“删除 feed”“刷新订阅”“导入配置”“导出配置” 都必须由 Rust 实现。
- “首页”“订阅”“文章”“设置”的跳转必须由 Rust 路由控制。
- CSS 可以决定按钮放在顶部、侧边还是卡片内，但不能改变按钮的业务语义。

### 2. 命令是第一类能力

用户可见的核心操作应被定义为明确的命令，而不是散落在 UI 点击事件中的临时逻辑。

每个命令都应具有：

- 明确名称
- 明确输入参数
- 明确成功/失败结果
- 可被 CLI 和 UI 共用的实现入口

### 3. CLI 和 UI 是两个入口，不是上下游关系

前端按钮不直接调用 CLI 进程。

推荐结构：

- `rssr-application` 提供命令对应的 service/use case
- CLI 调用这套 service
- 前端按钮也调用这套 service

不推荐结构：

- 前端点击按钮后去 spawn CLI 子进程

原因：

- Web 端不能自然调用本地 CLI
- 桌面端直接调用 CLI 会引入路径、参数、权限和 stderr 解析问题
- CLI 应当是交互入口，不应成为 UI 的内部 RPC 协议

### 4. 样式自定义通过“公开接口”完成

项目不承诺所有内部 DOM 永远不变，但承诺一组稳定的样式接口：

- 页面级命名空间
- 命令按钮语义标记
- 导航语义标记
- 组件级稳定 class
- CSS 变量

用户样式应优先依赖这些接口，而不是耦合内部层级结构。

---

## 当前实现状态

截至当前版本，这套设计已经部分落地，并且以下能力已经真实实现：

### 已实现的样式接口

- 页面级接口：
  - `data-page="home"`
  - `data-page="feeds"`
  - `data-page="entries"`
  - `data-page="reader"`
  - `data-page="settings"`
- 导航级接口：
  - `data-nav="home"`
  - `data-nav="feeds"`
  - `data-nav="entries"`
  - `data-nav="settings"`
  - `data-nav="feed-entries"`
- 命令级接口：
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
  - `data-action="filter-unread"`
  - `data-action="filter-starred"`

### 已实现的用户 CSS 能力

- 默认样式由 `assets/styles.css` 提供
- 用户可在设置页编辑并保存 `custom_css`
- 应用启动时会在默认样式之后注入用户 CSS
- `custom_css` 已进入设置持久化、配置导入导出和 schema 契约
- 主题作者可参考 [theme-author-selector-reference.md](./theme-author-selector-reference.md)

### 已实现的 CLI 命令

当前 `crates/rssr-cli/` 已提供这些命令：

- `rssr-cli list-feeds`
- `rssr-cli add-feed <url>`
- `rssr-cli remove-feed <feed-id>`
- `rssr-cli refresh --all`
- `rssr-cli refresh --feed-id <id>`
- `rssr-cli export-config [--output <path>]`
- `rssr-cli import-config <file>`
- `rssr-cli export-opml [--output <path>]`
- `rssr-cli import-opml <file>`
- `rssr-cli show-settings`
- `rssr-cli save-settings ...`
- `rssr-cli push-webdav <endpoint> <remote-path>`
- `rssr-cli pull-webdav <endpoint> <remote-path>`

这些 CLI 命令与 UI 按钮共享同一套应用服务语义，不通过 spawn 子进程互相调用。

---

## 推荐架构

## 分层职责

### 应用服务层

位置：

- `crates/rssr-application/src/`

职责：

- 定义并实现核心命令
- 调用 repository、parser、config sync 等基础设施
- 返回结构化结果和错误

示例命令能力：

- 添加 feed
- 删除 feed
- 刷新单个 feed
- 刷新全部 feed
- 导入配置
- 导出配置
- 导入 OPML
- 导出 OPML
- 保存设置
- 上传 WebDAV 配置
- 下载 WebDAV 配置

### CLI 层

建议位置：

- `crates/rssr-cli/` 或在现有二进制中增加 CLI 模式

职责：

- 将 service 暴露为命令行命令
- 负责参数解析、终端输出、退出码

职责边界：

- 不实现新的业务逻辑
- 不与 UI 共享私有状态
- 不成为 UI 的调用桥

### 前端 UI 层

位置：

- `crates/rssr-app/src/`

职责：

- 调用应用服务
- 管理页面状态
- 呈现结果
- 处理导航和交互反馈

职责边界：

- 不直接写 SQL
- 不自己实现导入导出逻辑
- 不直接依赖命令行输出格式

### CSS 层

位置：

- `assets/styles.css`
- 未来用户自定义样式存储位置

职责：

- 控制视觉表现
- 控制布局
- 控制组件显隐
- 覆盖默认样式

职责边界：

- 不参与业务流程
- 不决定命令可用性
- 不存储业务状态

---

## 前端按钮模型

前端按钮分为两类，这一分类应稳定存在于整个项目中。

### 1. 命令按钮

命令按钮触发后端命令或应用服务操作。

典型命令按钮：

- 添加 feed
- 删除 feed
- 刷新单个 feed
- 刷新全部 feed
- 导入配置
- 导出配置
- 导入 OPML
- 导出 OPML
- 保存设置
- 上传 WebDAV 配置
- 下载 WebDAV 配置
- 标记已读
- 切换收藏

设计要求：

- 每个命令按钮必须有稳定语义标记
- 每个命令必须有明确错误提示
- 每个命令执行中必须能提供 loading/disabled 状态

### 2. 导航按钮

导航按钮只负责页面跳转或视图切换，不应承载后端副作用。

典型导航按钮：

- 首页
- 订阅
- 文章
- 设置
- 返回全部文章
- 进入某个 feed 的文章列表
- 进入阅读页

设计要求：

- 跳转逻辑由 Rust 路由控制
- CSS 可以移动这些按钮的位置
- CSS 不应替代路由逻辑

---

## 样式接口准则

为了支持完整用户自定义 CSS，UI 需要提供稳定的样式接口。

## 页面级接口

每个页面应带页面命名空间：

- `data-page="home"`
- `data-page="feeds"`
- `data-page="entries"`
- `data-page="reader"`
- `data-page="settings"`

作用：

- 让用户 CSS 可以安全限制作用域
- 降低不同页面间样式污染

## 命令接口

每个命令按钮应暴露：

- `data-action="<action-name>"`

推荐命名：

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

## 导航接口

每个导航元素应暴露：

- `data-nav="<nav-name>"`

推荐命名：

- `data-nav="home"`
- `data-nav="feeds"`
- `data-nav="entries"`
- `data-nav="settings"`
- `data-nav="back"`

## 组件接口

每个高层组件都应有稳定 class，避免用户只能通过 DOM 层级猜测选择器。

推荐示例：

- `.app-shell`
- `.app-header`
- `.app-nav`
- `.page-section`
- `.feed-list`
- `.feed-card`
- `.entry-list`
- `.entry-card`
- `.reader-toolbar`
- `.settings-panel`
- `.status-banner`
- `.button`
- `.button--primary`
- `.button--secondary`
- `.button--danger`

## 状态接口

状态应通过 class 或 data attribute 明确表达。

推荐示例：

- `.is-active`
- `.is-loading`
- `.is-disabled`
- `.is-read`
- `.is-starred`
- `.has-error`
- `data-state="loading|ready|error"`

---

## CSS 变量策略

为了让用户既能轻量换肤，也能完全重写布局，推荐默认样式先变量化。

至少应暴露以下变量：

- 颜色：
  - `--color-bg`
  - `--color-surface`
  - `--color-text`
  - `--color-text-muted`
  - `--color-accent`
  - `--color-border`
  - `--color-danger`

- 字体：
  - `--font-body`
  - `--font-heading`
  - `--font-mono`
  - `--font-size-base`

- 间距：
  - `--space-1`
  - `--space-2`
  - `--space-3`
  - `--space-4`
  - `--space-6`
  - `--space-8`

- 圆角与阴影：
  - `--radius-sm`
  - `--radius-md`
  - `--radius-lg`
  - `--shadow-sm`
  - `--shadow-md`

- 布局：
  - `--content-width`
  - `--sidebar-width`
  - `--toolbar-height`

---

## 用户自定义 CSS 方案

## 加载顺序

建议固定为：

1. 内置默认样式
2. 可选主题变量覆盖
3. 用户自定义 CSS

这样保证：

- 默认 UI 永远可运行
- 用户可以按需轻量覆盖
- 用户也可以完全重写样式

## 存储形式

建议支持两种来源：

- 配置中的原始 CSS 文本
- 本地文件导入后的 CSS 文本

最终运行时统一视为一段用户样式文本注入页面。

## 风险提示

允许完整 CSS 自定义意味着用户可能：

- 隐藏关键按钮
- 把布局改坏
- 制造阅读可用性问题

这应被定义为“高级用户能力”，不是 bug。

项目应保证的是：

- 行为逻辑仍然正确
- 公开样式接口尽量稳定
- 默认样式始终可用

---

## CLI 设计准则

## 设计目标

CLI 不是给 UI 兜底，而是给高级用户、自动化脚本和调试流程使用。

## 建议命令集合

建议至少提供：

```text
rssr feed add <url>
rssr feed remove <feed-id>
rssr feed list
rssr refresh --all
rssr refresh --feed <feed-id>
rssr config export [--format json]
rssr config import <path>
rssr opml export <path>
rssr opml import <path>
rssr settings get
rssr settings set <key> <value>
rssr remote push
rssr remote pull
```

## CLI 输出准则

- 终端友好，但不要把自然语言输出作为机器协议
- 若未来需要脚本自动化，可增加 `--json`
- 退出码必须可靠
- 错误信息应与应用服务错误保持一致语义

---

## 实施准则

## 短期落地步骤

1. 为现有页面和按钮补齐稳定的 `data-page`、`data-action`、`data-nav`
2. 将默认样式逐步改造成 CSS 变量驱动
3. 为应用服务梳理“命令清单”
4. 新增 CLI crate 或 CLI 模式，直接复用应用服务
5. 在设置中加入“自定义 CSS”输入或导入能力
6. 启动时按顺序注入默认样式和用户样式

## 演进约束

- 新增按钮时必须先定义语义接口，再设计样式
- 组件重构时优先保持 `data-*` 接口稳定
- CLI 不得绕开应用服务直接操作数据库
- UI 不得直接依赖 CLI 文本输出

---

## 非目标

以下内容不属于本方案目标：

- 用 CSS 改变业务权限
- 用 CSS 触发后端命令
- 把 CLI 变成前端内部 RPC
- 承诺内部 DOM 层级永远不变
- 提供无边界的脚本执行能力

---

## 总结

本项目的前端与命令系统应遵循以下总原则：

- **Rust 决定行为**
- **CLI 与 UI 共享服务，不互相模拟**
- **CSS 决定布局与视觉**
- **公开稳定的样式接口，而不是公开内部实现**
- **默认体验必须可靠，自定义体验必须强大**

这套哲学可以同时支持：

- 桌面/Web 的统一前端行为
- 高级用户的深度样式自定义
- 后续引入 CLI 自动化能力
- 长期演进中的结构重构与兼容性控制

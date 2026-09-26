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

### Home、搜索与阅读交互

顶部常态为 `[R] [搜索] [S] [设置]`，Reader 最左侧增加返回图标。R 同时是 brand / Read / Home，没有独立“文章”导航按钮。

| 稳定接口 | Rust 语义 / 结果 |
| --- | --- |
| `data-action="activate-home"` | `resolve_home_action(AppRoute)`：只有全局 `EntriesPage` 返回 `ManualRefresh`；其他路由只导航到全局文章页 |
| `data-action="refresh-all"`、首页下拉刷新 | 与 Home 再次点击共用 `AppShellState::manual_refresh` → `ShellCommand::ManualRefresh` → 既有 `RefreshPort` / application 刷新用例 |
| `data-action="toggle-search"` | shell 的 `NavMode::Normal / Search`；搜索态收起 S / Settings，保留 R / Reader 返回 |
| `data-field="entry-search"` | shell 持有并持久化搜索词；Enter 进入全局文章页按标题搜索，Esc 退出搜索态（输入法 composing 期间放行 Esc） |
| `data-nav="feeds"` / `settings` / `back` | 纯导航；返回按钮和 Android 系统返回复用 history / fallback 策略 |
| `data-action="entry-page-previous"` / `entry-page-next` | 既有 Entries reducer 更新并限制页号，保持筛选和分组，滚至新页起点 |
| `data-action="open-reader-image"` / `close-reader-image` | `ImageViewerState::Closed / Open`，复用已渲染图片地址；Esc、背景或 Android back 同样关闭 |

手动刷新在 App scope 中运行，同步取得 in-flight 状态，连续 R、下拉和订阅页“刷新全部”不会重复创建批次。页面卸载不取消该任务。完成或部分失败后 revision 只让列表与订阅快照重取；Reader 正文不订阅刷新 revision。自动刷新继续使用现有 host 调度；host capability 内的共享 `RefreshFlight` 让自动与手动的重叠请求等待同一轮结果，不改变阅读页加载语义。进行中任务被取消时，等待者收到错误，下一次请求可以重试；不缓存已完成的刷新结果。

默认样式在 Reader 中只通过 R 的进行中动画与成功 / 错误小标记显示刷新反馈，避免提示浮层盖住标题或正文；反馈期间 `manual-refresh-status` 的 live region 与 R 的完整 `title` 提示保留，其他页面显示文字结果。成功结果约 3 秒、错误结果约 6 秒后由 App 级状态清除，避免页面切换后重现旧提示；旧计时器不能清除新一轮刷新。此呈现差异不改变刷新命令或任务生命周期。

文章列表的 `LoadEntries` 查询保持页面生命周期，并以页面内 generation 校验结果：只有最新发起的查询可发布列表、归档计数和状态，旧查询晚到的成功或失败都被丢弃，避免刷新与筛选切换交错时显示错误结果。该校验不改变写入命令或全局刷新任务的生命周期。

首次列表查询还需等待设置、工作区偏好和来源映射全部就绪，避免先按默认分页或筛选显示，再跳到保存的设置。页面内 `Pending / Loaded / Unavailable` 区分等待、成功和读取失败：失败后允许读取文章并保留错误提示，但不保存回退偏好覆盖原值；后续刷新可重试。Bootstrap 不订阅自身的完成状态，避免成功后重复查询来源；独立的 bootstrap generation 丢弃其旧成功和旧失败结果，不与列表查询互相失效。

下拉刷新仅在全局文章页、页面已到顶部且向下拖动至少 80 CSS px 后松手触发；横向、多指、文本选择、表单操作及嵌套滚动区不参与。被动 DOM bridge 只传坐标、滚动和目标事实；Rust 决定 pulling / armed，刷新反馈复用 shell 的 refreshing / finished / error。没有字母 R 快捷键，也没有全局 `touch preventDefault`。

图片 bridge 仅对已消毒正文中的图片增加点击/键盘入口，不改变 sanitizer。Rust 持有 viewer 状态，原生 modal dialog 承担焦点约束和背景隔离；DOM 适配锁定并恢复滚动。正文、标题、metadata 的选择和复制沿用原生行为，带 modifier 的阅读快捷键继续放行；原生端 release WebView 允许系统右键菜单，不接管剪贴板。

Reader session 以 `entry_id + load_generation` 校验异步 UI 结果，切换文章再返回同一篇也不会接纳上次访问的迟到结果。图片本地化只更新缓存，不触发当前正文重载；下次打开文章使用新的本地引用，避免正文替换打断滚动、选区或图片查看器。

位置记忆由 UI runtime 的 `reading_position` 管理，仅保留本次运行内的分页和坐标/锚点；没有 domain/application、数据库或 CLI 契约变化。列表只在初次加载成功后恢复分页，等待 presenter 与页码一致后才进行 DOM 恢复；主动筛选或翻页不会套用旧位置。以文章 ID 和视口偏移定位，找不到时回退到有效滚动范围内的原坐标。Reader 使用正文块锚点与偏移，顶部不因元信息高度变化而离开顶部。DOM bridge 提供测量、导航代次、时钟与用户输入事实，Rust 决定恢复、回退与停止，最多校正 2 秒；用户主动输入或导航会终止校正。图片 dialog 锁定滚动期间不记录临时坐标。

返回列表的文章可有 `data-return-highlight="true"`，约 2 秒后移除；这是定位提示，不代表已读。`data-position-key` / `data-position-page` / `data-position-ready` / `data-position-entry` 是内部桥接字段，不作为用户主题接口。Web 页面刷新或应用结束清除位置，位置不参与配置交换。

位置采集发生在操作边界：DOM 捕获阶段的控件点击、左右方向键切文、表单提交与浏览器 `popstate`。Web host 在 Dioxus 启动前安装 `popstate` 转发，通过内部 `rssr-history-leave` 事件在旧正文卸载前采集。连续滚动不监听位置、不扫描正文；每次进入页面的首次滚轮／触摸／滚动按键输入只发送轻量取消事实。Rust 只保存 `capture`，不会以取消或布局探测覆盖已存锚点。桥接消息带序号，旧响应不能覆盖更新的输入或导航事实。Android 系统返回通过内部 `rssr-capture-position` 事件先取得快照，最多等待 500ms，失败仍继续返回；等待期间合并重复返回并在路由已变化时放弃旧返回请求。这些事件与消息序号均为内部 host bridge 协议，不是主题或 application/domain 契约。

`data-nav="entries"` 仍用于纯导航的“返回全部文章”链接；R 使用 `data-action="activate-home"`，不能把它当作纯导航选择器。旧 `show-top-nav` / `hide-top-nav`、`app-nav-brand-name`、`reader-toolbar` 已移除。旧 `nav_hidden` / `rssr-nav-hidden` 偏好被忽略，搜索词与文章筛选折叠偏好保留。`app-nav-shell` 的 `data-state` 现为 `normal` / `search`。

搜索输入框保持 shell 级状态；当持久化的旧版侧栏 CSS 把导航压窄时，导航行允许换行，输入框占满下一行，避免只露出极窄的一截。

来源选择继续保留 `entry-filters-source-chip` selector 兼容用户主题，但视觉为带可见 checkbox 的换行选择行；选择区有纵向滚动上限，名称本身不省略。分页只渲染一份 `entry-pagination`，位于页面 panel 的同级，固定于视口下方并预留 safe-area / 内容末尾空间。

### 订阅相关

- 添加订阅
- 删除订阅
- 刷新单个订阅
- 刷新全部订阅

`feed-form` 使用原生 submit；地址输入 Enter 与 `data-action="add-feed"` 调用同一添加命令。`refresh-all` 为独立 button，不提交地址输入。

Feeds reducer 用正在提交的地址 `Option<String>` 同步去重，按钮以 disabled / `aria-busy` 提示；输入仍可编辑，成功仅清空未变化的已提交地址，失败释放 pending 并保留输入。`LoadSnapshot` 以页面内 query generation 丢弃旧查询的成功和错误，避免刷新、添加与删除交错时旧统计覆盖新结果。两者均属于共享 Rust 页面交互，不改变 application 用例或增加平台分支。

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
- 应用所选主题
- 清空当前自定义 CSS
- 上传 WebDAV 配置
- 下载 WebDAV 配置

所有外观保存入口共用同步 pending gate。一次保存针对点击时的草稿快照；期间的新编辑继续留在草稿中，完成提示明确其尚未保存。失败不会恢复旧值覆盖草稿。该策略属于页面 session，不改变 settings application 用例。

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

`refresh` 必须且只能指定其中一个目标；参数无效时在打开本地数据库前退出。

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

结构化导出与 `show-settings` 的 stdout 只含结果数据；诊断日志写入 stderr。导出可直接重定向后再导入；失败返回非零退出码。

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
- `data-action="open-original"`：原文外部打开入口（无 URL 时不渲染）
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
- `data-action="activate-home"`
- `data-action="toggle-search"`
- `data-action="entry-page-previous"`
- `data-action="entry-page-next"`
- `data-action="open-reader-image"`
- `data-action="close-reader-image"`

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
- `data-layout="reader-image-viewer"`
- `data-layout="reader-image-viewport"`
- `data-layout="reader-body"`
- `data-layout="reader-bottom-bar"`
- `data-slot="page-header-actions"`
- `data-slot="page-title"`
- `data-slot="page-intro"`
- `data-slot="feed-card-title"`
- `data-slot="feed-card-meta"`
- `data-slot="entry-card-title"`
- `data-slot="entry-filters-source-unread-count"`：来源全部未读数，含 0，弱化行内数字
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

## 阅读元信息与来源未读数

`reader-meta-block` 内顺序为订阅名、可选作者、发布时间及独立 `open-original` 链接。
长文本允许换行；原文入口触控高度至少 44px。原生复用 Dioxus 的外部导航处理器，
Web 使用 `_blank` 与 `rel="noopener noreferrer"`，不触发阅读状态重置。
完整时间显示设备本地数值偏移，零偏移或回退显示 UTC；卡片与年月日分组共用加载时转换后的时间。

`data-layout="entry-filters-source-chip"` 继续保留。数字槽位
`data-slot="entry-filters-source-unread-count"` 使用弱化行内文本；控件可访问名称仍为来源名，
通过 `aria-describedby` 关联“未读 N 篇”，可见数字不重复朗读。
计数来自 `FeedSummary.unread_count`，不由当前文章集合计算；成功标记后重查订阅汇总，失败不预减计数。

## 按筛选批量已读

`EntriesCommand::PreviewMarkRead { query }` 冻结当前查询，忽略分页限制，返回排序后的未读 ID 集合；`ConfirmMarkRead { preview }` 在存储锁/事务内重新比较完整集合。变化时返回新预览并要求再次确认，未变化时一次批量写入。查询范围变化取消待确认预览；写入期间拒绝重复提交，已开始的写入不因筛选变化而取消。成功后加载当前查询、夹紧页码并 bootstrap 权威订阅计数。列表状态反馈放在筛选折叠区外。

稳定接口：`data-layout=entry-bulk-read`，`data-state=idle|confirm`，`data-action=preview-mark-filtered-read|confirm-mark-filtered-read|cancel-mark-filtered-read`。按钮忙态 disabled/aria-busy，触控目标至少 44px。

## 订阅自动发现

`FeedsCommand::AddFeed` 增加可选 `fallback_site_url`；RefreshPort 同步透传。host 调用 SubscriptionWorkflow.prepare_subscription，Ready 进入 add_prepared_subscription，NeedsSelection 返回订阅页候选。UI 不抓取或解析 HTML。候选到达时核对当前草稿；改输入取消候选，添加期间仍由 adding_feed_url 去重，成功只清除对应草稿。候选标题 / URL 作为文本渲染。

新增 `data-layout=feed-discovery-candidates`、`data-action=select-feed-candidate|cancel-feed-discovery`、`data-slot=feed-candidate-url`。既有 feed-form 原生 submit、add-feed 稳定接口保留。

## 刷新实际新增计数

RefreshStorePort.commit 返回 RefreshCommitOutcome.inserted_count；Updated.entry_count 保留解析条目数语义，新增 inserted_count 由存储实际插入结果提供，RefreshAllSummary 累加成功 Updated。批次 commit 的计数只有 end_batch 成功后才可发布；结束落盘失败沿用原逻辑把结果改为失败。SQLite索引/正文分库的既有部分失败边界不变。

host 的 RefreshAllExecutionOutcome 透传 inserted_count / total_count / failed_count，RefreshFeedExecutionOutcome 透传 inserted_count。shell区分成功、部分失败、全失败。成功3秒、错误6秒；单订阅页反馈通过 revision 与消息匹配避免旧定时器清除新结果。自动刷新静默、阅读页图标规则、RefreshFlight、批次收尾及稳定 data-* 接口均不变。

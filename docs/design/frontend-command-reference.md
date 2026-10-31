# 前端命令与界面接口清单

## 目的

这份文档记录 RSS-Reader 当前对外公开的前端命令面、导航语义和稳定界面接口。

它回答的是“现在有哪些命令和接口可以长期依赖”，而不是“为什么这样设计”。功能边界与设计原则本身见：

- [功能设计哲学](./functional-design-philosophy.md)
- [Headless Active Interface 设计目标](./headless-active-interface.md)
- [UI Shell / Bus / Page Facade 边界](./ui-shell-bus-page-facade.md)

---

## 命令边界

原生界面之前的交互控制器、当前宿主适配职责及验证入口见[原生界面前置模块边界](./native-ui-preparation.md)。目录重置/回复和阅读快捷键规则已独立为普通 Rust 模块；没有新增业务命令或改变本文件的用户操作语义。

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

当前页面通过 `UiCommand` → `execute_ui_command` → `UiIntent` 连接统一用例；
CLI 直接复用 `AppUseCases`，不依赖 UI 命令类型。实现边界见
[UI Shell / Bus / Page Facade](./ui-shell-bus-page-facade.md)。核对日期：2026-09-30。

### Home、搜索与阅读交互

顶部展开态为 `[R] [搜索图标] [RSS 图标] [齿轮图标] [收起箭头]`，Reader 最左侧增加返回图标。R 同时是 brand / Read / Home，没有独立“文章”导航按钮。

R 的可访问名称随实际动作变化：全局文章页为“Read / 刷新全部订阅”，其他路由为“Read / 首页”。`title` 在此基础上附加当前刷新反馈；名称变化不改变导航或刷新命令。

| 稳定接口 | Rust 语义 / 结果 |
| --- | --- |
| `data-action="activate-home"` | `resolve_home_action(AppRoute)`：只有全局 `EntriesPage` 返回 `ManualRefresh`；其他路由只导航到全局文章页 |
| `data-action="refresh-all"`、首页下拉刷新 | 与 Home 再次点击共用 `AppShellState::manual_refresh` → `ShellCommand::ManualRefresh` → 既有 `RefreshPort` / application 刷新用例 |
| `data-action="toggle-search"` | shell 的 `NavMode::Normal / Search` 切换；与 `Collapsed` 共用单一状态，收起时搜索操作不展开导航 |
| `data-field="entry-search"` | shell 持有并持久化搜索词；Enter 进入全局文章页按标题搜索，Esc 退出搜索态（输入法 composing 期间放行 Esc） |
| `data-nav="feeds"` / `settings` / `back` | 纯导航；返回按钮和 Android 系统返回复用 history / fallback 策略 |
| `data-action="entry-page-previous"` / `entry-page-next` | 既有 Entries reducer 更新并限制页号，保持筛选和分组，滚至新页起点 |
| `data-action="open-reader-image"` / `close-reader-image` | `ImageViewerState::Closed / Open`，复用已渲染图片地址；Esc、背景或 Android back 同样关闭 |

手动刷新在 App scope 中运行，同步取得 in-flight 状态，连续 R、下拉和订阅页“刷新全部”不会重复创建批次。页面卸载不取消该任务。完成或部分失败后 revision 只让列表与订阅快照重取；Reader 正文不订阅刷新 revision。自动刷新由 `rssr_infra::local_backend` 的循环调度；其中共享的 `RefreshFlight` 让自动与手动的重叠请求等待同一轮结果，不改变阅读页加载语义。进行中任务被取消时，等待者收到错误，下一次请求可以重试；不缓存已完成的刷新结果。

订阅探测与刷新在 infra 共用 8 MiB 响应限制，按解压后的流及转码后的 UTF-8 文本检查，超限走既有失败反馈并保留文章。SQLite 与 Web 的 `EntryContent.updated_at` 只在正文记录（含哈希）变化时更新；订阅刷新时间与文章索引更新时间仍正常更新。Web 仅在正文记录变化时发布正文片，逐订阅完整提交及新增数语义保持不变。没有新增 UI 命令或 `data-*` 接口。

默认样式在 Reader 中只通过 R 的进行中动画与成功 / 错误小标记显示刷新反馈，避免提示浮层盖住标题或正文；反馈期间 `manual-refresh-status` 的 live region 与 R 的完整 `title` 提示保留，其他页面显示文字结果。成功结果约 3 秒、错误结果约 6 秒后由 App 级状态清除，避免页面切换后重现旧提示；旧计时器不能清除新一轮刷新。此呈现差异不改变刷新命令或任务生命周期。

文章列表的 `LoadEntries` 查询保持页面生命周期，并以页面内 generation 校验结果：只有最新发起的查询可发布列表、归档计数和状态，旧查询晚到的成功或失败都被丢弃，避免刷新与筛选切换交错时显示错误结果。该校验不改变写入命令或全局刷新任务的生命周期。

归档截止时间在查询范围、归档设置或刷新代次改变时计算，普通重绘沿用同一时间参数；勾选、目录展开／收起和同一结果集内翻页不触发列表重查。查询期间若已读、收藏或批量写入成功，查询结果视为可能过期：丢弃旧成功／失败并确保按当前范围重新查询，不能覆盖已保存的标记或重新插入已不匹配的文章。没有并发查询时，单条标记仍直接更新当前列表。

首次列表查询还需等待设置、工作区偏好和来源映射全部就绪，避免先按默认分页或筛选显示，再跳到保存的设置。页面内 `Pending / Loaded / Unavailable` 区分等待、成功和读取失败：失败后允许读取文章并保留错误提示，但不保存回退偏好覆盖原值；后续刷新可重试。Bootstrap 不订阅自身的完成状态，避免成功后重复查询来源；独立的 bootstrap generation 丢弃其旧成功和旧失败结果，不与列表查询互相失效。

单篇已读和批量操作后的补充读取走完整的 `Reread`：保留偏好读取失败期间用户选择的筛选，也不丢掉被其替换的在途设置读取。恢复读取本身不保存这些筛选。记住刚打开的订阅属于 App 写入，快速返回不会取消；迟到失败沿用全局提示。

下拉刷新仅在全局文章页、页面已到顶部且向下拖动至少 80 CSS px 后松手触发；松手时须已无剩余触点，并按最终坐标重验距离和方向，不能沿用上一次移动的 armed 状态。横向、多指、文本选择、表单操作均排除；任一嵌套滚动祖先仍能消费当前移动方向时也排除，所有祖先都已到顶部时允许向下拉。没有可用触点坐标的事件取消手势，不保留 armed。被动 DOM bridge 只传当前坐标、页面是否到顶、控件/选区事实及嵌套滚动的双方向余量；`pull_refresh/gesture.rs` 独立管理起点、上一触点、排除条件、阈值、取消与释放，不依赖 Dioxus。嵌套消费按相邻触点方向判断，刷新距离按起点判断；松手也重验这两种距离。界面只订阅 idle / pulling / armed 阶段，刷新反馈复用 shell 的 refreshing / finished / error。没有字母 R 快捷键，也没有全局 `touch preventDefault`。

图片 bridge 仅对已消毒正文中的图片增加点击/键盘入口，不改变 sanitizer。Rust 持有 viewer 状态，原生 modal dialog 承担焦点约束和背景隔离；DOM 适配锁定并恢复滚动。正文、标题、metadata 的选择和复制沿用原生行为，带 modifier 的阅读快捷键继续放行；原生端 release WebView 允许系统右键菜单，不接管剪贴板。

Reader session 以 `entry_id + load_generation` 校验异步 UI 结果，切换文章再返回同一篇也不会接纳上次访问的迟到结果。图片本地化只更新缓存，不触发当前正文重载；下次打开文章使用新的本地引用，避免正文替换打断滚动、选区或图片查看器。

Reader 根节点的 `data-state` 由 Rust 中的当前文章身份、内容完成状态与错误共同决定：加载中为 `pending`，当前内容就绪后为 `loaded`，当前文章失败为 `error`。`aria-busy` 与 pending 同步；阅读位置的就绪判断复用同一状态，不能把“没有错误”直接当作“加载完成”。

位置记忆由 UI runtime 的 `reading_position` 管理，仅保留本次运行内的分页和坐标/锚点；没有 domain/application、数据库或 CLI 契约变化。列表只在初次加载成功后恢复分页，等待 presenter 与页码一致后才进行 DOM 恢复；主动筛选或翻页不会套用旧位置。以文章 ID 和视口偏移定位，找不到时回退到有效滚动范围内的原坐标。Reader 使用正文块锚点与偏移，顶部不因元信息高度变化而离开顶部。DOM bridge 提供测量、导航代次、时钟与用户输入事实，Rust 决定恢复、回退与停止，最多校正 2 秒；用户主动输入或导航会终止校正。图片 dialog 锁定滚动期间不记录临时坐标。

`reading_position/controller.rs` 独立承载保存集合与恢复规则，不依赖 Dioxus。只有保存集合在本次运行中共享；恢复状态和访问代次随每次适配器连接重建，避免重挂载后拒绝新消息。外层负责异步桥接、清理与 Android 返回前捕获。事件使用明确的 Rust 类型；字段完整的未知事件不打断接收，也不触发保存或实际滚动，只有布局事实可产生目标坐标。

返回列表的文章可有 `data-return-highlight="true"`，约 2 秒后移除；这是定位提示，不代表已读。`data-position-key` / `data-position-page` / `data-position-ready` / `data-position-entry` 是内部桥接字段，不作为用户主题接口。Web 页面刷新或应用结束清除位置，位置不参与配置交换。

位置采集发生在操作边界：DOM 捕获阶段的控件点击、阅读页上无修饰键的 J / K 与左右方向键切文（输入框与可编辑区域内、图片查看器打开时除外）、表单提交与浏览器 `popstate`。搜索框光标移动、带修饰键及输入法组词期间的按键不采集位置，阅读快捷键同样放行组词输入。Web host 在 Dioxus 启动前安装 `popstate` 转发，通过内部 `rssr-history-leave` 事件在旧正文卸载前采集。连续滚动不监听位置、不扫描正文；每次进入页面的首次滚轮／触摸／滚动按键输入只发送轻量取消事实。Rust 只保存 `capture`，不会以取消或布局探测覆盖已存锚点。桥接消息带序号，旧响应不能覆盖更新的输入或导航事实。Android 系统返回通过内部 `rssr-capture-position` 事件先取得快照，最多等待 500ms，失败仍继续返回；等待期间合并重复返回并在路由已变化时放弃旧返回请求。这些事件与消息序号均为内部 host bridge 协议，不是主题或 application/domain 契约。

`data-nav="entries"` 仍用于纯导航的“返回全部文章”链接；R 使用 `data-action="activate-home"`，不能把它当作纯导航选择器。旧 `show-top-nav` / `hide-top-nav`、`app-nav-brand-name`、`reader-toolbar` 已移除。旧 `nav_hidden` / `rssr-nav-hidden` 偏好仍被忽略，搜索词仍持久化；文章筛选折叠偏好已退役，筛选栏常驻。`app-nav-shell` 的 `data-state` 为 `normal` / `search` / `collapsed`。

`data-action="toggle-nav"` / `data-slot="app-nav-toggle"` 是顶栏最右侧的收起/展开按钮，`aria-expanded` 表示导航展开状态，`aria-controls="app-nav-content"` 指向内容区域。收起时只有箭头按钮可交互，刷新 live region 仍可向辅助技术报告结果。状态由 App shell 在本次运行内共享，跨页保留但不写入配置；收起关闭搜索模式但保留搜索词。Rust 负责显隐与状态，CSS 控制向左收拢后的 44px 占位和窄箭头外观。`AppNav` 实现集中于 `components/app_nav.rs`，原 `app::AppNav` 导出保持兼容。

搜索输入框保持 shell 级状态；窄屏或持久化的旧版侧栏 CSS 把导航压窄时，导航行允许换行，输入框占满下一行。换行预算包含收起按钮预留的 48px，避免新增箭头把输入框挤窄。

文章列表在筛选栏下显示 `entry-filter-summary`，其中 `entry-filter-summary-text` 概括当前搜索、已读、收藏与来源条件；单订阅页不显示未参与查询的来源偏好。`clear-entry-filters` 清空 shell 搜索和上述状态条件，重置第一页，沿既有查询、偏好保存和清空选择路径生效；分组、归档显示偏好与路由订阅范围保留。列表选项保留清除入口；有生效筛选且无匹配结果时，`entries-page-state[data-state="empty"]` 内另有同一动作的“清除筛选”按钮，操作后焦点回到可用的搜索框或列表选项。自动化应限定所在区域，不能假定该动作只有一个节点。空结果继续区分没有订阅、订阅尚无文章、文章已归档及当前筛选无匹配；没有生效筛选时不添加这个清除按钮。首次文章查询成功后才显示空结果说明；加载中或首次读取失败时只显示相应状态，不推断文章库为空。

来源选择使用 `open-source-filter` 打开原生 dialog，Rust 管理草稿；全部／部分入口分别显示“全部来源”／“…”。完整名称在可搜索、可滚动的勾选列表内显示。旧 `entry-filters-source-chip` 与来源未读数槽位已退役；统计移至订阅页。分页只渲染一份 `entry-pagination`，位于页面 panel 的同级，固定于视口下方并预留 safe-area / 内容末尾空间。

### 阅读工作区（2026-10-24）

- `select-entry`、`select-current-page`：Rust reducer 只接受当前页 ID。`data-selected` 与 `data-read` 分别表示勾选和阅读状态；master 的 `aria-checked` 可为 `mixed`。16px 方框配 44px 点击区域，键盘焦点单独可见。
- `mark-selected-read`、`archive-selected` → `EntriesCommand::ApplySelection` → `EntriesListService::apply_entry_batch`；在点击时冻结显式 ID，存储层原子更新。没有选择时禁用，执行中禁止修改勾选。旧 UI 筛选全量预览按钮已退役；CLI 和旧 host 预览接口仍保留。
- 单篇已读／收藏写入期间，该行两种状态操作暂时禁用；当前文章工作区有单篇写入待完成时，批量操作也暂时禁用。批量操作期间禁止发起单篇写入。Rust session 同步检查忙碌状态，拒绝已排队的重复事件；成功或失败均恢复操作，其他行在单篇写入期间仍可使用。
- 已接受的单篇／批量文章写入由 App 持有，切页不取消。回执只更新发起操作的页面；该页已卸载时由全局提示报告结果，并重新读取当前页面。结果未知不自动重试。写入后的补充查询仍属于原页面，离页即取消；关闭整个 App 结束客户端等待。
- `group-by-source` / `group-by-time` 是单选按钮组；状态筛选四个 icon button 以 `aria-pressed` 表达选择，再次点击取消该条件。
- 工作区偏好任务执行前先核对页面当前选择，丢弃已经过时的捕获值；比较与写入在共享 `EntriesWorkspaceService` 中串行完成。快速开启再关闭筛选、切换分组时，后一请求须等前一次保存结束后再判断是否有变化；前一次保存失败也释放等待，避免“恢复全部”被误判为无需保存。
- 偏好保存与来源 URL 编辑共享 App 内的写入顺序；新文章页的初始读取／重新读取先等待此前写入完成，再读取偏好基线，避免将编辑前的来源地址再次保存。读取期间不占用写入锁，已有请求代次机制仍负责丢弃迟到结果。
- 来源分组按 `feed_id` 区分身份，自定义名称仅用于显示。目录来源锚点、列表分组标识不依赖名称，同名来源和名称含不同标点的来源不会被合并或产生锚点冲突。目录继续使用完整查询结果的分组投影；全选仍只作用于当前页。
- `open-source-filter` / `apply-source-filter`：取消、Esc、点外侧只关闭草稿；应用后清空文章选择，空来源集合不允许提交。页面里的“全部”偏好仍用空 URL 列表表达。
- 来源弹窗的全选框同时提供未选／部分／全选状态；Rust 计算 `aria-checked`，DOM 仅同步原生复选框的 `indeterminate` 属性。部分选中时点击或空格选中全部，再次操作取消全部。
- 来源弹窗在高度不足时优先缩短来源列表，列表内部滚动；标题、搜索、全选和底部取消／应用按钮保留可用空间，避免为了操作底部按钮再滚动整个弹窗。
- `open-directory` / `close-directory`：目录可以收起，≤1000px 默认收起为入口。目录浏览和同页定位保留选择；换页、筛选、搜索或切换视图清空选择。原有目录 Rust 状态与分组 memo 分离，选择不重建分组树。
- 目录隐藏时卸载导航树，关闭的来源／月份只保留父级控制；完整目录投影、当前路径与手动展开状态仍由 Rust 保存。重新显示时，手动浏览恢复原目录滚动位置，跟随模式对齐当前位置。DOM adapter 只测量实际可见性与滚动位置，沿用 epoch／revision 提交确认后执行恢复；隐藏状态的尺寸变化不能清除手动浏览位置。目录显隐与展开不派发文章查询、不改变勾选。
- 大文章分支按视口挂载可变高度行，完整目录引用仍保留；未挂载文章的定位、目标页和键盘访问均使用原身份。窗口移动不更改展开/跟随或勾选规则，焦点行保持稳定；隐藏后恢复手动目录位置。实现边界与测量限制见 `native-ui-preparation.md`。
- `edit-feed` / `save-feed-edit` 仅在 Subscribe；`FeedsCommand` 经 runtime 调用 `Backend` / `FeedEditService`，Rust 管理加载、草稿、校验和忙碌状态。native dialog 只负责焦点／模态，DOM 测量负责定位。
- 来源编辑的异步结果由 `feed_id + edit_generation` 校验，关闭后重新打开同一来源也不会接纳上次访问的迟到结果。打开编辑取消旧的删除／覆盖导入确认；加载中可以关闭，保存中阻止重复提交以及关闭按钮／Esc／Android 返回的弹窗关闭操作。
- 编辑身份、草稿和 gate 由页面内 `FeedEditState` 统一管理，原来的状态与校验语义不变。原生 dialog 的 `cancel` 与键盘 Esc 都经 Rust 关闭入口，覆盖保存按钮禁用后焦点移到 body 的情形，避免仅关闭原生弹窗而保存仍在进行。
- 来源筛选和来源编辑中，输入法组词期间的 Esc 不关闭弹窗、不丢弃草稿；同时阻止 native dialog 的默认关闭，避免 Rust 状态仍打开而原生弹窗已关闭。组词结束后的 Esc 沿用取消逻辑，名称／URL 与来源勾选草稿都不自动保存。
- Android 系统返回沿用现有的 dismiss 优先路径：先关闭来源筛选、目录浮层或来源编辑，之后才返回上一页；来源编辑保存中消耗返回操作并等待结果。Web 的 Esc／取消行为保持不变。实机覆盖范围见本轮交接，编译与状态测试不替代系统返回验收。

Android 外壳通过 `OnBackPressedDispatcher` 将系统返回经 JNI 放入有界输入队列，Rust 页面在自身 UI 任务中执行上述 dismiss / history / fallback 规则。页面卸载仅注销自己的监听代次，不能清掉新页面的监听；积压的重复输入合并。返回前捕获的忙碌标记使用跨线程共享的原子状态，因为 WebView 点击回调与 Dioxus 异步任务可能在不同线程执行；完成、超时或取消都释放标记，不能让下一次返回永久失效。WebView 不消费片段历史，Java 线程不读写 Dioxus 状态。根页无可返回目标时只把任务移入后台，不通过销毁 Activity 模拟返回。

返回选择与首页提示由 `hooks/use_mobile_back_navigation/policy.rs` 私有策略管理，hook 只收集事实、发布显示投影和执行副作用。首页忽略历史，首次系统返回显示提示，5 秒内再次返回才退后台；超时后重新开始。提示代次拒绝旧计时器，恢复前台清除提示。工具栏保持同步导航，页面浮层及保存门禁仍由原页面拥有；这次模块提取不增加公开命令或宿主协议。
- `stats-grid` 中 `data-stat=feeds|entries|unread|archived` 是全局统计；归档计数为手动与自动归档的并集，不重复计数。

`library.sqlite3` 的增量迁移 `0002_manual_archive.sql` 增加 `entries.is_archived`；`custom_title` 使用 v0.2 基线既有字段。已发布基线与根目录 v0.1 历史迁移保持原样。原生、正式 Web 与 Pages 共用 SQL 事务；批量写入仅处理已验证的显式 ID，单次提交失败整批回滚。来源编辑保持 feed ID，地址变化时清除旧缓存验证器并提升 generation，迟到的旧刷新不能覆盖新地址；工作区已选 URL 同步迁移。配置格式未增字段：JSON `title` 和 OPML 显示名导入时作为自定义名，刷新不会覆盖；导入不覆盖已有的远端原名，新导入来源在抓取前不伪造远端原名。重新导入后仍可用“使用原名”恢复已有远端名称。

新增同版本 HTTP 路由 `POST /api/v1/entries/batch`、`GET /api/v1/feeds/{feed_id}`、`PATCH /api/v1/feeds/{feed_id}`，沿用认证、CSRF、build ID、请求体上限与统一错误处理。`LocalBackend` 和 `HttpBackend` 实现同一接口；未新增 crate 或扩大配置同步范围。

### 订阅相关

- 添加订阅
- 删除订阅
- 取消尚未提交的删除确认
- 刷新单个订阅
- 刷新全部订阅

`feed-form` 使用原生 submit；地址输入 Enter 与 `data-action="add-feed"` 调用同一添加命令。`refresh-all` 位于 `exchange-header[data-section="saved-feeds"]` 的“已保存订阅”标题旁，不属于新增订阅表单，也不提交地址输入。

订阅页“刷新全部”直接读取 shell 的共享刷新状态；任一入口启动刷新后，该按钮同步显示“正在刷新…”、原生 `disabled` 与 `aria-busy="true"`，结束后恢复。页面不另建刷新中的局部状态。

`remove-feed` 第一次点击只进入该来源的确认态，显示来源名称、删除本地文章的后果、危险色“确认删除”和就近“取消”。`cancel-remove-feed` 经 facade/session 取消匹配来源的未提交确认，并把焦点还给该行删除按钮；再次删除需要重新确认。确认提交后进入 `busy`，显示“正在删除…”并移除取消入口；同页所有删除入口暂时禁用，该行编辑／刷新也禁用，reducer 拒绝排队的重复删除。原命令的成功或失败回执释放忙碌状态，失败后的重试仍需重新确认。取消不撤回已提交的删除，也不新增恢复或撤销能力；原有其他操作及快照变化清除待确认状态的规则保留。

Feeds reducer 用正在提交的地址 `Option<String>` 同步去重，按钮以 disabled / `aria-busy` 提示；输入仍可编辑，成功仅清空未变化的已提交地址，失败释放 pending 并保留输入。`LoadSnapshot` 以页面内 query generation 丢弃旧查询的成功和错误，避免刷新、添加与删除交错时旧统计覆盖新结果。两者均属于共享 Rust 页面交互，不改变 application 用例或增加平台分支。

单来源刷新在整批结果发布后捕获当前提示，普通提示约 3 秒、错误或结果未知提示约 6 秒后清除；只有提示 revision 和正文都仍匹配时才生效，旧计时器不能清掉后来出现的删除确认。快照身份在命令构造前同步更新，过期结果整批丢弃，包括状态提示和重载请求；编辑及写入结果不受后来快照查询影响。快照查询属于页面作用域；单来源刷新、添加、配置导入、来源保存与“刷新全部”保持主线 App 作用域及离页回执。

### 阅读相关

四个主要页面的主标题为 `h1`，应用自身的小节标题按层级递进；Reader 正文中的来源 HTML 保留内容标题语义，不作为应用导航层级改写。字号由稳定的 `data-slot` 样式控制。

`data-layout="reader-shortcut-scope"` 是可用 Tab 进入的具名 `region`，提供 M / F / J / K / 左右方向键说明和可见焦点。快捷键监听整个阅读页，不要求焦点在阅读区（`pages/reader_page/webview/reader_shortcuts.js`）；输入框与可编辑区域内、带修饰键、输入法组合中、自动重复以及图片查看器打开时的事件放行。底栏固定在视口底部，渲染在阅读页之外，以免主题的 `backdrop-filter` 困住 fixed 定位。收藏按钮的固定名称与 `aria-pressed` 搭配，底栏四个装饰图标均为 `aria-hidden="true"`。

按键与动作统一定义在普通 Rust `ui/reader_shortcuts/policy.rs` 的 `BINDINGS`，当前 WebView 由同模块的适配代码生成静态配置并共用 `shortcut_input.js`。页面专用 hook 归 `pages/reader_page/shortcuts.rs`，共享阅读位置不引用页面模块。切文位置仍在捕获阶段测量，保留对已被阻止事件的原采集行为；冒泡阶段才检查 `defaultPrevented` 并同步阻止默认动作。此区别、作用域及受控输入验收见[快捷键适配边界](./native-ui-preparation.md#reader-快捷键定义与同步边界2026-10-30)。

动态操作结果使用持续挂载的 `StatusBanner` live region：`role="status"`、`aria-live="polite"`、`aria-atomic="true"`。空消息区域保留在可访问树中但不占布局空间；加载/成功/错误更新文本时不抢焦点。静态空列表或归档说明不参与播报。颜色状态继续使用 `data-state="info|success|error"`，不把错误色自动映射成打断式通知。

Reader 的收藏 / 已读成功反馈通过按钮状态和 `sr-only` live region 呈现，正文不因成功提示重排；错误仍为可见状态条。标记命令携带目标值（`SetRead` / `SetStarred`），结果区分已改变、本来就是与文章不存在；文章不存在时以错误色调的可见状态条提示。上一 / 下一未读按钮及方向快捷键只选择未读目标，无目标时按钮禁用并提供 title 说明；同订阅文章导航沿用独立按钮，不作为未读操作的回退。

用户触发的已读／收藏写入由 App 持有，返回列表不会取消。回执仍校验原来的 `entry_id + load_generation`；原访问结束后仅显示全局结果并触发当前页面重新读取，不把旧结果写入新访问。关闭 App 结束客户端等待，未知结果不自动重试。正文读取与自动图片本地化仍随阅读页结束。

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
- 预览当前 CSS
- 导入主题文件并预览
- 导出当前 CSS
- 从主题卡片选择并预览内置主题
- 恢复默认主题的预览
- 上传 WebDAV 配置
- 下载 WebDAV 配置

列表密度选择已移除：界面使用原默认间距，旧 `list_density` 仅为 0.2.x API/配置兼容字段，UI 不编辑、不预览、不提交该字段。旧标签页草稿恢复时丢弃该项，保留其他逐字段编辑；0.3.x 删除参数，见[退役计划](../maintenance-backlog.md#03x-移除旧列表密度参数)。

设置页采用“本页预览 → 显式保存”的统一模型。主题卡片 `apply-theme-preset`、`apply-custom-css`、导入 CSS 和 `clear-custom-css` 都只修改草稿及本页预览，不写入资料库，也不顺带保存其他字段。浅色／深色与阅读字号的草稿修改可即时预览；CSS 文本输入本身不注入样式，须点击“预览当前 CSS”或显式保存。预览前校验 CSS，失败保留当前预览。

内置主题入口合并为默认主题及四个预设的卡片；`aria-pressed` 与 `theme-card[data-state="active|inactive"]` 表示草稿选择，可访问名称包含可见按钮文字及主题名。CSS 编辑和文件操作位于可展开的 `theme-advanced`。旧 `apply-selected-theme`、`remove-theme-preset`、`preset-theme-select`、`theme-preset-selector` 与 `theme-preset-quick-actions` 已退役；恢复默认统一使用 `clear-custom-css`，不再为每个预设提供移除按钮。

`save-settings` 是上述草稿的统一保存入口，位于 `settings-save-region`，与未保存／保存中状态、结果 live region 和离页提示相邻。该区域的 `data-state` 为 `clean | dirty | pending`；结果的成功／失败色调仍由 `StatusBanner` 表达。保存共用同步 pending gate，一次只提交点击时已编辑字段的补丁；期间的新编辑继续留在草稿中，完成提示明确其尚未保存。失败保留草稿及当前预览。WebDAV 端点和远端路径仍是会话字段，不由这个按钮写入配置；上传／下载沿用独立命令。

预览与实际已存设置分离，不增加资料库写入版本。普通页内导航离开设置时丢弃该页草稿及预览，恢复实际已存外观；旧页面的迟到预览或清理不能覆盖新页面。正式 Web 的版本更新／重新登录恢复流程仍按既有规则暂存草稿，返回设置后待用户保存，不把该恢复流程改成普通导航丢弃。

已保存的内置主题 CSS 是当时的文本快照，升级保留原样并继续识别旧版本身份。要采用新版样式，重新点击对应卡片预览，再点“保存设置”；旧版本显示“当前已选”不代表已替换成最新 CSS。以上变更属于页面呈现与 session，不改变 settings application 用例或配置格式。

保存按主线 SettingsPatch 逐字段提交，只有编辑过的 CSS 参与界面语法校验；存储、已提交编辑和后来编辑按 stored → submitted → pending 投影。save_policy.rs 负责补丁准备及回执汇总，session 保留主线门禁、失败恢复和任务作用域。保存成功清除 submitted，后续 pending 继续保留；失败把 submitted 合并回 pending 下方。

初次加载和下载只更新 stored，不覆盖尚未保存的逐字段编辑。读取受最新代次与成功写入版本约束；写入期间的读取回执暂存，成功写入丢弃，失败且没有更新读取时再应用。跨刷新/登录的草稿恢复沿用主线。

保存与实际下载共用同步 gate；下载确认及目标捕获保持主线规则，保存中被拒绝的下载保留确认。上传和下载属于 App，离页仍完成回执并触发重新读取，更新实际存储主题。数值解析提示按字段投影代次退休，未被存储结果供应的编辑字段保留提示。

主题文件导入先检查 UTF-8，再校验 CSS，仅更新草稿和本页预览；需要点击“保存设置”才提交。读取失败、编码错误和无效 CSS 不覆盖草稿或已有预览，显式保存失败则保留导入草稿供重试。导出使用点击时输入框中的内容，不提交设置。Web 取消选择保持现有提示，桌面取消显示取消提示；离页取消页面等待，Web 同时中止未完成的文件读取。系统选择器的结束与已发生的写入仍由宿主负责。

主题导入在打开选择器时取得页面身份；后来的导入、设置草稿编辑、预设选择、预览/清空或显式保存会使旧读取失效，取消新选择器也不会恢复旧请求。失效结果不改草稿、不提交设置、不覆盖提示。加载/保存回填实际改变 CSS 时也会使旧导入失效，完全相同的回填和单纯状态提示不会。已开始的保存仍按公共 pending gate 完成；期间导入的新草稿保留，需再次明确保存。

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
- `rssr-cli remove-feed <feed-id> [--keep-entries]`
- `rssr-cli refresh --all`
- `rssr-cli refresh --feed-id <id>`

`refresh` 必须且只能指定其中一个目标；参数无效时在打开本地数据库前退出。

`remove-feed` 默认连同该订阅的文章一起删除；`--keep-entries` 只删订阅、保留已抓取的文章。

### 配置交换相关

- `rssr-cli export-config [--output <path>]`
- 文章页的来源筛选：没有所选来源表示全部订阅；所选来源都已删除时结果为空并提示，批量已读不会扩大到全部订阅。按订阅打开文章页时单独记住“上次打开的订阅”。
- `rssr-cli import-config <file>`（读取当前配置指纹后立即替换；界面的导入则携带订阅页快照的指纹，配置在载入后被改过时拒绝并重新载入）
- `rssr-cli export-opml [--output <path>]`
- `rssr-cli import-opml <file>`
- `rssr-cli push-webdav <endpoint> <remote-path>`
- `rssr-cli pull-webdav <endpoint> <remote-path>`

### 设置相关

- `rssr-cli show-settings`
- `rssr-cli save-settings ...`

结构化导出与 `show-settings` 的 stdout 只含结果数据；诊断日志写入 stderr。导出可直接重定向后再导入；失败返回非零退出码。

---

### 批量已读与订阅发现

- `rssr-cli mark-read --all [筛选参数]` 或 `--feed-id <id>`（可重复）；默认只预览，加 `--yes` 执行。
- 筛选参数为 `--search`、`--read-filter all|unread|read`、`--starred-filter all|starred|unstarred`、`--archive-filter active|all|archived`。
- `add-feed <url>` 同样接受网站首页；多个候选列出 URL 后非零退出。`--skip-refresh` 仍验证订阅，只跳过文章入库。
- `save-settings` 的参数以 `--help` 为准；当前 CLI 没有设置文章每页数量的参数。只提交给出的参数（设置补丁），未给出的字段保持资料库中的值；合并、校验与保存在同一次写入中完成。自定义 CSS 的三个参数同时给出时，文件优先于 `--custom-css`，再优先于 `--clear-custom-css`。
- 当前参数定义与执行入口均在 `crates/rssr-cli/src/main.rs`，不存在独立 `args.rs`。

## 页面级接口

路由为 `/`（启动路由）、`/entries`、`/feeds`、`/feeds/:feed_id/entries`、`/entries/:entry_id`（阅读）、`/settings`；不存在 `/reader/:id` 路由。

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
- `data-action="select-feed-candidate"`
- `data-action="cancel-feed-discovery"`
- `data-action="remove-feed"`
- `data-action="cancel-remove-feed"`
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
- `data-action="apply-custom-css"`
- `data-action="export-custom-css-file"`
- `data-action="import-custom-css-file"`
- `data-action="apply-theme-preset"`
- `data-action="clear-custom-css"`
- `data-action="open-github-repo"`
- `data-action="activate-home"`
- `data-action="toggle-search"`
- `data-action="toggle-nav"`
- `data-action="show-entry-controls"` / `hide-entry-controls`（已退役）
- `data-action="clear-entry-filters"`
- `data-action="select-entry"` / `select-current-page` / `mark-selected-read` / `archive-selected`（替代旧的按筛选全量预览操作）
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
- 映射到 Rust 动作语义；多个 UI 入口可派发同一命令

而不是：

- 业务逻辑本体
- 页面私有临时点击逻辑
- DOM 结构的替代命名
- 容器或展示位本身的标签

当前 UI 命令族为 `Shell / Entries / Reader / Feeds / Settings`；配置交换属于设置命令。

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
- `data-density`（0.2.x 兼容常量，固定为 `comfortable`；0.3.x 移除）

通用 class 仍然保留为公开界面接口，但范围应收敛在真正通用的壳和组件上：

- `.app-shell`
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
- `data-field="entries-page-size"`
- `data-field="startup-view"`
- `data-field="refresh-interval"`
- `data-field="archive-after-months"`
- `data-field="reader-font-scale"`
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
- 订阅行与其操作组：`data-state="idle" | "confirm" | "busy"`
- 设置保存区：`data-state="clean" | "dirty" | "pending"`
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
- `data-slot="page-section-header"`
- `data-layout="stats-grid"`
- `data-layout="feed-workbench-single"`
- `data-layout="exchange-grid"`
- `data-layout="settings-grid"`
- `data-layout="entries-layout"`
- `data-layout="entry-groups"`
- `data-layout="entry-directory-rail"`
- `data-layout="entry-top-directory"`
- `data-layout="entry-filters"`
- `data-layout="entry-filter-summary"`
- `data-slot="entry-filter-summary-text"`
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

## 批量操作边界

文章 UI 使用显式 ID 的 `ApplySelection`，只操作当前页勾选项，规则见上文“阅读工作区”。旧 `EntriesCommand::PreviewMarkRead` / `ConfirmMarkRead` 和对应 UI selector 已退役。

CLI 保留按筛选全范围的预览／确认用例，存储锁或事务内重新比较完整未读集合；匹配变化则重新预览。`active` 排除手动及按时间归档项，`archived` 包含二者并集。该 CLI 行为不得移作文章 UI 的默认批量范围。

## 订阅自动发现

`FeedsCommand::AddFeed` 增加可选 `fallback_site_url`；RefreshPort 同步透传。host 调用 SubscriptionWorkflow.prepare_subscription，Ready 进入 add_prepared_subscription，NeedsSelection 返回订阅页候选。UI 不抓取或解析 HTML。候选到达时核对当前草稿；改输入取消候选，添加期间仍由 adding_feed_url 去重，成功只清除对应草稿。候选标题 / URL 作为文本渲染。

新增 `data-layout=feed-discovery-candidates`、`data-action=select-feed-candidate|cancel-feed-discovery`、`data-slot=feed-candidate-url`。既有 feed-form 原生 submit、add-feed 稳定接口保留。

## 刷新实际新增计数

`RefreshStorePort.commit` 返回实际新增计数。刷新响应的校验、文章索引、正文和抓取诊断在共用 SQL 事务中提交；失败回滚，迟到响应按 generation 和刷新票据拒绝。正式 Web 使用服务器资料库，Pages 使用同一 SQL 的私有内存库；浏览器不保存文章分片或跨标签页提交记录。网络请求不占用数据库事务。当前边界以 `v0.2-architecture.md` 与 `crates/rssr-infra/src/store/AGENTS.md` 为准。

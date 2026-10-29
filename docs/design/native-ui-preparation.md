# 原生界面前置模块边界

核对日期：2026-10-29。本文件记录当前阅读分支已经具备的交互模块及剩余边界，不代表已替换 WebView，也不改变七个 crate 的职责。

控件接入时的输入、回填、取消与宿主职责见[原生控件适配契约](./native-control-adapter-contracts.md)；当前证据等级、平台缺口和首批 Windows 系统文件流程见[原生控件验收清单](../testing/native-control-acceptance.md)。

## 已有可独立测试的 Rust 规则

| 模块 | 负责的规则 | 当前界面适配职责 |
| --- | --- | --- |
| `ui/reading_position/controller.rs` | 本次运行的位置保存、访问代次、锚点回退、恢复期限、用户输入取消 | 页面测量、时钟、实际滚动、导航前捕获、生命周期 |
| `pages/entries_page/pull_refresh/gesture.rs` | 触点历史、方向、嵌套消费、阈值、释放和取消 | 触摸监听与 DOM 滚动余量测量 |
| `components/entry_filters` 的 `SourceFilterDraft` | 来源草稿、搜索、选择、取消、来源集合变化 | 表单、焦点、弹窗、原生半选外观 |
| `pages/entries_page/directory/controller.rs` | 上下文重置、活动路径、展开集合、跟随/手动、消息校验、回复和导航请求 | presenter 身份、Dioxus signal、事件桥接、渲染确认、滚动执行 |
| `pages/entries_page/directory/window.rs` | 可变行高缓存、可见范围、占位高度、测量变化时的锚点补偿、未挂载目标与焦点保留 | 当前挂载行与视口测量、同步键盘默认动作、提交确认后的滚动/聚焦 |
| `hooks/reader_shortcut_policy.rs` | 阅读快捷键、修饰键/输入法排除 | 将平台键盘输入映射为普通 Rust 值，执行导航和 session 命令 |
| `pages/entries_page/request_lifecycle.rs` | 初始化/查询的独立代次、偏好就绪准入、旧结果过滤、写入后重查决策、列表到达后的返回页码顺序 | 页面任务作用域、命令执行、位置存储读取、signal 发布与既有查询触发 effect |
| `pages/entries_page/write_lifecycle.rs` | 单条/批量写入准入、选择快照、批量结果身份、发布/解锁/来源刷新顺序 | 同步派发开始状态、页面任务执行、intent 发布、来源刷新 |
| `pages/entries_page/preferences_policy.rs` | 捕获与当前偏好就绪检查、任务开始前的选择一致性检查 | effect 触发、保存任务执行、错误发布；应用服务负责串行比较与持久化 |
| `pages/reader_page/result_lifecycle.rs` | 文章 ID 与加载代次共同确定结果归属、整批迟到结果过滤、接纳结果保序移动 | 同步派发 BeginLoading、页面任务作用域、命令执行和 intent 发布；既有 reducer 持有加载代次 |
| `pages/settings_page/save_policy.rs` | 保存快照的 CSS 校验/零页大小回退、最后结果归并、整份草稿与主题选择回填判定、完成提示 | 同步 pending gate、提交快照捕获、页面任务作用域、命令执行、草稿/主题/提示 signal 发布 |
| `pages/settings_page/request_lifecycle.rs` | 初次加载身份、草稿/预设编辑代次、保存与下载共用的写入准入 | effect 派发加载、页面任务、结果投影、草稿与已保存主题的分别发布；不存另一份设置 |
| `pages/settings_page/sync/state.rs` | 下载的再次确认、输入/上传取消确认、提交时的端点与路径快照 | session 显示确认提示、执行原页面任务、按序投影 Settings 结果；host/infra 校验目标并执行 I/O |
| `pages/feeds_page/request_policy.rs` | 快照查询代次分配与结果准入、刷新命令归属、单来源刷新提示时长与清除身份 | 原查询 signal、同步命令构造、页面任务作用域、按序发布结果、实际等待；Shell 持有全局刷新 |
| `pages/feeds_page/edit_state.rs` | 编辑身份与代次、加载结果准入、草稿变更、保存 gate、关闭与失败重试 | reducer 串接命令及页面提示/reload，session 执行任务；dialog 负责焦点和原生取消事件 |

这些模块不直接查询数据库或网络。业务动作继续走 facade/session → `UiCommand` → runtime → application/host。前端交互状态不下沉到 application/domain，不建立另一份文章数据或第二套业务状态。

## 文章工作区请求生命周期的使用约束

- `EntriesRequestLifecycle` 只持有初始化与查询两种代次，二者互不失效。页面状态、写入 revision 和完整文章投影继续由既有 `EntriesPageState` 持有；每次请求只捕获少量标量，不克隆列表。
- 初始化开始时根据当前偏好状态决定是否加载偏好：Pending / Unavailable 加载，Loaded 保留。旧初始化的成功和失败都被拒绝；所有来源摘要仍按原命令加载。
- Pending 不准入文章查询，不轮询 Future；Unavailable 允许读取文章，保留既有错误提示和禁止保存回退偏好的规则；偏好保存准入见下节。
- 查询完成先校验查询身份，再比较 `flags_revision`。旧身份直接丢弃；当前身份若遇到已完成写入，则在尚无新 `reload_revision` 时请求重查，否则仅丢弃。单独增加 reload revision 不等于使查询身份失效。
- 接纳后按原顺序输出 intent，仅在 `SetEntries` 后插入捕获的返回页码，由现有 reducer 对新结果集约束页码。失败结果、被拒绝结果都不能恢复页码；被拒绝的结果载荷不遍历、不复制。
- Dioxus 适配器以非订阅读取获取当前事实，先同步取得请求身份，再启动原页面作用域的任务。查询完成时的借用在发布前释放；页面销毁继续取消未完成任务。初始化和查询状态不进入 presenter 输入，也不新增自动查询循环。
- 单条/批量写入和偏好保存使用下面的独立规则。归档时钟、页面查询与偏好保存 effect 继续留在当前装配层，保留 facade/session 和 `UiCommand` 边界。

## 文章写入与偏好保存规则的使用约束

- `write_lifecycle.rs` 不保存另一份 busy 集合或 revision；唯一状态仍在 `EntriesPageState`，由既有 reducer 转移。session 必须同步完成准入检查和开始状态发布，再启动异步任务；按钮禁用不能替代该检查。
- 单条写入只阻止同一 ID 的重复操作以及与批量操作重叠，其他 ID 可并行。结果按原顺序发布，随后释放本行 busy；仅收到已读变更时，才在解锁后刷新来源摘要。失败、空结果和仅收藏变更均解锁；没有已读变更就不刷新来源。
- 批量命令在用户操作时取得明确所选 ID 的独立快照，空选择不发命令。存在任何单条写入时拒绝启动，不排队；`BeginBulk` 仍由 reducer 分配代次，随后捕获该代次校验结果。旧成功和旧失败都不能覆盖后来的批次，拒绝时不遍历结果载荷。
- `CancelBulk` 沿用 reducer 的原语义：正在写入的批次不会因筛选/查询上下文切换而取消。成功结果先发布 `SelectionApplied`，再刷新来源摘要；失败释放 busy、保留当时的当前选择并允许重试，不把请求快照重新写回选择集合。revision、成功提示和重查仍由 reducer 与现有查询 effect 协作。
- `preferences_policy.rs` 只检查资格：调度时要求捕获的偏好已就绪且当前仍 Loaded；任务开始时再核对当前就绪状态和分组、归档、已读、收藏、来源 URL 列表。过期任务不调用保存；URL 比较保留现有顺序，不另行规范化或复制列表。
- 偏好比较与写入继续由共享 `EntriesWorkspaceService` 串行完成；页面不增加保存队列、缓存或去重状态。保存失败继续发布原错误，后续选择仍能保存；初始化失败的回退默认值不能覆盖存储。
- 页面销毁继续取消其未完成 UI 任务；不把此行为描述成回滚已提交的存储。`WriteEffect` 仅表达本页面的状态发布与来源刷新，不是通用命令框架，也不进入 domain/application。

## Reader 异步结果归属的使用约束

- `ReaderResultScope` 每次仅捕获命令所属文章 ID 和现有 `load_generation`。唯一页面状态仍为 `ReaderPageState`，代次仍由原 `BeginLoading` reducer 增加，不新增 signal、计数器或正文副本。
- 加载先同步发布 `BeginLoading`，再捕获结果归属并启动原页面作用域任务；已读、收藏和图片本地化命令捕获当前访问的同一代次，不互相作废。同篇重载、A→B 和 A→B→A 都使原访问的结果失效。
- 完成时同时比较文章 ID 和代次，对整个结果批次作一次准入判断，包括自身不带 ID 的状态提示。拒绝时不遍历载荷；接纳时保持原顺序并移动正文数据，不另行收集或克隆结果。状态借用在 session 发布前结束。
- 这是结果发布规则，不是命令取消或持久化回滚机制。旧访问发起的写入可以已在存储中成功，但其迟到提示不能进入新访问；页面销毁仍由 Dioxus 取消未完成 UI 任务。
- reducer 中按文章 ID 检查正文、错误和标记的防线保持原样。同篇重载的提示保留、切文清空提示、正文节点复用、导航和快捷键作用域均不改变。图片本地化继续仅写缓存，完成后不替换正在阅读的 HTML。

## Settings 保存规则的使用约束

- `save_policy.rs` 是页面内的纯规则，不持有 signal、任务、持久化句柄或另一份长期草稿。CSS 校验仍调用 domain 的唯一实现；其余数值校验继续由 application 执行。只有页大小 0 在提交副本中回退为默认值，原草稿先保留。
- session 必须先同步检查公共 pending gate，再捕获草稿和预设；通过准备后置 pending，并在 `spawn` 之前构造保存命令。按钮禁用不能替代该入口；保存期间没有自动队列，后续修改需要再次明确保存。
- 返回后先清除 pending，再投影和归并结果：最后一个 `SettingsLoaded` 与最后一个状态消息分别生效。有设置结果即按成功处理，不以状态 tone 推断成败；没有设置结果（包括空结果）保留草稿并提示重试。
- 当前整份草稿仍等于提交快照时，才用保存结果替换；不逐字段合并。改动后又改回原值仍视为相等，保存回填不使用加载/下载的编辑代次。当前草稿已等于保存结果时不提示未保存。
- 预设选择与草稿分开判断：只有选择仍等于提交时的选择才回填，检测依据是替换后的有效草稿 CSS。仅选择新预设而未应用，不单独计入未保存编辑；用户的新选择必须保留。
- 发布顺序继续为草稿、预设、已保存的全局主题、完成提示。页面销毁仍由 Dioxus 取消自己的任务，不承诺回滚已提交持久化。可变 pending 状态仅在页面 `request_lifecycle` 中持有；`SettingsPageSaveState` 是供 facade 渲染的只读快照。

## Settings 加载、同步与主题文件的边界

- 初次加载继续由页面 effect 派发 `SettingsCommand::Load`；runtime 从 settings capability 取得结果。只接纳当前读取身份一次；实际保存或确认后的下载准入会永久使旧读取失效，包括旧错误，写入失败也不会恢复旧读取。写入期间不再发起初次读取。
- 初次加载只回填进入本页以来未编辑的草稿/预设；下载则以第二次确认、命令发出时为界，允许替换此前的输入，保留此后的编辑。两种编辑代次分别比较，同值事件、改回原值、无法解析的数字输入也属于编辑。整份草稿保留，不逐字段合并；数字输入仍沿用最后有效设置值与原有错误提示，不写入虚构值。
- 接纳的加载/下载结果仍更新全局已保存主题；草稿保留且与结果不同时，提示尚未保存。预设仅在没有后续选择时，根据有效草稿的 CSS 识别。当前结果缺少设置值时只投影其状态，不回填草稿。加载/下载成功按“预设 → 草稿 → 已保存主题 → 提示”发布。
- `sync/state.rs` 是端点、路径和下载确认的唯一状态来源。`prepare_pull` 首次返回 `None` 要求显示确认，再次调用返回 `PullConfig` 并消耗确认；每次实际下载都重新确认。修改任一输入（包括同值事件）或准备上传都会取消已有确认。
- `prepare_push` / `prepare_pull` 在同一次状态借用内清除确认并克隆当前两个字符串；session 释放借用后才启动任务。值不裁剪、不预校验、不落盘；后来改输入不改变已构造的命令。
- 保存与实际下载共用一个页面内写入 gate。下载会写入本地配置，不能只按请求先后丢弃其回复；同页正在保存/下载时，另一写入直接提示稍后重试，不构造命令、不排队。被阻止的下载清除旧确认，重试须再次确认；确认本身不占写入 gate。成功、404、错误和空结果都释放 gate。
- 下载准入也使此前的主题文件读取失效；下载期间新发起的主题导入可以成为后续草稿，保存仍受公共 gate 约束。上传保持既有端点快照与提示行为，不占本地写入 gate。页面任务作用域保持原样，离页不承诺回滚已发生的持久化；gate 不提供跨页面、跨标签页或跨进程的事务串行化。
- 主题文件仍分三层：`theme_file_io.rs` / Android export 适配系统文件或浏览器下载，`theme_io.rs` 组织成功/取消/失败反馈，`theme_apply.rs` 校验 CSS 并进入现有保存流程。预设识别已有纯 Rust 测试；不为统一接口另建 controller、crate 或文件状态镜像。Android 导入目前仍提示手动粘贴，未接入系统选择器。
- 主题文件的取消、编码、失败与离页契约见下节；不继续细拆已有的简单纯规则。

## 主题文件交互契约

- Web 与桌面导入只接纳有效 UTF-8，随后复用 domain 的 CSS 校验和原有外观保存入口。读取/编码/CSS 校验失败不覆盖草稿或已应用主题；保存失败保留已导入草稿，需明确重试。空文件可作为空 CSS 导入；只有导出会拒绝全空白内容。
- 有效文件的内容处理保持原状：Web 移除开头可选的 UTF-8 BOM，桌面保留；CRLF 不在文件适配器中改写，textarea 的换行显示由浏览器处理。
- Web 选择器取消或返回空列表不改提示/草稿；桌面选择器取消显示既有取消提示。重复选择同一文件前继续清空隐藏 input 的选择值。
- Web 字节读取使用已有 `gloo-file` 适配器，错误事件会返回失败，页面任务释放时中止未完成读取。桌面先取得 `rfd::FileHandle`，再经 `tokio::fs` 读取，文件消失/不可读返回错误，不再经过会 panic 的 `rfd::FileHandle::read`。
- 导出捕获点击时的草稿，不保存设置。桌面成功/取消/失败分别反馈；Web 成功表示已发起下载，不能据此确认用户最终保存到磁盘。Web 的下载节点准备失败不分配 object URL，插入节点失败也会清理已分配的 URL。
- 原生导入/导出继续归属页面任务，私有 Future 注入点仅用于检验返回/取消契约。离页取消等待，不承诺关闭操作系统对话框，也不回滚已经发生的文件或配置写入；Android 系统保存器仍自行完成回调。
- `SettingsPageSession` 只持有一个主题导入代次；在打开选择器时同步分配身份。新导入即使取消或失败，也永久取代旧读取。`theme_io.rs` 在校验 CSS、发布提示、修改草稿或保存之前统一检查并消耗身份；过期的成功、失败和取消均不产生页面副作用。
- 用户修改设置草稿、选择预设、应用/清空主题或显式保存时，使在途导入失效；同值事件和改回原值也不能恢复旧身份。Web 隐藏 input 只暂存本次选择器身份，不持有另一份设置；桌面任务在开始轮询选择器前也检查身份。
- 加载或保存结果实际替换草稿/预设时同样使导入失效；回填完全相同的值、仅更新提示、以及保存保留后续编辑时，不取消新导入。是否回填由上节的加载/下载与保存规则决定。
- 不新增文件操作 busy 状态，也不取消已接纳的设置保存。若上一份设置正在保存，新文件可成为后续草稿，继续由公共保存 gate 提示尚未提交，并要求显式保存。离页仍由页面任务作用域释放等待。

## Feeds 查询与刷新提示规则的使用约束

- `request_policy.rs` 只根据当前值产生计划，不持有 signal、任务或第二份页面状态。只有 `LoadSnapshot` 分配下一查询代次，保留 `wrapping_add`；编辑加载和所有写入继续使用原有规则，不受快照代次影响。
- session 同步发布代次后才构造 executor Future，再在原页面作用域启动任务。返回时先检查身份，过期快照的整批结果（包括无身份字段的全局状态、reload）直接丢弃；接纳时保序投影并移动结果，不克隆列表。
- `ManualRefresh` 直接交给 Shell 的既有同步去重入口，使用 App 作用域；单来源刷新仍是页面任务。页面销毁取消未完成结果等待与提示计时，不承诺回滚已提交的写入。
- 只有单来源刷新在整批结果发布后捕获当前提示：精确 tone `error` 等待 6 秒，其余等待 3 秒。等待由适配器执行；纯规则只提供时长和捕获的清除 intent。
- 清除必须同时匹配 `status_revision` 与消息正文。删除确认等现有转移可能只改消息而不增 revision，不能删去消息比较；匹配时只清空提示文本，不改 tone、revision 或确认状态。页面 reducer 仍是页面状态的唯一写者。
- 新增/删除后的快照重取、编辑草稿、导入导出及 Shell 刷新 revision 的既有 effect 不变；不新增自动重试、去重门或通用页面控制框架。

## 来源编辑状态的使用约束

- `FeedEditState` 内嵌在既有 `FeedsPageState`，接管原来的五个编辑字段，是唯一编辑状态；不增加 signal、第二份长期草稿或业务状态镜像，不依赖 Dioxus 或 I/O。
- 打开/关闭沿用环绕递增代次。加载期间可以关闭或切换来源；保存期间拒绝打开、关闭、草稿变更及重复提交。页面返回仍先消耗弹窗关闭请求，保存中不导航。
- 加载完成同时核对来源 ID、代次、busy 及尚无 details；重复读取和同来源上次访问的迟到成功/失败都不替换草稿。保存结果保留原有的 ID/代次准入，不额外要求 busy，成功清空编辑状态，失败保留草稿并释放 gate。
- 只把精确空名称映射为 `None`；名称/URL 的 trim、格式和长度校验继续由 application 执行。准备保存只复制请求所需的 URL 与自定义名称，不复制原始名称或整个列表。
- reducer 在同一次状态借用内串接编辑转移与页面行为：获准打开才清理旧删除/导入确认，成功保存才增加 reload 并发布原有提示；status revision、tone 与失败提示规则保持原样。任务构造与页面作用域仍由 session 负责。
- 弹窗的键盘 Esc 与原生 `cancel` 都交给同一个 Rust 关闭入口。禁用控件导致焦点移到 body 时，也不能绕过保存 gate 只关闭原生窗口；输入法组词期间的 Esc 仍由原键盘分支拦截。此修复与状态提取的验收边界见 [来源编辑交接](../handoffs/2026-10-28-feed-editor-state.md)。

## 目录控制器的使用约束

- `reset` 只从页面/查询/分组的现有身份变化触发，提升 epoch 和渲染 revision；保留可见性，清除旧交互上下文。
- `receive` 一次检查 epoch/sequence，再完成状态转移和回复构造。过期事实不改变状态、不导航、不恢复滚动，但仍回复当前 epoch 和原请求 sequence，释放适配器的单条在途消息。
- 返回的 `set_page` 交给现有分页 reducer；同页是否需要更新不由控制器另行判断。
- `publish` 只表达渲染 revision 是否改变。序号变化或相同路径观测不要求发布 Dioxus signal；跟随命令仍可在无重新渲染时对齐。
- 单个 epoch 内桥接的本地控制器是唯一写者。另一写者只有重置 effect，且每次必定提升 epoch。适配器在处理下一事实前按 epoch 同步，保留“派发分页 → 按需发布 → 回复”的顺序。
- 展示层仅通过只读访问器读取状态；目录操作不进入文章查询/分组输入，不清空同页勾选。
- 现有事实及成功回复的 serde 字段保持不变。`directory/transport.rs` 区分单条消息解码失败与通道终止：前者返回拒绝回复并继续接收，后者结束监听并通知 DOM adapter 清理。拒绝消息只释放该私有单飞通道的一次等待，不重置 Rust 交互状态，不循环重试错误载荷。
- 时间目录和来源目录都使用同一文章叶节点组件及全局 `(index, id)` 引用校验。时间路径为月份 → 日期 → 文章，来源路径为来源 → 月份 → 文章；只有展开的路径挂载叶节点。测量器在两种分组下都观测具体文章。
- 超过 80 篇的展开文章分支使用独立窗口模型，保留完整全局文章引用；小分支继续直接渲染。窗口模型不进入查询/分组依赖，不复制业务数据，也不改变选择范围。
- 行高是宿主测量值；初始估计 56px、上下预留 240px 仅用于未测区域，不能作为固定行高契约。宽度变化使旧测量失效，变高行修正视口之前的偏移时补偿滚动锚点；纯滚动没有行高变化时不重建偏移索引。
- 几何缓存按目录 epoch 和分支身份保存。折叠/隐藏后复用；上下文重置后清除。焦点行可独立保留，不能为了保留一个远端焦点而挂载中间所有文章。
- 控件身份使用文章 ID，且必须位于动态行迭代器的键控根部；不能在外面增加无 key 的片段，以免范围变化时重建已有按钮并丢失焦点。
- 查询缩短投影与目录 epoch 重置之间可能有一帧过渡；已失效的窗口行应直接跳过，不能用无 key 的空节点混入键控迭代器。归档、筛选后仍须使用新投影的全局引用与分页身份。
- 可见范围桥接独立于展开/跟随桥接，单条在途；回复等待本窗口 revision 提交后执行。显式定位/聚焦后丢弃定位前排队的几何事实，下一帧重新测量，避免旧视口卸载新焦点。两条通道共用消息损坏恢复逻辑。
- 未挂载的跟随目标通过 Rust 全局引用找到逻辑行，先挂载后对齐；Tab/Shift+Tab 和方向键可跨越窗口边界，Home/End 可到分支首末篇，真实边界保留 Tab 离开分支的默认行为。
- 自动跟随的窗口回复只负责挂载目标，实际对齐统一由目录控制器适配器在确认没有更新的手动意图后执行；窗口适配器仅为显式键盘聚焦执行定位，不能让迟到的自动回复覆盖用户滚动。目录内 Tab/Shift+Tab 属于手动浏览，目录外 Tab 不作为主列表滚动输入。
- 每个窗口适配器同时绑定挂载节点、目录 epoch 和实例所有权；在异步回复后仍须重新检查。筛选、分页、分组或路由替换窗口后，旧回复不能测量、滚动或聚焦新目录，旧等待必须释放。
- 目录定位的 DOM 执行请求从点击时绑定序号、查询上下文、原页和目标页。Rust 接受且目标页提交后才能滚动；其他页面或查询取代该请求后，旧回复不能重建它，即使用户随后又返回原目标页。已就绪页面若已无该目标，也应结束等待。这是宿主等待提交的生命周期，不改变 Rust 的目标页判定、全局引用或分页 reducer。

## 阅读快捷键的使用约束

- 纯策略只输出上一篇、下一篇、切换已读、切换收藏；路由和业务副作用留给当前适配层。
- M/F 的大小写、左右方向键、修饰键及 composing 排除规则保持不变。Ctrl+F 继续交给浏览器查找，组合方向键不切换文章。
- 在排除修饰键/输入法之后才解析平台键值，不再调用显示字符串和小写转换；仍有 Dioxus 本身的事件数据成本，不宣称零分配或用户可感知的性能收益。
- 不新增按键重复抑制或 `prevent_default`。正文阅读区与搜索输入的作用域保持现状。
- `reading_position.js` 对切文按键的提前采集仍须与 Rust 快捷键边界一致；浏览器专项同时验证两侧。

## 暂时保留在宿主适配器中的工作

DOM 几何测量、原生对话框焦点/模态、同步阻止链接或空格默认动作、真实滚动、滚动来源归因、帧与提交确认，继续由现有适配器负责。不能为了减少 JS 行数而把同步事件处理改成异步往返，或在每帧传递整棵 DOM 的测量数组。

目录输入归因的时间窗口、阅读位置的 DOM 锚点识别等仍属于当前浏览器实现。未来原生适配应按相同交互契约提供对应事实和执行能力；当前抽取不等于已经具备原生渲染器。

下一阶段优先保持这些控制器的单一规则来源，为选定的原生控件补输入、测量和滚动适配。正文、图片、选择复制、焦点、输入法和系统返回需要各自的实际宿主验收；本轮不选框架、不新增 crate，不迁入 v0.2 主线。

## 验证入口

- 纯状态与策略：`cargo test -p rssr-app --locked`。
- 初始化/查询/写入适配：`scripts/browser/rssr_entries_lifecycle_acceptance.cjs`，通过离线 fixture 覆盖选择、筛选、单条并发、批量失败重试、写入期间切换筛选、偏好失败重试/重开、查询稳定性，以及桌面/小视口搜索、刷新和阅读返回。基线和候选分别指定 `ARTIFACT_DIR`，构建目录可由 `PUBLIC_DIR` 指定。偏好重开前需确认最终值已提交，不能仅等待锁暂时空闲。
- Windows 写入与重开：`scripts/browser/rssr_entries_writes_native.cjs --mode write|reopen --cdp <endpoint> --database <isolated-db> --artifact-dir <path>`。仅用于独立进程、独立 WebView profile 与 5000 篇阅读 fixture；只读 SQLite 核对真实提交，运行者负责安全启动/关闭自己的进程，write 后结束进程再启动执行 reopen。
- Reader 结果归属：`scripts/browser/rssr_reader_lifecycle_acceptance.cjs` 使用既有离线 fixture 与 `PUBLIC_DIR` / `ARTIFACT_DIR`，覆盖桌面/窄屏正文、按钮与 M/F、文章身份、往返、迟到成功/失败和不存在文章后的恢复；每次标记操作比对全体文章标记。传 `--cdp <endpoint> --database <isolated-db> --artifact-dir <path>` 可复测独立 Windows WebView2 的正常阅读流程，只读 SQLite 核对 5000 篇 fixture；迟到回复注入仅在 Web 模式运行。原生进程和测试库由调用方管理。
- Reader 本批的源码、产物身份及实际平台边界见 [结果归属提取交接](../handoffs/2026-10-28-reader-result-lifecycle.md)；隐藏原生窗口的截图不作为视觉通过证据。
- Settings 保存：`scripts/browser/rssr_settings_save_acceptance.cjs --artifact-dir <path>` 使用 `PUBLIC_DIR` 指定构建包和既有离线 fixture，在 1536/390 视口验证校验失败、零值回退、CSS/主题保存与重载；另以真实 Web Locks 延迟写入、以 Storage 写失败验证后续草稿和显式重试。`--mode write|reopen --cdp <endpoint> --database <isolated-db> --artifact-dir <path> --expect <write/acceptance.json>` 用于独立 Windows 进程的正常保存与重开，只读 SQLite 对照完整设置和 5000 篇文章标记。原生进程由调用方管理，重开前须真正结束旧进程。旧 fixture 的归档阈值 0 在隔离数据中准备为合法的 120；不改用户数据库。详见 [Settings 保存提取交接](../handoffs/2026-10-28-settings-save-policy.md)。
- Windows Settings 验收从新启动的 Entries 页开始；任何导航、编辑或 fixture 设置准备前，先只读核对数据库的 5000 个连续 ID/固定标题，再对照当前页面已挂载文章的 ID/标题。错误目标直接记为 `status: failed`，不得先修正归档阈值后才检查目标。这个检查不能替代调用方的独立进程、复制数据库和独立 profile 要求；负向复现见 [Settings review 交接](../handoffs/2026-10-28-settings-save-review.md)。
- Settings 配置交换：`scripts/browser/rssr_settings_sync_acceptance.cjs --artifact-dir <path>` 使用 `PUBLIC_DIR` 与离线 fixture，覆盖 1536/390 视口的确认/修改目标/上传清除确认、参数快照、成功导入、404、无效配置和重新确认后重试。自管 `127.0.0.1:9277` WebDAV fixture 保留原订阅集合，仅修改配置中的主题/合法归档阈值；比对全体文章标记及订阅 ID/URL。Web 由 Playwright 转发到回环服务，不覆盖真实站点的 CORS/网络权限；Windows 传 `--cdp <endpoint> --database <isolated-db>`，先校验 5000 篇数据库和当前页面身份及回环来源 URL，再开始操作。调用方负责独立进程/profile/复制库，见 [Settings 同步边界交接](../handoffs/2026-10-28-settings-sync-boundary.md)。启动失败回归可复用 `rssr_feeds_lifecycle_startup_test.cjs <artifact-dir> settings-sync`。
- Settings 回填交错：同一运行器追加 `--scenario lifecycle`，仅用于隔离 Web fixture。以 Web Lock 的结果延迟和回环下载延迟，覆盖初次加载/下载期间的编辑、改回原值、预设、无效数字、与保存互斥、重复下载、前后主题文件导入及失败后重试。基线额外传 `--baseline true` 断言旧缺陷，候选不得带该参数；源码/产物身份和平台边界见 [Settings 回填交接](../handoffs/2026-10-29-settings-load-ownership.md)。
- 主题文件：`scripts/browser/rssr_theme_files_acceptance.cjs --artifact-dir <path>` 使用 `PUBLIC_DIR` 与隔离 fixture，覆盖 1536/390 视口的有效/空/无效 CSS、UTF-8/BOM/CRLF、同名重选、取消、真实下载字节、错误事件、导出失败清理、离页、持久化失败重试及全部文章标记。`--baseline true` 仅用于记录旧版本的已知缺陷，不能当作候选通过；默认模式严格要求修复后的行为。Rust 的文件适配测试使用真实临时文件，页面取消/返回使用 Dioxus 可控 Future，不替代系统选择器实机验收。见 [主题文件交接](../handoffs/2026-10-28-theme-file-contracts.md)。
- 主题导入归属：同一运行器追加 `--scenario ownership`，覆盖新旧文件乱序、旧读取失败/无效 CSS、草稿改回原值、外观/预设编辑、清空、同值保存、新选择器取消和新文件校验失败；真实 Web Lock 延迟保存，确认已开始的保存仍完成且保留后来导入的草稿。旧基线与候选使用相同脚本，候选不得传 `--baseline true`；证据和未覆盖的原生 GUI 边界见 [主题导入归属交接](../handoffs/2026-10-28-theme-import-ownership.md)。
- 原生文本传输：`scripts/browser/rssr_native_transport_acceptance.cjs --process-file <独立进程回执> --artifact-dir <证据目录> --baseline false` 直接输入并应用 CSS，验证真实 Rust 字符串表的 BOM/Unicode 与后续控件字段对齐、持久化和重开；不操作系统选择器。Windows 系统导入曾发现 BOM 错位，修复及直接回归见 [本批文件与传输交接](../handoffs/2026-10-29-native-theme-file-acceptance.md)。系统导出、真实 IME 等缺口仍单列，不推定通过。
- Feeds 查询与提示：`scripts/browser/rssr_feeds_lifecycle_acceptance.cjs --artifact-dir <path>` 使用 `PUBLIC_DIR` 构建包和离线 fixture，在 1536/390 视口验证成功/错误提示消退、旧计时器保留删除确认、全局刷新 busy、添加/删除后的统计重取以及全体文章标记保持。Windows 模式传 `--cdp <endpoint> --database <isolated-db> --feed-origin http://127.0.0.1:<port>`，调用方须提供独立进程、profile 和 5000 篇复制 fixture，并预先将复制库中的来源 URL 指向该空闲端口；脚本先只读核对数据库、当前 Entries 页及 URL，再启动自己管理的回环 RSS 服务。乱序快照和离页取消由 Dioxus 可控 Future 测试覆盖；源码/产物身份与平台边界见 [Feeds 查询与提示交接](../handoffs/2026-10-28-feeds-request-lifecycle.md)。
- Feeds 验收启动失败也必须覆盖旧报告并释放连接；清理错误计入失败，最终报告在清理后写出。`node scripts/browser/rssr_feeds_lifecycle_startup_test.cjs <证据目录>` 使用自建无头 Chrome、临时 SQLite 和回环 HTTP 端点验证缺少目标页、数据库打不开、CDP 拒绝连接三类真实失败，检查非零退出、旧成功报告替换，以及调用方浏览器和数据库保持不变。复现与修复见 [Feeds review 交接](../handoffs/2026-10-28-feeds-lifecycle-review.md)。
- 来源编辑复用上述 Feeds 运行器，追加 `--scenario edit`，由 `feed_edit_scenarios.cjs` 验证取消、重复地址失败保留草稿、修正后重试、重开回填、恢复原名、确认状态清除；两视口比对所有文章标记及来源 ID/URL/名称。Web 以真实 Web Lock 延迟保存，检查禁用、重复 submit、焦点移出后的 Esc；Windows 仍要求隔离副本，仅覆盖正常保存流程，忙碌与乱序边界另由纯 Rust / Dioxus 测试覆盖。
- 目录实际行为：`scripts/browser/rssr_small_viewport_assertions.mjs --directory-only true`，包含日期/来源/文章层级、跨页、手动/跟随、显隐和选择保持。
- 不启动预览服务的回归：先构建 Web release 包，再运行 `scripts/browser/rssr_directory_offline_regression.cjs` 和 `rssr_directory_window_acceptance.cjs`。Playwright 在隔离无头 Chrome 内提供构建文件与测试数据；后一脚本覆盖 600 篇单分支、变高行、焦点、跨页、显隐恢复及损坏测量通道。`rssr_directory_transport_acceptance.cjs --offline-fixture true --artifact-dir <path>` 同样可离线回归旧目录通道。
- `scripts/browser/rssr_directory_window_boundaries.cjs` 延迟真实 Rust/WASM 回复，覆盖筛选/翻页/分组/返回后的旧焦点、被取代的跨页定位，以及来源筛选后末页正文返回的位置恢复。`rssr_directory_window_review.cjs` 补充手动浏览与自动跟随交错、隐藏时变宽和批量操作。
- 阅读位置及快捷键：`scripts/browser/rssr_reading_position_acceptance.cjs`，包含返回、切文、M/F、修饰键/IME、正文测量次数。
- 具体源码身份、命令和平台范围见 [2026-10-26 交接](../handoffs/2026-10-26-native-ui-preparation.md)。编译、浏览器运行、原生设备运行和性能结论分别记录。

## 原生验收后的优化顺序

`34a63202` 的 Windows / Android 实际目录、连续返回和本会话阅读位置恢复已验证；完整证据与未验范围见 [原生目录验收](../handoffs/2026-10-26-native-directory-validation.md)。真实输入法留待用户空闲，后台 WebView 的降频样本不能用作前台性能基线。

Android 展开一个包含 597 篇文章的来源月份时，渲染 trace 显示样式计算和布局是明确成本。现已按上述边界实现文章分支窗口化，功能、原生验收和测量范围见 [目录窗口化交接](../handoffs/2026-10-26-directory-windowing.md)。不切掉文章层级，不用固定行高假设，不改变查询或业务层职责；挂载节点、布局耗时、宿主往返延迟和内存应分别报告，不能仅凭节点数减少推断整应用更快。

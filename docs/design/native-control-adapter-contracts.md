# 原生控件适配契约

核对日期：2026-10-29；源码基线：`ea70ff19f7713220e8c1dec93aa53fa6fd679bc9`。本文把已有行为转换为控件接入时的检查项，不新增公开 API，也不代表原生渲染器已实现。规则细节仍以[原生界面前置模块边界](./native-ui-preparation.md)和[前端命令清单](./frontend-command-reference.md)为准；执行步骤、证据等级和首批范围见[原生控件验收清单](../testing/native-control-acceptance.md)。

## 当前可复用到哪里

普通 Rust 策略可以独立测试，但 facade/session 仍依赖 Dioxus Signal、页面作用域和 effect。Windows / Android 当前仍使用 WebView；`components/dialog.rs` 的“native dialog”指浏览器的 HTML `<dialog>`，不是 Windows 系统窗口。系统文件选择器才是已有的 OS 控件适配路径。

| 层次 | 当前权威来源 | 控件适配需要保留的职责 |
| --- | --- | --- |
| 状态和决策 | 页面 reducer、既有 request/write lifecycle、save policy、目录和位置控制器 | 只调用已有规则，不镜像一套 busy、设置、文章列表或请求队列 |
| 页面装配 | facade/session、`UiCommand`、runtime | 同步准入和捕获输入，再启动任务；按各操作已有的准入规则在页面作用域发布，是否检查请求身份见下表 |
| 业务和存储 | application 用例、host capability、infra 适配 | 保持配置交换、查询、写入的同一语义；不把平台分支搬到 domain/application |
| 控件和宿主 | Dioxus 事件、DOM bridge、文件 I/O、Android JNI | 提供输入、焦点、测量、时钟、实际滚动和系统回调；负责控件资源释放 |

不要求为这张表建立新 trait、crate 或通用事件总线。将来替换 Dioxus 装配需要单独确定宿主和渲染方式，不能把当前私有 Rust 类型当作已稳定的跨渲染器 SDK。自定义 CSS、正文 HTML 和语义选择器仍是现行产品契约；更换渲染方式不能默认取消这些能力。

## 输入必须保留的事实

以下是适配要求，不是拟新增的事件类型。宿主可以维护输入法组词、未完成数字文本等控件缓冲，但设置和保存资格仍以现有 Rust 草稿为准。

| 输入面 | 交给 Rust 的事实与时点 | 控件责任和容易丢失的边界 | 源码入口 |
| --- | --- | --- | --- |
| Settings 阅读偏好 | 三个枚举字段的选择；四个数值字段每次输入的解析结果 | 同值事件、改回原值和无法解析的输入都算用户编辑。解析失败只调用 `note_draft_edit` 并显示原提示，保留上次可解析值；不能擅自填 0、裁剪或增加自动保存。范围校验仍走原应用服务 | [preferences.rs](../../crates/rssr-app/src/pages/settings_page/preferences.rs) |
| Settings CSS / 预设 | CSS 文本编辑、预设选择、应用/清空的明确动作 | 草稿与预设分别记编辑代次；仅选择预设不等于应用。程序回填控件时不能回送成用户编辑，否则会虚增代次、使后续加载和导入失效 | [session.rs](../../crates/rssr-app/src/pages/settings_page/session.rs)、[lab.rs](../../crates/rssr-app/src/pages/settings_page/themes/lab.rs) |
| Settings 保存 / 下载 / 上传 | 保存捕获草稿和预设；实际下载及上传捕获当时的 endpoint / remote_path | 保存和实际下载同步共用页面写入 gate；按钮禁用不能代替准入。首次下载只显示确认，目标字段编辑（含同值）和上传取消确认；被 gate 拒绝的下载重试也须重新确认。上传不占该 gate | [save/session.rs](../../crates/rssr-app/src/pages/settings_page/save/session.rs)、[sync/session.rs](../../crates/rssr-app/src/pages/settings_page/sync/session.rs)、[sync/state.rs](../../crates/rssr-app/src/pages/settings_page/sync/state.rs) |
| 主题文件 | 打开导入选择器时取得请求身份；导出点击时取得当前 CSS 文本快照 | 文件尚未读完也已拥有身份；之后打开的新选择器即使取消，也不能让旧读取重新生效。导出不保存设置，不重新读取后来编辑的 CSS | [theme_io.rs](../../crates/rssr-app/src/pages/settings_page/themes/theme_io.rs)、[theme_file_io.rs](../../crates/rssr-app/src/pages/settings_page/themes/theme_file_io.rs) |
| 来源编辑 | 来源 ID、打开代次、名称/URL 原始输入、明确保存或关闭 | 加载中可关闭/切换；保存中拒绝修改、关闭、重开及重复提交。精确空名称转为 `None`，trim/校验留在 application；失败保留草稿供修正重试 | [edit_state.rs](../../crates/rssr-app/src/pages/feeds_page/edit_state.rs) |
| Entries 查询 / 写入 | 当前查询与偏好就绪状态；单条 ID；批量操作时明确选中的 ID 快照 | 同步准入后再启动任务；单条和批量互斥关系复用 reducer，不能仅按可见行或窗口内下标构造批量命令 | [request_lifecycle.rs](../../crates/rssr-app/src/pages/entries_page/request_lifecycle.rs)、[write_lifecycle.rs](../../crates/rssr-app/src/pages/entries_page/write_lifecycle.rs) |
| 阅读快捷键 | 按键、是否带修饰键、是否正在组词，以及现有正文作用域 | 修饰键或 composing 先放行，再解码 M/F/方向键；不新增 repeat 抑制或拦截 Ctrl+F，不将快捷键扩展到输入框 | [reader_shortcut_policy.rs](../../crates/rssr-app/src/hooks/reader_shortcut_policy.rs)、[use_reader_shortcuts.rs](../../crates/rssr-app/src/hooks/use_reader_shortcuts.rs) |
| 下拉刷新 | 触点数量、坐标、顶部状态、表单/选区事实及嵌套滚动双方向余量 | 松手重验最终坐标和剩余触点；丢失坐标、取消、多指等不能留下 armed。现有 80 CSS px 是当前坐标单位；新宿主须明确逻辑像素换算，不直接使用物理屏幕像素 | [gesture.rs](../../crates/rssr-app/src/pages/entries_page/pull_refresh/gesture.rs) |
| 目录 / 位置恢复 | 全局文章引用、查询上下文、页码、访问身份、单调时钟、挂载行几何和焦点 | 当前浏览器适配传 CSS px 和毫秒；未来宿主需给出一致单位、真实布局提交及用户滚动来源。控制器不猜测 OS 帧时序 | [目录维护入口](../../crates/rssr-app/src/pages/entries_page/directory/README.md)、[controller.rs](../../crates/rssr-app/src/ui/reading_position/controller.rs) |

## 回填不能统一成一种规则

对于已有请求身份检查的操作，“将结果写回控件”必须晚于 Rust 的结果准入；失败提示和无文章 ID 的状态消息同样不能绕过这项检查。不是每个操作都有结果代次，上传和 CSS 导出的现行区别单独列出，不能在适配时顺便增加“只接受最新请求”的规则。

| 结果 | 接纳和回填的现行规则 | 适配验收重点 |
| --- | --- | --- |
| Settings 初次加载 | 只接纳当前读取一次；按本页挂载以来的草稿/预设编辑代次分别判断。获准保存或下载永久作废旧读取，写入失败也不恢复它 | 加载 effect 执行前发生的编辑也保留；旧成功和旧错误均不能回填 |
| Settings 下载 | 第二次确认实际发命令时捕获编辑代次；只替换此后未编辑的整份草稿，预设独立判断 | 确认前输入可以被下载替换；发出后的同值事件、A→B→A 和无效数字也保留。实际已保存主题仍更新 |
| Settings 保存 | 当前整份草稿仍等于提交值时才回填；预设仍等于提交选择时才回填 | **保存采用值相等，不采用编辑代次**。A→B→A 仍可回填；不逐字段合并。结果与当前草稿相同不提示未保存；后续不同草稿须明确再保存 |
| 主题导入 | 在验证 CSS、显示提示、改变草稿和触发保存之前消费当前导入身份 | 新文件、草稿/预设编辑、下载准入会作废旧导入；加载/保存仅在实际改变草稿或预设时作废。读取/编码/CSS 错误不改草稿；有效导入进入既有保存流程，保存被阻止或失败则保留草稿 |
| Settings 上传 / CSS 导出 | 上传只投影状态提示；桌面/Android 导出在页面任务完成时提示成功、取消或失败，Web 导出同步提示已发起下载。它们没有页面结果代次过滤，不占保存/下载 gate | 同页后续编辑或操作不会自动使先前完成提示失效，较早请求晚完成仍可能覆盖当前提示；导出仍使用点击快照且不保存设置。页面销毁取消等待的规则不变；Android 导出另保留系统保存器的单在途限制 |
| Entries / Reader / Feeds | 复用各自请求归属，拒绝旧结果整批发布，接纳结果保持原顺序 | Entries 先比较查询身份再比较 flags revision；Reader 同时比较文章 ID 和访问代次；Feeds 编辑重开同一来源仍拒绝上次加载。不能退化为“只比较 ID” |

Settings 的详细发布顺序见[既有模块边界](./native-ui-preparation.md)。适配器只反映投影结果，不根据提示文案或 tone 推断保存是否成功，不把界面回填当作新的用户输入。已保存全局主题与正在编辑的草稿是两个已有语义，不能为了让控件看起来同步而覆盖后续草稿。

数字解析提示属于产生它时的草稿回填代次。加载、下载或保存结果获准替换整份草稿时，即使值完全相同，也永久作废旧提示；A→B→A 不能让提示复活。因后续编辑而拒绝回填时保留提示，新无效输入只标记自己的字段。该代次只描述接纳的回填，不计为用户编辑，也不改变保存的值相等规则、加载/下载的编辑代次规则或主题导入的失效条件。对应回归见 [Settings 表单交接](../handoffs/2026-10-29-settings-form-feedback.md)。

## 取消分为不同的作用范围

| 触发 | 会停止或拒绝什么 | 不承诺什么 |
| --- | --- | --- |
| 页面卸载 | 原页面作用域的未完成 UI 等待、结果发布和提示计时；新挂载使用新的页面状态 | 不回滚已提交的数据库/文件写入；不保证撤销已经发出的网络操作或关闭系统选择器 |
| 带身份的请求被后续操作取代 | 按对应模块身份拒绝旧成功、旧错误和旧提示；过期结果不进入当前界面 | 结果失效不是存储事务撤销；不外推为上传/导出已有结果代次过滤 |
| 下载确认被清除 | 首次点击形成的待确认意图失效，下次需重新确认 | 不能用它取消已获准并开始的下载 |
| 弹窗 Esc / 背景点击 / `cancel` | 先进入同一个 Rust 关闭入口；来源保存中拒绝关闭。Esc 组词分支同步阻止浏览器默认关闭 | 不能让原生窗口先关掉再异步询问 Rust；禁用控件后焦点移出弹窗也不能绕过关闭 gate |
| 文件选择取消 | Web 取消/空列表保持提示和草稿；桌面取消显示原有信息；新导入已取代旧请求 | 不统一为报错或重启旧请求。Web 未选择文件时不要依赖一定触发 `change` |
| 用户滚动 / 切换上下文 | 结束对应位置恢复；目录切换后旧测量、回复和定位不得操作新实例 | 不取消业务写入，也不把“收到回复”当作目标页已经布局完成 |
| 手动全局刷新 | 继续由 Shell 的 App 作用域和共享 refresh flight 管理 | 页面退出不取消全局刷新；不能套用页面任务取消规则 |

取消与焦点的实际接线见 [Dialog](../../crates/rssr-app/src/components/dialog.rs) 和 [系统返回](../../crates/rssr-app/src/hooks/use_mobile_back_navigation.rs)。Android 返回先给当前覆盖层/页面关闭逻辑处理机会，随后走已有 history/fallback；返回前位置捕获和重复返回 gate 保持原样。JNI 只传事件，不跨线程直接修改 Dioxus 状态。

## 宿主必须自行完成的能力

| 能力 | 现有事实 | 接入新控件时须证明 |
| --- | --- | --- |
| 焦点和模态 | HTML `<dialog>` 负责焦点约束；移除时仅向仍连接的原节点恢复焦点 | 打开、Tab/Shift+Tab、Esc、原生关闭事件、IME 与禁用控件交错；回焦不额外滚动。OS 文件窗口和 HTML 弹窗分开验收 |
| 布局提交和滚动 | 目录 bridge 按 epoch/revision 等待挂载，异步后复查实例所有权；位置恢复有 visit/sequence | 先挂载/布局后滚动或聚焦；替换窗口后旧回复不能复活，手动滚动不能被迟到自动跟随覆盖；宽度变化重测行高 |
| 文件读写 | Web 字节读取/下载；桌面 rfd + 文件 I/O；Android 系统保存器 | 成功、取消、真实读写失败、重复选择和页面离开分开记录；不能用临时文件单测代替系统选择器 GUI |
| 原生文本传输 | [原生文档启动适配](../../crates/rssr-app/src/bootstrap/native_document.rs)为 Dioxus 解释器保留字符串表开头的 BOM；不修改文件或业务草稿，不替换全局 `TextDecoder` | Dioxus / sledgehammer 升级时复核私有 `c` 解码器及 module → DOMContentLoaded → load 启动顺序；用真实 Rust 字符串表验证 BOM、Unicode 和后续字段对齐，见[传输运行器](../../scripts/browser/rssr_native_transport_acceptance.cjs) |
| Android 文件回调 | [Rust 导出桥](../../crates/rssr-app/src/pages/settings_page/themes/theme_android_export.rs) 与 [MainActivity](../../android/MainActivity.kt) 协作，返回成功/取消/失败；Activity 销毁报告未完成 | 实测系统文档提供器、系统返回、前后台和销毁后的回调；页面离开不等于 OS 写入停止。Android 文件导入仍未接入，手动粘贴是当前路径 |
| 正文、图片、选择复制 | 沿用现有已消毒 HTML、图片入口和系统选择/剪贴板行为 | 必须在实际宿主验收中文输入、选择复制、图片缩放/关闭和系统返回；Rust 单测与 Web 小视口不能替代设备验证 |

## 首个适配切口

先补 **Windows 主题 CSS 文件选择/保存的实际宿主验收**。理由是当前已有 rfd 适配和真实文件单测，Web 也有明确回归场景，而 OS GUI 完整路径仍缺证据；可以验证平台边界而无需决定新渲染器。具体成功标准见[验收清单](../testing/native-control-acceptance.md)的 W-FILE 场景。

2026-10-29 已完成部分系统导入实测，并修复由此发现的原生 BOM 传输错位；修复后以命令行/CDP 验收。系统导出、窗口等待期间离页和真实输入法仍有缺口，不能声明此切口全部完成，详见[本批交接](../handoffs/2026-10-29-native-theme-file-acceptance.md)。

Settings 阅读偏好的三个枚举和四个数字字段已补现有 Web / Windows WebView 的同路径验收，并修复数字提示的回填归属，详见 [Settings 表单交接](../handoffs/2026-10-29-settings-form-feedback.md)。它们仍是后续新控件的最小候选；新宿主、渲染及 CSS 兼容方案须另行明确。当前继续保持七个 crate 与 facade/session → `UiCommand` → runtime → application/host 的职责，不为这项建议提前添加控制器或持久状态。

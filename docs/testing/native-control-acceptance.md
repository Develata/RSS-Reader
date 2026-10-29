# 原生控件适配验收清单

核对日期：2026-10-29；契约基线：`ea70ff19f7713220e8c1dec93aa53fa6fd679bc9`。配套[适配契约](../design/native-control-adapter-contracts.md)定义输入、回填和取消边界；本文定义如何证明它们。Windows 系统文件实测发现的 BOM 传输缺陷及修复候选身份，见[本批交接](../handoffs/2026-10-29-native-theme-file-acceptance.md)；不能把旧候选的系统窗口结果视为修复后全部 GUI 通过。

## 证据等级与平台边界

每条结果都写出“源码/产物身份、执行载体、实际动作、回执和缺口”。普通 Rust 单测、Dioxus 可控 Future、真实浏览器、WebView2/CDP、系统 GUI/设备手工和编译检查分别记录；不得互相升级。隐藏 WebView 截图不证明前台视觉，注入 composition 事件不证明真实输入法，`--baseline true` 只证明旧缺陷被记录。

下表是已有证据索引，不是所有平台在当前 HEAD 上新跑过。Settings 回填批次的回执与产物在契约审计时只读复核身份；本批新增 Windows 文件部分实测和原生传输回归另列。其他历史交接仅用于定位场景，未重验其旧产物，也未声明等同当前版本。

| 能力 | 普通 Rust / Dioxus | 静态 Web | Windows | Android / 其他宿主 | 证据入口 |
| --- | --- | --- | --- | --- | --- |
| Settings 初次加载 / 下载交错 | 10/29 已有代次、写入 gate、任务边界测试 | 10/29 两视口各 18 场景；真实 Web Lock / 回环响应延迟 | 10/29 仅正常同步，独立 WebView2 + HTTP + SQLite；未注入同样乱序 | Android 仅编译；Linux/macOS、托管 `rssr-web` 未运行 | [回填交接](../handoffs/2026-10-29-settings-load-ownership.md) |
| Settings 保存、主题文件、导入归属 | 本批 Settings 58 项通过，含 5 项真实文件 I/O 与 6 项 Dioxus 文件任务测试；均不包含 OS GUI | 回填批次已有保存/文件/导入归属结果，本批未新跑 Web UI | 旧候选 6 项 OS 导入场景通过并发现 BOM 错位；修复候选 CDP 13 项与进程重启通过，GUI 仍部分验收 | 本批 Android 编译通过；系统文件设备路径、Linux/macOS 未验 | [本批文件与传输交接](../handoffs/2026-10-29-native-theme-file-acceptance.md)、[文件契约交接](../handoffs/2026-10-28-theme-file-contracts.md)、[导入归属交接](../handoffs/2026-10-28-theme-import-ownership.md) |
| Entries 查询 / 写入 / Reader 结果 | 已有纯规则及适配测试 | 10/27–28 历史同路径与迟到结果场景 | 历史正常流程、写入/重开；不覆盖全部宿主乱序 | 历史编译边界按各交接；不能由桌面小视口推定移动通过 | [Entries 查询](../handoffs/2026-10-27-entries-request-lifecycle.md)、[Entries 写入](../handoffs/2026-10-28-entries-write-lifecycle.md)、[Reader](../handoffs/2026-10-28-reader-result-lifecycle.md) |
| Feeds 编辑 / 弹窗 | 已有身份、关闭 gate、失败重试测试 | 10/28 历史保存延迟、禁用后失焦 Esc、CDP composition | 历史正常编辑，未覆盖真实输入法或精确延迟 | Android 系统返回及 Linux/macOS UI 未验 | [编辑交接](../handoffs/2026-10-28-feed-editor-state.md) |
| 目录 / 阅读位置 / 手势 | 已有控制器、几何、快捷键策略测试 | 历史跨页、窗口替换、焦点和手动/自动交错 | 有历史目录/返回证据，不能外推到新控件 | 有历史 Android 目录证据，版本和设备范围按原记录；真实 IME 未覆盖 | [目录设备记录](../handoffs/2026-10-26-native-directory-validation.md)、[窗口化](../handoffs/2026-10-26-directory-windowing.md)、[边界回归](../handoffs/2026-10-27-directory-window-boundaries.md) |

10/29 Settings 已有结果：工作区 571 通过 / 0 失败 / 2 个既有性能探针忽略；Windows 正常同步每视口核对 5000 篇文章标记和 6 个订阅。它们是上一批的执行结果，**不是本轮新增测试数**。Web 四张比较图中三张逐字节相同，窄屏 loaded 图 R 标识有 108 像素差异，原因未确立；不能记为全像素一致。详见上述回填交接。

契约审计使用 `target/settings-load-20261029/` 的 `validation-identity.json`、`final-receipt.json` 和六份最终验收回执。WASM SHA-256 为 `9bea9bf2e6af9f317ed290c8623d8931df770d396ee18afbaa776b8d0d161293`，旧 Windows EXE 为 `7f1cd5e5b19b06f7827af65ce5d0efa03f813ac5314cb3d3ac338ad3b056980c`；审计回执在 `target/native-control-contracts-20261029/`。本批文件/传输证据位于 `target/native-theme-files-20261029/`。这些是本机 ignored 证据，其他 checkout 缺失时应记“证据不可用”，不能凭文档补写通过。

## Windows 文件实测与 BOM 修复结果

旧 EXE 的实际 OS 导入将 BOM/CRLF 原文正确写入 SQLite，但 Dioxus 字符串表解码丢掉了开头的 BOM，导致文本框、枚举、数字和保存按钮状态一同错位。修复限定在原生文档启动适配：仅为解释器的文本解码器保留 BOM，不改导入原文、Web 去 BOM 规则、保存 gate 或浏览器全局解码器。

新 EXE SHA-256：`43cc29086c9f9097aa6426c7eefc055e82778792a62ae398e52bffafbc1ef8ee`。`rssr_native_transport_acceptance.cjs` 在真实 WebView2 中直接输入并应用 CSS，捕获真实 Rust 字符串表，旧候选稳定复现、新候选 13 项通过。1280×900 / 390×900 普通 CSS 渲染图逐字节相同；这属于 CDP 渲染证据，不是前台 OS 视觉或真实输入法验收。

| W-FILE | 已执行结果与范围 |
| --- | --- |
| 01 | 旧候选：系统选择有效中文 CSS、自动保存、重开页面通过；源文件、全部文章标记/订阅保持。新候选未重跑系统选择器 |
| 02 | 旧候选：取消、同名重选、空文件 3 项通过；取消后可继续导入 |
| 03 | 旧候选：无效 CSS / UTF-8 各通过；BOM 显示缺陷已修复并经新 EXE/CDP 验证。系统选定后真实读失败未覆盖；真实文件单测单独列出 |
| 04 / 05 | 系统保存框、覆盖确认、真实 OS 写失败及重试未执行；Rust 文件 I/O 与 Dioxus 导出快照测试不代替这些结果 |
| 06 | 系统窗口等待期间离页未执行；普通页面重开和进程重启已验，不算该异步场景通过 |
| 07 | 旧候选：中文路径、Esc 取消及返回导入按钮焦点、滚动位置保持已记录；Tab/Shift+Tab 和真实 IME 未覆盖 |

本批后半段按用户要求使用命令行与 CDP，不继续 Computer Use。系统文件流程总体仍为**部分验收**；修复候选的原生传输、58 项 Settings 测试、Clippy、WASM/Android 编译及数据保护结果见交接。

可重复的原生传输入口（调用方先启动独立 EXE/profile/复制库与回环服务，页面停在 Entries；不是 OS 选择器运行器）：

```powershell
node scripts/browser/rssr_native_transport_acceptance.cjs --process-file '<run>/process.json' --artifact-dir '<evidence>/transport' --baseline false
```

`process.json` 包含 `pid`、`exe`、`profile`、`cdp`。脚本核对实际进程、CDP 所属进程链、`target/` 下的复制库与 5000 篇 fixture，才允许设置写入；启动失败覆盖旧回执并返回非零。旧候选另传 `--baseline true`，结果只记 `baseline-reproduced`。调用方负责退出本次进程与回环服务。

## 按改动面选择验收

下列用例是新适配器必须保持的结果。存在现成运行器只表示可复用入口；具体运行和平台范围仍需单独留证。

| ID | 场景和成功标准 | 现有入口 / 仍需的宿主验证 |
| --- | --- | --- |
| FORM-01 | 三枚举、四数字逐一输入；清空/无法解析时保留上次可解析值和提示，仍算编辑；控件程序回填不触发编辑或自动保存 | `preferences.rs`、Settings 单测；新控件逐字段验收，包含真实 IME 和重复同值通知 |
| FORM-02 | 延迟初次加载，先编辑或 A→B→A；草稿/预设不被覆盖。先保存/下载后释放旧加载，旧成功和旧错误均无效 | `rssr_settings_sync_acceptance.cjs --scenario lifecycle`；现有受控场景仅 Web，原生需独立证据 |
| FORM-03 | 保存前捕获快照；后来不同草稿保留，改回提交值仍可回填；失败可重试；零页大小只在提交副本回退 | `rssr_settings_save_acceptance.cjs`；Windows write/reopen 模式验证真实存储，延迟边界另验 |
| FORM-04 | 下载确认前后分别编辑；发出后编辑保留，实际已保存主题更新。保存与下载相互拒绝、重复下载不排队，失败释放 gate | `rssr_settings_sync_acceptance.cjs --scenario lifecycle`；还须确认 endpoint/path 捕获及阻止后重新确认 |
| FILE-01 | 新旧导入乱序、新选择器取消、读取错误、草稿改回原值；旧读取不能显示提示、回填或保存 | `rssr_theme_files_acceptance.cjs --scenario ownership`；桌面系统窗口另按 W-FILE 验证 |
| STATUS-01 | 上传/导出期间同页继续编辑；原请求完成仍可发布提示，导出使用点击时 CSS 且不保存设置；页面卸载后不再发布 | 上传接线见 [sync/session.rs](../../crates/rssr-app/src/pages/settings_page/sync/session.rs)；[theme_io.rs](../../crates/rssr-app/src/pages/settings_page/themes/theme_io.rs) 已有导出快照/完成提示/卸载测试。不要把导入的身份过滤或保存/下载 gate 加到这两条路径；Android 导出单在途限制保持 |
| DIALOG-01 | 来源加载中关闭/切换、保存中重复 submit、Esc、背景与原生 cancel；失败保留草稿、成功关闭 | `rssr_feeds_lifecycle_acceptance.cjs --scenario edit`；当前 HTML Dialog 的失焦场景已有 Web 测试，新 OS 控件须实测关闭通知和焦点 |
| READ-01 | 正文作用域 M/F/方向键；带修饰键、Ctrl+F、输入框、组词期间不误触发；切文结果不污染新访问 | `rssr_reader_lifecycle_acceptance.cjs`、`rssr_reading_position_acceptance.cjs`；真实 IME、选择复制/系统菜单须宿主验证 |
| NAV-01 | 目录未挂载目标先布局再聚焦；筛选/翻页/离页后的旧回复失效；手动滚动压过迟到自动跟随 | `rssr_directory_window_boundaries.cjs`、`rssr_directory_window_review.cjs`；新宿主需验证布局提交及像素单位 |
| NAV-02 | 阅读返回、重复返回、图片覆盖层关闭、没有历史时 fallback；用户滚动立即终止旧恢复 | 位置运行器 + Android 实际系统返回；不能只发送 DOM click |
| TOUCH-01 | 下拉阈值、松手最终坐标、多指、无坐标、嵌套滚动与取消；只允许一次刷新 | `gesture.rs` 用例 + 实际触摸宿主；桌面合成触摸不足以证明设备行为 |

所有脚本均位于 `scripts/browser/`。仅改文档时核对链接、源码与证据即可；涉及 Rust 规则时运行相应测试、格式和 Clippy；涉及页面/装配时构建实际候选并执行[Headless 同路径验收](./headless-refactor-equivalence.md)。全局发布仍按[发布清单](./release-ui-regression-checklist.md)，本文不取代发布门禁。

可复用的命令形状（示例，需先准备隔离 fixture 和准确产物；不代表已执行）：

```powershell
cargo test -p rssr-app --locked settings_page::
$env:PUBLIC_DIR = '<candidate>/public'
node scripts/browser/rssr_settings_sync_acceptance.cjs --scenario lifecycle --artifact-dir '<evidence>/web-load'
node scripts/browser/rssr_theme_files_acceptance.cjs --scenario ownership --artifact-dir '<evidence>/web-import'
```

Windows CDP 运行器只接受显式独立进程、profile 和复制库，先核对当前 Entries 页及数据库身份再写入。现有主题文件运行器是 Web 文件路径验收，**没有 Windows OS 选择器驱动能力**；不要给它加 `--cdp` 就记为原生文件验收。

## Windows 系统 CSS 文件流程（验收规范）

范围限定在现有 `theme_file_io.rs` / `theme_io.rs` 的桌面适配与验收工具。先跑现有实现，只有发现可复现缺陷才修改对应边界；不引入新渲染器、新设置状态或新的跨平台能力。

### 环境与样本

1. 固定待验源码、锁文件、EXE SHA-256 和运行副本。记录 Windows / WebView2 版本、profile、进程路径/PID、测试库绝对路径；窗口标题不能独自证明目标身份。
2. 建立新的测试目录，将 EXE 复制到其中，并把已 checkpoint 的 `rss-reader.db` / `rss-reader-content.db` 放入该 EXE 同目录下的 `RSS-Reader/`。Windows 数据目录由[可执行文件位置](../../crates/rssr-infra/src/db/sqlite_native.rs)决定，只改工作目录或 WebView profile 不会隔离数据库。只从已知 fixture 复制；本机历史样本是 `target/reading-baseline-20261025/seed-matched/`，缺失时先重新准备，不改用用户库。设置独立 `WEBVIEW2_USER_DATA_FOLDER`；保留源样本及其哈希，不向源样本写入。
3. 启动前只读核对复制库的 5000 个固定 ID/标题和 6 个订阅，将**复制库**中的 feed URL 指向本次独占的回环服务，保留来源 ID 并记录这项准备差异；服务返回受控响应，不能依赖旧 fixture 端口仍属于原测试。复制库和 EXE 路径确认后再启动；CDP 再核对当前 Entries 页与复制库一致，失败即停止，不执行设置准备或文件动作。
4. 当前历史 fixture 的 `archive_after_months` 为 **0**，合法设置保存要求至少为 1；有效 CSS 导入会保存整份设置，因此直接照旧样本执行 W-FILE-01 会因归档阈值失败。身份检查通过后，按[现有 Settings 验收准备](../../scripts/browser/rssr_settings_save_acceptance.cjs)的做法，在隔离页面将该值设为 120 并明确保存，等待 pending 释放，再只读核对数据库。只对值为 0 的旧样本做这项准备，其他有效值不改；不要放宽生产校验。准备差异单独记录，确认文章标记和来源集合未被意外改变，随后捕获 W-FILE 的设置/标记/来源基线。
5. 准备有效中文 CSS、空文件、无效 CSS、无效 UTF-8、带 BOM/CRLF 的文件及专用可覆盖输出文件；保存每个样本的原始字节和 SHA-256。用可控制的临时路径构造真实读写失败，失败注入方法和实际发生阶段必须可证明。全部文件选择及导出只使用测试目录；需要修改构建时，保存旧候选，做相同路径比较。
6. 系统 GUI 验收须看到或可靠定位实际 OS 对话框及其所属进程，记录打开/选择/取消/返回焦点；页面可经 CDP 操作，但 DOM 文件赋值、注入 Future 和文件 I/O 单测不能替代系统对话框。启动辅助进程保持隐藏，只有这项验收需要交互的测试窗口进入可见状态。

### 必跑场景

| ID | 操作 | 通过标准 |
| --- | --- | --- |
| W-FILE-01 | 在 Settings 打开系统导入框，选择有效 CSS；完成后重开页面 | 实际出现 OS 选择器；读取、校验、原保存流程和主题更新完成；重开持久化值一致，源文件未变，全部文章标记/订阅保持 |
| W-FILE-02 | 取消导入、再次导入、重复选择同一文件；另试空文件 | 取消显示桌面既有提示且不保存；后续能重试；同名文件可重选；空文件可导入空 CSS |
| W-FILE-03 | 选无效 CSS / 无效 UTF-8；选定后让专用文件不可读或消失 | 各失败阶段有真实证据；草稿和已保存主题不变，应用不改写源文件（测试主动注入的删除/权限变化另记）；错误可恢复。无法稳定复现的阶段记未覆盖，不能用单测冒充 GUI 通过 |
| W-FILE-04 | 通过系统保存框导出未保存 CSS，另执行取消；在专用输出文件上验证覆盖确认 | 导出使用点击时文本，不隐式保存设置；成功文件逐字节一致，取消不改目标；空白 CSS 不打开保存器；覆盖只作用于样本文件 |
| W-FILE-05 | 导出到确实不可写的专用目标，随后改到可写目标重试 | 返回真实失败并保留草稿；重试可成功，不遗留占用或永远 busy。OS 在选择阶段拒绝路径与真正写入失败分开记录 |
| W-FILE-06 | 等待文件结果期间离页/返回；重新打开操作；检查焦点恢复 | 旧页面不发布迟到提示或草稿；新页仍可操作；实际 OS 窗口存续与已发生写入如实记录，不以“离页必须关框/回滚”判失败。若模态导致无法离页，此子项记受限，Dioxus 测试另列 |
| W-FILE-07 | 系统窗口中 Tab、Shift+Tab、Esc、中文路径/输入法；关闭后继续编辑和保存 | 真实输入法不误关闭或触发页面快捷键；焦点返回可用且不额外滚动；显示缩放和前台截图单独记录，不能用隐藏 CDP 截图判视觉通过 |

如果 GUI 无法观测或时序无法复现，保留已完成项与失败/未覆盖项。不能把 Web 取消、桌面 `None` 分支测试或 Android 编译检查填进 Windows 系统窗口通过栏。Android 系统保存器回调、Linux/macOS 选择器和新原生表单不属于这批的完成声明。

### 完成与交接

- 每个 W-FILE 场景保存结果、操作方式、前后文件/设置快照和日志；如果修改实现，补复现用例及相应 Web/桌面回归，避免把既有平台差异统一掉（Web 去 BOM，桌面保留）。
- 报告 UI 成功、真实持久化、视觉、资源清理四类结果；关闭只针对本次创建且路径匹配的进程，读回确认释放自己的端口/句柄。保留样本和证据，不做工作区清理。
- 用 `docs/handoffs/TEMPLATE.md` 留下基线/候选身份、执行命令、平台范围、失败与未覆盖项，完成相称验证后按任务路径本地提交。若缺关键系统 GUI 证据，状态写“部分验收”，不宣称原生适配完成。

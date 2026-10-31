# 原生控件前置验收

核对日期：2026-10-30。PR #40、返回策略 PR #46 和 App 设置读取 PR #47 已进入 main；本日分别补齐所列返回与主题选择的实际宿主检查。当前契约见[控件适配契约](../design/native-control-adapter-contracts.md)，各次源码、环境与边界见[返回专项回执](../handoffs/2026-10-30-native-back-validation.md)和[主题选择评估回执](../handoffs/2026-10-30-native-theme-selector-assessment.md)。历史结果不能替代后续源码验收。

## 当前入口

| 范围 | 命令/入口 | 证明的边界 |
| --- | --- | --- |
| Rust 规则与 Dioxus 接线 | cargo test --workspace --locked | 保留 main 全部用例，补写入规则、编辑状态、文件归属、逐字段数值提示；不证明 OS 控件 |
| Reader 规则独立性 | python scripts/check_reader_rule_boundary.py | 用生产模块及其测试编译独立程序，仅链接现有 API/domain/serde，拒绝框架依赖；不创建替身状态或新 Cargo package。app 集成测试继续约束任务与发布顺序 |
| 返回与退出提示 | cargo test -p rssr-app --locked use_mobile_back_navigation | 普通 Rust 事件序列、Dioxus 提示投影、捕获完成/失败/超时/取消/路由变化与旧监听清理；不证明实际 Android 返回 |
| 静态检查 | cargo fmt --all --check；cargo clippy --workspace --all-targets --locked -- -D warnings | 编译、格式与 lint |
| 正式 Web | dx build --platform web --package rssr-app --locked --release --debug-symbols false | 当前 HTTP Backend 包；不使用 pages-demo |
| Entries/来源编辑/阅读 | rssr_entries_lifecycle_acceptance.cjs、rssr_workspace_acceptance.cjs，以及主线 rssr_server_acceptance.mjs | 真实服务器、单库、离页回执、失败恢复、偏好顺序、目录与返回 |
| Settings/主题接入 | node scripts/browser/rssr_native_preparation_acceptance.cjs | 1536/390 两视口，PATCH 字段边界、保存与后续编辑、下载回填、字段提示、导入仅预览与显式保存、UTF-8/导出、迟到文件 |
| 原生字符串表 | rssr_native_transport_acceptance.cjs --process-file <run>/process.json --artifact-dir <evidence> | 实际 Windows EXE/WebView2、单库与传输；不操作 OS 选择器 |
| 原生返回专项 | rssr_back_navigation_acceptance.cjs --process-file <run>/process.json --artifact-dir <evidence> --fixture-dir <fixture> | Windows 实际 WebView2；Android 独立测试包的系统返回、保存门禁、采集失败/超时/取消、首页提示与恢复前台 |
| Reader 宿主专项 | 上行命令加 --reader-only true | 只运行图片关闭/回焦/位置恢复、M/F 写入保留正文、输入排除、J/方向键切文与工具栏返回；Android 图片关闭使用实际系统返回。repeat/composing 为受控事件，不证明物理键盘/系统 IME；不需要保存锁工具 |
| Reader 宿主生命周期与快捷键 | 上行 Reader 命令另加 --reader-host true；正式 Web 用 node scripts/browser/rssr_reader_host_acceptance.cjs | 共用 reader_host_scenarios.cjs 和 reader_shortcut_scenarios.cjs；检查三种关闭/离页、焦点/滚动/body 样式、监听/observer 清理，以及按键过滤、同步默认动作和切文前捕获。正式 Web 为真实服务器与隔离 SQLite，进入 reading-workspace CI |
| 主题选择宿主专项 | rssr_theme_selector_windows.mjs / rssr_theme_selector_android.mjs | Windows 实际 EXE + UIA + CDP；Android 真机系统列表、adb 选择/返回、预览及显式保存。不是新增 OS 控件或完整辅助技术验收 |
| Android 主题文件导入 | rssr_theme_import_android.mjs | 实际 DocumentsUI + ContentResolver + JNI；测试包专属提供器控制文件、读取错误和延迟；与真实云端提供器、正式包权限验收区分 |
| Android 编译 | cargo check -p rssr-app --target aarch64-linux-android --locked 及 PR android-smoke | 仅编译/CI smoke，不能替代上行真机检查；选择器、IME 和辅助技术另验 |

Web 脚本复用 directory_fixture.cjs 的真实同源服务器与隔离 library.sqlite3，须同时准备本次源码的 rssr-web、browser_fixture_library 和 Web 包。用 RSSR_WEB_BIN、RSSR_FIXTURE_BIN、PUBLIC_DIR 指定产物，ARTIFACT_DIR 指定证据；Playwright 使用 CHROME_BIN 或 Chrome channel。运行器控制 HTTP 响应和 FileReader 的完成时机，不能把这些延迟注入宣称为真实网络故障。

新增 Settings 脚本进入 CI 的 reading-workspace 作业，失败保留 JSON、日志、截图；正常 PR 检查必须基于冲突解决后的新 head，旧分支 workflow_dispatch 成功不能代替。

Reader 正式 Web 运行器直接复用 `server_harness.mjs`，使用上述产物环境变量及 `target/` 下全新 `ARTIFACT_DIR`。它只在新夹具库加入本机图片，不访问线上来源。监听探针记录真实回调/AbortSignal 与 observer，不替换应用回调或业务状态；退出 Reader 时排除随后新建的 Entries 监听。浮层期间离页由受控路由动作触发，不是物理点击穿透 inert。滚动比较以 dialog 挂载时保存的位置为准，同时记录按键前和激活时位置；脚本化鼠标/按键不证明物理输入设备。完整基线/候选与采样修订见[Reader 宿主回执](../handoffs/2026-10-30-reader-host-boundary.md)。

快捷键场景打开列表第四篇，每个视口检查 70 个排除/阶段边界与 10 个有效键值，再检查离页后输入。它包装原 `Element.querySelectorAll` 观察正文位置扫描，记录目标节点事件到达前的扫描数和 `dispatchEvent` 返回时的 `defaultPrevented`；应用测量和回调仍执行原实现。有效按键实际导航并返回恢复位置，或等待标记状态改变再恢复；重复、组字、修饰键、可编辑祖先、通用 dialog、预先阻止与目标停止传播均为受控事实。用当前包同路径重跑基线/候选，不把合成 `KeyboardEvent` 写成系统键盘或 IME 实测，见[快捷键回执](../handoffs/2026-10-30-reader-shortcut-definition.md)。

表单与可编辑目标用未预先取消的按键验证排除，并断言宿主没有 `preventDefault`；不能让预先取消的门禁掩盖编辑目标过滤失效。`contenteditable="false"` 是允许捕获的对照，单独预先取消它以检查捕获阶段且不触发实际切文。已用移除 M/F 编辑目标过滤的受控故障确认：旧测试漏报，修正后的输入框 M 用例会失败；该故障注入不进入产品代码。

共享快捷键模块与页面装配的归属调整继续使用上述同一测试，不另造替身控制器或重复实现；独立编译直接加载移动后的生产 `policy.rs`。本次脚本哈希、模块依赖复核和真实三端基线/候选回执见[模块归属交接](../handoffs/2026-10-30-reader-shortcut-cohesion.md)。

正文渲染兼容矩阵见[正文、资源与锚点契约](../design/reader-rendering-boundary.md#兼容面与验证证据)。PR #50–#52 的整合没有改变其产品树；合并后的 Rust/CI 与既有实际宿主回执分开记录，不把主线编译或同树证明写成重新执行了真机验收，也不外推为候选渲染器通过。

## 验收约束

- 每次记录源码/产物身份、实际命令、成功/失败与未覆盖平台。
- 设置保存只发逐字段补丁；初次加载、下载、保存均以 stored/submitted/pending 规则投影。无法解析的输入不构造补丁，反馈按字段归属。
- 新文件选择器、后续 CSS/预设编辑、明确保存或下载准入后，旧文件成功/错误都不能改草稿或再次保存。
- process.json 的 pid/exe/profile/cdp 必须属于本次隔离进程，单库路径在其 target 下的运行目录内；核对 fixture 身份后才写入。
- 不修改用户原库，不用历史哈希或 mock 回执替代本次产物验收。
- 返回策略检查首页首次/再次返回、5 秒超时、旧提示超时与恢复前台、浮层关闭顺序、保存中消费返回、连续阅读及位置恢复。Web 与 Windows 小视口不能替代 Android；本批实测见[10 月 30 日记录](../handoffs/2026-10-30-native-back-validation.md)。

## 返回专项的隔离与复跑

运行器位于 [scripts/browser/rssr_back_navigation_acceptance.cjs](../../scripts/browser/rssr_back_navigation_acceptance.cjs)。调用者负责启动已校验的实际宿主；运行器不安装 APK、不重置资料库，也不附着日常应用。Playwright 依赖按机器环境配置。

1. 用 `scripts/seed_reading_native.py <seed> --fixture-dir <fixture> --fixture-base http://127.0.0.1:9299 --entries 100 --paragraphs 120` 生成独立资料库。原生进程各用一份副本，所有来源只指向本机夹具。运行器在 9299 端口提供六份 RSS；本批启动刷新发生在夹具服务监听之前，该刷新不作为验收场景。
2. Windows 把 EXE、`RSS-Reader/library.sqlite3` 和独立 WebView2 profile 放在仓库 `target/` 的新运行目录下。配置 `WEBVIEW2_USER_DATA_FOLDER` 和 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=<port>`；`process.json` 填写 `platform=windows`、`pid`、`exe`、`profile`、`cdp`、`artifactSha256`、`fixtureEntries=100` 及源码身份。运行器核对 EXE、CDP 进程归属及合成库内容后才操作。
3. Android 仅使用 `io.github.develata.rssreader.backpolicytest`。记录 `platform=android`、`serial`、`package`、`pid`、`apk`、`cdp`、`artifactSha256`、`fixtureEntries=100`、源码身份及 `lockHelperSha256`；核对安装包哈希及 CDP forward 对应 PID。实际系统按键之前还核对测试包拥有前台 Activity。仅添加本轮 CDP forward 与 `adb reverse tcp:9299 tcp:9299`，结束后只移除本轮映射。
4. 保存门禁使用 [native_back_save_lock.rs](../../scripts/native_back_save_lock.rs)，用 Android target、NDK linker 和当前 `libsqlite3-sys` 的 `libsqlite3.a` 编译，复制到独立测试包的 `files/native-back-save-lock`。它仅接受该包目录和指定的 100 篇/6 来源合成库，打开已有文件并持有 `BEGIN IMMEDIATE`；收到释放、EOF 或 12 秒到期均回滚，不提交数据。它使真实保存等待 SQLite 写锁，不通过修改 UI 状态制造“保存中”。
5. 采集故障使用 CDP 在实际 WebView 的捕获回复处注入错误或扣留回复，并断言注入确实命中。取消检查在 500 ms 到期前改变页面；计时来自页面捕获事件与 DOM 切换，包含宿主/渲染调度，不是仅凭 adb 命令耗时推定。运行器恢复探针并输出 JSON、截图；正常路径及故障注入须分别阅读。

本批具体启动/复制/编译命令、包与资料库散列以及失败诊断记录在交接指定的本地证据目录中。合成 Android 库预置存储标记只用于隔离测试，不证明旧数据目录迁移。

## 主题选择专项的隔离与复跑

原始主题场景与结论见[宿主评估](../design/native-theme-selector-assessment.md)；当前运行器另覆盖[列表密度退役](../handoffs/2026-10-30-density-retirement.md)。使用 Node 的 `node:sqlite`、已有 CDP 模块和系统 adb/UIA，不新增 npm 依赖。

- Windows：`node scripts/browser/rssr_theme_selector_windows.mjs --app-bin <exe> --seed-dir <seed> --artifact-dir <target下全新目录> --port 9495`。运行器复制 EXE 和合成库，建立独立 profile，核对 CDP 进程祖先。UIA 助手只显示本次测试窗口且不激活，不向全局键盘发送输入；按键经 CDP 进入 WebView2。调用者核对 EXE 对应的源码身份。后台截图和物理键盘不在其通过范围内。
- Android：调用者先备份和校验已有测试包资料，再仅安装 `io.github.develata.rssreader.backpolicytest`，预置独立库并启动。`process.json` 提供 `platform=android`、`serial`、`package`、`pid`、`apk`、`artifactSha256`、40 位 `source` 和 `cdp=http://127.0.0.1:<port>`。调用 `node scripts/browser/rssr_theme_selector_android.mjs --process-file <run>/process.json --artifact-dir <target下全新目录>`；运行器核对安装包、PID、data root、forward 映射和前台 Activity，不安装、重置资料库或管理映射。调用者结束自己的进程和映射。
- 两端夹具都必须是 100 篇文章、6 个仅指向 `127.0.0.1` 的来源。主题初始为 dark、旧 `list_density=compact`、启动视图 all，刷新间隔为合法的长周期 525600，归档月数为合法值 6；当前脚本确认旧 compact 不影响默认行高且界面没有密度控件。更新 SQLite 时只改 `app_settings.key='user_settings'`，保留 `app_state_v2`。不能把旧夹具的归档月数 0 当成合法设置保存前提。
- 从未提交快照构建时，`source` 记录基线提交，同时提供 `sourcePatchSha256` 和逐文件输入清单。不同源码快照使用独立 Android 构建目录；打包后核对 APK 内 `libmain.so` 与本轮编译输出相同，不能只凭 `dx build` 成功沿用共享 target 的旧 APK。
- Android 记录真实设备截图与 UI 树；系统返回使用 adb keyevent 4，不冒充手势返回。UIAutomator 的只读快照最多重试一次，失败也留在报告中。SQLite 在稳定 UI 检查点复制主文件及 WAL；安装前的保全备份则先停止测试包再完整归档。
- 本批未运行夹具 RSS 服务，启动刷新对本机地址失败不属于主题专项断言。未访问线上订阅，也不据此宣称离线刷新功能通过。

## Android 主题导入的复跑

沿用上文独立包、资料库与 `process.json` 约束。先归档并校验旧测试 APK / files，在独立源码副本仅修改包 identifier / BuildConfig 引用；用独立 target 构建，保留不含测试提供器的 APK。对生成的 `app/src/main` 运行 `node scripts/android/theme-import/install-fixture.cjs <main目录>`，再 Gradle `assembleDebug`。该脚本限定当前工作树 target 下的独立测试包，把 [ThemeImportDocuments.kt](../../scripts/android/theme-import/ThemeImportDocuments.kt) 加入生成工程，不修改产品 manifest；比较两份 APK 的 `libmain.so` 与本轮编译输出。

安装并启动已核验的测试包后运行 `node scripts/browser/rssr_theme_import_android.mjs --process-file <run>/process.json --artifact-dir <target下新目录>`。它用实际系统列表选择、系统返回取消和真实 JNI，检查 UTF-8 / BOM / CRLF、65536 / 65537 字节、无效 CSS、提供器错误、后续编辑、离页取消 / 忙碌 / 重试、明确保存与重开。提供器大小元数据故意不可信；延迟和错误是受控夹具，不代表云盘服务实测。界面状态改变使用 CDP，系统文件交互使用受前台 Activity 约束的 adb 输入。

`--bom-mode baseline|fixed` 单独复现和回归样式首 BOM 问题，适用于不含提供器的旧测试包；基线应保留缺陷证据，不能列为产品验收通过。失败时只关闭确认由该测试包打开的前台选择器。调用者始终停止自己的测试进程、恢复原资料库、校验日常包并移除本轮 forward；保留失败回执，不覆盖成功记录。实际版本与范围见[本批交接](../handoffs/2026-10-30-android-theme-import.md)。

## 已退役的旧分支运行器

PR 原先的 feeds/reader/settings/theme 独立生命周期脚本及四份共享场景依赖 BrowserStore、Web Locks 或整份草稿相等语义。本次以主线现成真实服务器回归和新的 rssr_native_preparation_acceptance.cjs 替换；旧实现仍可从 PR 原 head 58157a1a 查阅。历史交接内命令仅描述当时运行，不应在当前架构直接执行。

Windows 系统文件选择/保存框、覆盖确认、真实 I/O 失败，以及本次返回专项之外的 Android 交互、macOS/Linux GUI 仍需各自证据。PR #40 的整合范围见[整合交接](../handoffs/2026-10-29-pr40-main-integration.md)。

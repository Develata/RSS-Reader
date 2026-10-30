# 原生控件前置验收

核对日期：2026-10-30。PR #40 已进入 main；返回策略 `62113c4f` 仍在候选分支，本日补齐所列 Windows / Android 实际宿主检查。当前契约见[控件适配契约](../design/native-control-adapter-contracts.md)，各次源码、环境与边界见[返回专项回执](../handoffs/2026-10-30-native-back-validation.md)和[主线合并回执](../handoffs/2026-10-29-pr40-final-review-merge.md)。历史结果不能替代后续源码验收。

## 当前入口

| 范围 | 命令/入口 | 证明的边界 |
| --- | --- | --- |
| Rust 规则与 Dioxus 接线 | cargo test --workspace --locked | 保留 main 全部用例，补写入规则、编辑状态、文件归属、逐字段数值提示；不证明 OS 控件 |
| 返回与退出提示 | cargo test -p rssr-app --locked use_mobile_back_navigation | 普通 Rust 事件序列、Dioxus 提示投影、捕获完成/失败/超时/取消/路由变化与旧监听清理；不证明实际 Android 返回 |
| 静态检查 | cargo fmt --all --check；cargo clippy --workspace --all-targets --locked -- -D warnings | 编译、格式与 lint |
| 正式 Web | dx build --platform web --package rssr-app --locked --release --debug-symbols false | 当前 HTTP Backend 包；不使用 pages-demo |
| Entries/来源编辑/阅读 | rssr_entries_lifecycle_acceptance.cjs、rssr_workspace_acceptance.cjs，以及主线 rssr_server_acceptance.mjs | 真实服务器、单库、离页回执、失败恢复、偏好顺序、目录与返回 |
| Settings/主题接入 | node scripts/browser/rssr_native_preparation_acceptance.cjs | 1536/390 两视口，PATCH 字段边界、保存与后续编辑、下载回填、字段提示、导入仅预览与显式保存、UTF-8/导出、迟到文件 |
| 原生字符串表 | rssr_native_transport_acceptance.cjs --process-file <run>/process.json --artifact-dir <evidence> | 实际 Windows EXE/WebView2、单库与传输；不操作 OS 选择器 |
| 原生返回专项 | rssr_back_navigation_acceptance.cjs --process-file <run>/process.json --artifact-dir <evidence> --fixture-dir <fixture> | Windows 实际 WebView2；Android 独立测试包的系统返回、保存门禁、采集失败/超时/取消、首页提示与恢复前台 |
| Android 编译 | cargo check -p rssr-app --target aarch64-linux-android --locked 及 PR android-smoke | 仅编译/CI smoke，不能替代上行真机检查；选择器、IME 和辅助技术另验 |

Web 脚本复用 directory_fixture.cjs 的真实同源服务器与隔离 library.sqlite3，须同时准备本次源码的 rssr-web、browser_fixture_library 和 Web 包。用 RSSR_WEB_BIN、RSSR_FIXTURE_BIN、PUBLIC_DIR 指定产物，ARTIFACT_DIR 指定证据；Playwright 使用 CHROME_BIN 或 Chrome channel。运行器控制 HTTP 响应和 FileReader 的完成时机，不能把这些延迟注入宣称为真实网络故障。

新增 Settings 脚本进入 CI 的 reading-workspace 作业，失败保留 JSON、日志、截图；正常 PR 检查必须基于冲突解决后的新 head，旧分支 workflow_dispatch 成功不能代替。

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

## 已退役的旧分支运行器

PR 原先的 feeds/reader/settings/theme 独立生命周期脚本及四份共享场景依赖 BrowserStore、Web Locks 或整份草稿相等语义。本次以主线现成真实服务器回归和新的 rssr_native_preparation_acceptance.cjs 替换；旧实现仍可从 PR 原 head 58157a1a 查阅。历史交接内命令仅描述当时运行，不应在当前架构直接执行。

Windows 系统文件选择/保存框、覆盖确认、真实 I/O 失败，以及本次返回专项之外的 Android 交互、macOS/Linux GUI 仍需各自证据。PR #40 的整合范围见[整合交接](../handoffs/2026-10-29-pr40-main-integration.md)。

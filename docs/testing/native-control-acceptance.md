# 原生控件前置验收

核对日期：2026-10-29。本文件对应 PR #40 合入 main 后的实现，当前契约见[控件适配契约](../design/native-control-adapter-contracts.md)，实际提交与检查见[最终合并回执](../handoffs/2026-10-29-pr40-final-review-merge.md)。此前各批证据保留在[历史交接](../handoffs/README.md)，不能替代整合后源码的验收。

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
| Android | cargo check -p rssr-app --target aarch64-linux-android --locked 及 PR android-smoke | 编译/CI smoke；真实系统返回、选择器、IME 和辅助技术须另验 |

Web 脚本复用 directory_fixture.cjs 的真实同源服务器与隔离 library.sqlite3，须同时准备本次源码的 rssr-web、browser_fixture_library 和 Web 包。用 RSSR_WEB_BIN、RSSR_FIXTURE_BIN、PUBLIC_DIR 指定产物，ARTIFACT_DIR 指定证据；Playwright 使用 CHROME_BIN 或 Chrome channel。运行器控制 HTTP 响应和 FileReader 的完成时机，不能把这些延迟注入宣称为真实网络故障。

新增 Settings 脚本进入 CI 的 reading-workspace 作业，失败保留 JSON、日志、截图；正常 PR 检查必须基于冲突解决后的新 head，旧分支 workflow_dispatch 成功不能代替。

## 验收约束

- 每次记录源码/产物身份、实际命令、成功/失败与未覆盖平台。
- 设置保存只发逐字段补丁；初次加载、下载、保存均以 stored/submitted/pending 规则投影。无法解析的输入不构造补丁，反馈按字段归属。
- 新文件选择器、后续 CSS/预设编辑、明确保存或下载准入后，旧文件成功/错误都不能改草稿或再次保存。
- process.json 的 pid/exe/profile/cdp 必须属于本次隔离进程，单库路径在其 target 下的运行目录内；核对 fixture 身份后才写入。
- 不修改用户原库，不用历史哈希或 mock 回执替代本次产物验收。
- 返回策略的实际宿主验收须另测首页首次/再次返回、5 秒超时、旧提示超时与恢复前台、浮层关闭顺序、保存中消费返回、连续阅读及位置恢复。Web 与 Windows 小视口不能替代 Android；本批编译/运行范围与阻塞见[返回策略交接](../handoffs/2026-10-29-native-back-policy.md)。

## 已退役的旧分支运行器

PR 原先的 feeds/reader/settings/theme 独立生命周期脚本及四份共享场景依赖 BrowserStore、Web Locks 或整份草稿相等语义。本次以主线现成真实服务器回归和新的 rssr_native_preparation_acceptance.cjs 替换；旧实现仍可从 PR 原 head 58157a1a 查阅。历史交接内命令仅描述当时运行，不应在当前架构直接执行。

Windows 系统文件选择/保存框、覆盖确认、真实 I/O 失败、Android 真机以及 macOS/Linux GUI 验收仍需各自证据。本轮准确执行结果见[整合交接](../handoffs/2026-10-29-pr40-main-integration.md)。

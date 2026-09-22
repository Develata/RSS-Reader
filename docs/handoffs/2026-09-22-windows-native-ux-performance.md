# Windows 原生阅读体验、首次列表偏好与跨端构建复核

- 日期：2026-09-22
- 作者 / Agent：Codex
- 分支：`main`
- 起始 HEAD：`c11e859b6be954c8581fdbabbde0f9f5e3c15c87`；写入本记录时 HEAD：`e496c6d`
- 相关 commit：`8988add`、`e496c6d`；验收工具、Web 标题模板与本记录见本文件所在提交
- 相关 tag / release：N/A；未 push、打 tag 或发布
- 状态：`validated`（Windows 原生与 Web；Android/macOS 实机未验）

## 工作摘要

在真实 Windows 11 WebView2 窗口和隔离 SQLite 数据上检查 800 / 2000 篇文章的阅读路径，修复了首次列表可能短暂使用默认分页与筛选、原生标题和图标不正确、Reader 刷新结果遮挡标题，以及 Android 返回键事件映射缺口。扩展已有浏览器断言直接连接原生 WebView；共享业务仍由 Rust command/runtime/application 持有。

## 影响范围

- 模块：`crates/rssr-app/src/pages/entries_page/`、`ui/runtime/entries.rs`、`app.rs`、`main.rs`、`hooks/use_mobile_back_navigation.rs`、`crates/rssr-app/index.html`、`assets/styles/shell.css`。
- 验收：`scripts/browser/cdp_session.mjs`、`rssr_small_viewport_assertions.mjs`、`scripts/run_static_web_small_viewport_smoke.sh`、相关测试文档。
- 平台：Windows 原生实测；Web release / Chrome 实测；Android 仅 target 编译与纯 Rust 事件测试；macOS 无设备。
- workflow：未修改或触发远程 CI。

## 关键变更与边界

### 首次列表与重复初始化

旧版的 Bootstrap 与 LoadEntries 会同时启动；已保存 `entries_page_size=50` 的 800 篇 fixture 曾在首次可用 DOM 显示 **100 张卡、1 / 8 页**，随后才跳为 50 张卡、1 / 16 页。证据：`target/native-device-review/final800-5.startup.json`。同一测试环境中，新版首次可用快照为 50 张卡；2000 篇为 1 / 40 页。

页面内 `Pending / Loaded / Unavailable` 让首次查询等待设置、工作区偏好和来源 URL→ID 映射全部应用。读取失败允许默认读取、保留错误提示，但不保存回退值覆盖原偏好；后续刷新可重试。Bootstrap 不因自己的成功再次取来源，独立世代丢弃晚到的旧成功或旧失败；列表查询仍有自己的世代，两类任务可以并行。规则位于共享 Rust 页面会话和 reducer，没有平台分支或新 crate/service。

### 实机交互

- Tao 0.34.8 的 Android `KEYCODE_BACK` 为 physical `Unidentified(Android(4))` + logical `BrowserBack`；原 hook 漏掉该 logical key。现以 pressed、非 repeat 的纯判定函数统一原有返回路径，测试该实际键组合；仍需 Android 真机确认系统行为。
- Native 启动时的 `document.title` 曾是 `Dioxus app`，不存在的 `/favicon.ico` 导致 console 404。桌面和 Android host 在挂载前注入固定产品标题与编译期 SVG data URI 图标；窗口标题和共享 App 名称一致。没有动态 HTML、远端图标请求或业务 adapter 分叉。
- 原生 360×800 的 Reader 截图和几何显示“刷新完成”提示位于 y=140–177，标题从 y=151 开始。Reader 中把结果文字保留在裁剪的 live region 与 R 的完整 title，并以 R 上的小标记显示完成/错误；进行中动画保留。Web 初始模板旧标题还会与 Dioxus `document::Title` 拼成 `RSS-Reader BaselineRSS-Reader`，已清除旧占位文字，检查最终构建 HTML 初始标题恰好为 `RSS-Reader`。
- 原生验收直接附着明确 ID 的已有 Dioxus WebView2 target，验证 URL/Windows UA，不创建标签页、注入 localStorage、模拟视口或关闭宿主窗口；复用既有 shell、来源、分页、Reader、图片、选择、console 断言。CDP 握手有超时和关闭退出。一次 43.999996 CSS px 的 44px 控件测量用 0.01px 浮点容差处理，43px 仍拒绝。

## 验证与验收

### 自动化验证

- `source /tmp/rssr-native-deps/env.sh && cargo fmt --all --check`：通过。
- `cargo clippy --locked --workspace --all-targets -j 2 -- -D warnings`：通过。
- `cargo test --locked --workspace -j 2`：**279 passed、0 failed、2 ignored**；ignored 是显式的性能探针。Entries targeted 为 39 passed、0 failed、1 ignored，覆盖初始化门控、失败降级/禁止写回、恢复，以及旧 Bootstrap 成功/失败乱序和独立查询世代。
- `cargo check --locked -p rssr-app --target wasm32-unknown-unknown`：通过。
- Windows MSVC `cargo build --locked --release -p rssr-app -p rssr-cli -j 4`：通过。最终隔离 exe SHA-256：`fe87ded1f7170d458aa369451a7c483418679e1a16b084d54ffbe7ed46af1072`；前测 exe：`70e67f68e83f4b8a22a659410a50e07bd51863f27ae9baaa5362765968b22c5a`。
- Windows 已安装 NDK 28.2/API 34 上 `cargo check --locked -p rssr-app --target aarch64-linux-android -j 2`：通过；与发布 CI 的 NDK 27.3 不等同。
- `/tmp/rssr-dx-0.7.9/dx build --platform web --package rssr-app --release --locked --debug-symbols=false`：通过；最终 `index.html` 仅有 `<title>RSS-Reader</title>`。
- `bash scripts/run_static_web_small_viewport_smoke.sh --release --skip-build --port 18801 ...`：**117 项通过，console error 0**；360×800 DPR3 和既有 1280×800 Web 场景。最终结果：`target/native-device-review/web-fixed-final-title/`。
- `node --check` 两个修改的浏览器模块、`bash -n` smoke shell、`git diff --check`：通过。native target 缺失、HTTP 503、WebSocket 无握手等注入失败曾验证为非零退出与失败报告；不把旧的通过报告当作本次结果。

### Windows 原生隔离验收与性能范围

Windows 11 build 26200、i7-12700H、WebView2 153.0.4234.48、屏幕 2560×1440。exe 和 SQLite 分别放在独立的含中文与空格路径中，WebView profile 只在测试子进程中设置。40 个 loopback 订阅、800 / 2000 篇文章、保存的每页 50 篇；HTTP feed 请求提供 304 和计数。GUI 串行执行。测试未触及日常安装或数据库。

| 真实窗口 / 数据 | 断言 | 十次翻页中位数 | 十次打开 Reader 中位数 | 十次返回中位数 |
| --- | ---: | ---: | ---: | ---: |
| 1280×900 / 800，前测 | 86 通过 | 340.9 ms | 50.9 ms | 327.1 ms |
| 1280×900 / 800，最终 | 86 通过 | 111.2 ms | 27.3 ms | 107.0 ms |
| 1280×900 / 800，最终另一次 | 84 通过 | 128.9 ms | 27.4 ms | 112.0 ms |
| 1280×900 / 2000，前测 | 86 通过 | 130.4 ms | 31.3 ms | 145.8 ms |
| 1280×900 / 2000，最终 | 86 通过 | 111.4 ms | 26.6 ms | 122.3 ms |
| 1280×800 / 2000，最终 | 86 通过 | 126.0 ms | 27.4 ms | 117.3 ms |
| 360×800 / 2000，最终 | 86 通过 | 309.3 ms | 27.4 ms | 314.9 ms |

“最终另一次”只有 84 项，是没有预先完成手动刷新时，2 项结果标记条件断言未触发；全部实际运行断言通过。时间是 WebView `performance.now()` 中真实点击事件到预期 DOM 状态加两帧，排除 CDP 传输、点击前滚动与屏幕显示时间。800 前测与后测差别大，但旧版补充重复测试受自动刷新和测试脚本连续无等待点击影响而失败，缺少稳定的同条件重复基线；**不据此宣称固定百分比提速**。首次可用耗时也在 1.3–3.2 秒间波动。单次进程私有内存快照包含 WebView 子进程（约 352–616 MiB），时点与缓存状态不同，不能推断泄漏或内存改善。原始时间、环境、截图与错误见上述 `target/native-device-review/windows*-{before,after}-init*/assertions.json` 和 `*.startup.json`。

四个有效的前/后测刷新路径均观测到三次快速 R 只增加 40 个 feed 请求；任务进行时进入 Reader，刷新完成后正文、标题和 `scrollY=500` 保持；Reader→Home 不再额外请求。原生 360×800 截图复核标题未被刷新提示遮挡，长来源完整可读、无横向溢出，正文拖选、Ctrl+A、图片查看与关闭后焦点/滚动恢复通过。测试用 data URI 图片，无证据覆盖 sanitizer 的所有输入。

## 结果

共享状态/命令没有被移入 Kotlin、JS 或 application/domain 的平台分支。当前代码可供 review；Windows 原生、Web release 和目标编译验证已通过。UI 测试是构建后的独立窗口 / 服务器，不代表安装包或实际远程发布。

## 风险与后续事项

- 用户目前只有 Windows：Android 系统返回、长按选择、下拉、SAF、safe area、发版 NDK 27、Android 实机性能和 macOS 均未运行。编译成功不能代替这些验收。
- 未实测 Windows 系统剪贴板 Ctrl+C→外部应用粘贴、真实屏幕阅读器播报、不同 DPI / 系统主题、安装包及长时间内存增长。Web 的浏览器断言不能代替原生系统能力。
- 一次额外的旧版重复交互测试在启动自动刷新尚未静止时错误地将余下 10 个请求当作完整手动刷新；继续向 Reader 注入连点后出现 CDP 超时、测试窗口未在 10 秒内关闭。已核对 exe 路径并只结束该隔离测试进程树；最终版独立启动与常规回归通过，但该异常未形成可归因的产品复现。后续若研究此路径，应先用用户可执行的逐步点击和稳定计数重现，再定位 WebView/应用原因。
- `target/native-device-review/` 与 Windows Temp 的隔离工作目录保留为本机证据，未纳入 Git；不要把其中的历史产物当成新版本的验收收据。

## 给下一位 Agent 的备注

先看 `entries_page/session.rs` 的两个 generation、`ui/runtime/entries.rs` 的 intent 顺序，以及 `docs/testing/manual-regression.md` 的 Windows 数据路径与 native target 操作。原生调试必须明确指定本次启动的 target / PID；不要连接用户已有窗口。继续 Android 实机调优时需真机和对应 NDK/发布包，Windows WebView2 结果仅证明共享 UI 在本机的路径。

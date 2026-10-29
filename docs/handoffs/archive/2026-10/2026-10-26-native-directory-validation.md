# 目录可靠性、时间文章层级与最新原生验收

- 日期：2026-10-26
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 起始 HEAD：`aff3c7e6`
- 当前源码 HEAD：`34a6320207e704c127aed34e99ef106a71856ac6`
- 相关 commit：`34a63202`（功能修复）；验收证据提交见本文件 Git 历史
- 相关 tag / release：N/A
- 状态：`validated`（下述限定范围）；真实输入法与 Windows 前台性能未验，不是发布全验声明

## 工作摘要

用户授权继续最新 Windows / Android 验收与目录性能定位，并要求修复前先评估架构边界。随后指出时间目录仍停留在日期，要求展示到文章。本轮先修复这两个功能缺口，再用同一源码候选构建并验收原生包，不迁入 v0.2 主线。

## 影响范围

- `rssr-app` 前端目录 transport、adapter、view、分组投影及回归脚本。
- 共享前端影响 Web、Windows / Linux / macOS、Android；已验 Web、Windows、Android，未验 Linux / macOS。
- 无新增 crate、依赖、数据库或 application/domain 职责变化。

## 关键变更

- 根因：原 `while let Ok(fact) = eval.recv::<Fact>()` 将消息解码失败等同于通道关闭，Rust 退出而 JS 仍在等待。修改前 Chrome 注入未知 kind 后 `whenSettled` 明确超时。
- `directory/transport.rs` 独立处理 Eval 错误分类：单条 Serialization 错误回复 `{rejected:true}` 并继续；通道关闭或拒绝回复发送失败则终止。JS 在私有单飞通道中释放该次等待，保留排队意图，后续正常更新重新观测。不重试损坏载荷，不更改控制器状态规则。
- 时间目录补为月份 → 日期 → 文章，保留日期跳转；复用来源目录的文章组件、`(index,id)` 校验和全局分页。DOM 观测两种分组下的具体文章，继续只挂载展开路径。
- 目录浏览器回归对时间与来源分别验文章层级、键盘展开、同页多选保持、跨页定位与清空、历史记录和具体文章跟随。新增独立 transport fault-injection 验收，不在生产中增加测试接口。
- 现有 ignored Rust 投影测量补充 5000 条规模；这项微测量不含 I/O、WebView 渲染，不与真机耗时混为一谈。

## 验证与验收

### 已完成

- `cargo test -p rssr-app --locked`：154 unit + 4 integration 通过，1 个已有手动性能测量 ignored。
- `cargo fmt --all --check` 与 `cargo clippy -p rssr-app --all-targets --locked -- -D warnings`：通过。
- `dx build --platform web --package rssr-app --locked`：通过，现有 dx 0.7.10 / Dioxus 0.7.9 提示仍在。
- Chrome `rssr_small_viewport_assertions.mjs --directory-only true`：151 项通过，覆盖两种目录与桌面 / 窄窗口。
- Chrome `rssr_directory_transport_acceptance.cjs`：未知类型、错误字段类型、null、数组、缺失字段五类真实 Eval 消息注入后，损坏请求不展开分支，下一条正常请求成功；无 uncaught page errors。
- transport Rust 测试覆盖坏消息之后的正常状态更新、关闭接收端不重试、拒绝回复发送失败终止。
- 实际查看时间目录桌面截图，已显示当日文章标题；后续原生证据不借用浏览器结果。

### 证据与待验项

- 根目录：`target/native-validation-20261026/`。
- 已有：`before-directory-fault.json`、`chrome-directory/assertions.json`、`chrome-transport/transport-acceptance.json`、`app-tests.log`、`web-build.log`、`time-directory-desktop.png`。
- 手机 Xiaomi 12 / serial `caa0c283` 已连接；已停止测试包并备份原 files/cache 下 RSS-Reader 数据到 `android-before.tar`，SHA256 `F3C1993812693BA67A03AE2C9C78604D3CEFFD13717B7BFD041A77FDA0CE5B2A`。没有清除或覆盖数据。
- Windows / Android 最新包实际操作、手机大目录性能分解与数据前后核对已补，见下文。

### 原生产物与实际交互

- Windows：`cargo build -p rssr-app --release --locked --target-dir target/reading-baseline-20261025/build-b` 通过。隔离副本 `target/native-validation-20261026/windows/rssr-app.exe`，SHA256 `6D883EBDA09BB1AACFF4588D799322F3A65BF45F924F8FE9EA5B6626DEB4913A`。使用复制的 5000 篇 / 6 来源测试库，没有使用用户主库。
- Android：`dx build --locked --platform android --package rssr-app --target aarch64-linux-android --release --debug-symbols false`，随后 `python scripts/prepare_android_bundle.py target/dx/rssr-app/release/android/app/app/src/main` 和生成工程的 `gradlew.bat assembleDebug --no-daemon` 通过。进程级 JDK 21；Rust release、Android debug wrapper 用于 WebView 检查，不是发布签名包。
- 第一次安装预检发现 dx 生成包名称是 `RssrApp`，安装脚本正确拒绝；运行仓库已有资源准备步骤后重新打包，没有放宽包名/名称校验。
- 最终 APK：`target/native-validation-20261026/rssr-android.apk`，SHA256 `A15F9CD56FB8621576E0257A8F3F47226CE1DCC9761D9909954F559B561CEDEC`。`scripts/install_android_test.ps1` 同包名覆盖安装，安装后实际文件哈希一致；没有清数据、卸载或修改手机安全设置。本次没有安装确认弹窗。
- 宿主：Windows WebView2 155、1280×900 CSS px；Xiaomi 12 / Android 15 API 35、WebView 152、392×872 CSS px、DPR 2.75。手机已有 5000 篇 fixture 与 Windows 的日期分布不同，不作两端速度对比。
- 本地脚本 `target/native-validation-20261026/native-directory.cjs` 通过 Playwright/CDP 连接各自 WebView，Windows 鼠标/键盘与 Android 触摸、系统 Back 事件作用于测试宿主。**Windows 36 项、Android 38 项通过**：时间/来源文章层级、键盘展开、目录浏览保持列表 DOM 身份与勾选、收起卸载、重开手动路径、手机 Back 保留目录滚动、同页定位保留选择、跨页清空、不新增 history、主列表滚动恢复文章级跟随。
- 两端各连续三次打开/滚动/返回同一缓存长文，120 段正文、4 张 640px 内嵌图片正常，阅读位置恢复，返回仍保持搜索范围。只证明本会话的返回恢复，未额外声称跨进程阅读位置恢复。
- 证据：`windows-acceptance-v4/acceptance.json`、`android-acceptance-v2/acceptance.json`，均无 uncaught WebView 错误。早期 harness 失败是重复运行未清选择、点击关闭/打开按钮自动滚到标题栏导致测量点变化；修正前置状态和观察时机后通过，没有为通过检查更改产品行为。
- 视觉检查：Windows `windows/time-directory.png` 与 `windows-acceptance-v4/time-article-jump.png`；Android 以 `adb exec-out screencap -p` 的 `android-time-directory-physical.png`、`android-large-drawer-physical.png` 为准，月份/日期/文章层级与浮层均正常。部分 CDP 截图出现滚动层合成错位，不把这些图片作为真机视觉结论。
- 用户正在使用电脑，明确要求优先 OpenCLI/Playwright，停止桌面 Computer Use。后续没有调用桌面鼠标键盘控制、激活窗口或切换用户标签页。真实系统输入法验证留待用户空闲；CDP 输入不算 IME 验收。
- 之前自动审批阻止了启动本地素材服务（仅返回 `blocked by policy`）；核实 fixture 的图片为 data URI 内嵌后，无须服务即可完成图片验收，没有绕过限制启动服务。

### 目录性能定位

- 源码依赖核对：目录展开/折叠只更改 `DirectoryState`；列表查询依赖 `query_request`，分组 memo 依赖 `EntriesPresenterInput`，均不读取目录展开状态。没有新增 SQL tracing，因此不报“实测 SQL 查询耗时为零”。DOM 身份检查是附加回归证据，不代替数据库调用计数。
- `cargo test -p rssr-app --release --locked --target-dir target/reading-baseline-20261025/build-b measure_entries_projection -- --ignored --nocapture --test-threads=1` 通过。5000 篇的独立 Rust fixture：时间 presenter 约 6.21ms、来源 presenter 约 2.72ms、来源分组树约 2.09ms；投影相等检查约 31ns。它使用 40 来源，不是手机同一数据库，不包含查询、UI 或 IPC，更不表示每次展开都重新分组。
- 本地 `directory-profile.cjs`：只记录“展开/收起当前来源月份”5 次循环，WebView timeline 标记覆盖每次输入到两次 rAF。Android 当前月份 **597 文章 / 1248 个目录元素**；展开 DOM 完成 37.4–52.8ms，中位 49.8ms；两次 rAF 中位 89.1ms；收起分别为 29.9ms / 37.4ms。两次 rAF 只是呈现代理，不是屏幕实际显示时间。
- 展开区间内渲染进程 trace 中位：`Layout` 20.27ms、`UpdateLayoutTree` 10.60ms、`FunctionCall` 20.79ms、`Paint` 1.51ms。事件可能嵌套，不能直接相加成总耗时。`android-profile/renderer-trace.json` 和 `profile.json` 保留原始数据。
- 同期 `simpleperf record --app ... -e cpu-clock -f 1000 -g --duration 35` 得到 7913 样本；采样含准备/恢复，且 kernel 符号受限，不改系统限制。`libmain.so` 2.35% 仅为这段 app 进程采样占比，不能据此声称目录 Rust 函数占比或完整 app+renderer CPU 占比。WebView 的布局归因以上述 renderer trace 为准。
- `gfxinfo` 记录 267 帧、15 janky（5.62%）；包含脚本准备与恢复，不作为单次目录展开或整个应用的 FPS/卡顿率。
- Windows 也采集了 10 个样本，但后台遮挡时两次 rAF 几乎固定约 2 秒，随后截图超时。这组呈现时延 **无效，不进入性能结论**；遵循用户不抢前台要求，没有激活窗口重测。文件 `windows-profile/profile.json` 保持 `fail`，未改写为通过。
- 本轮是诊断，不是受控 A/B，未宣称性能提升或内存下降；未新增内存基线。结论：来源目录大量可见节点的样式/布局是有证据的后续优化对象，无证据要求更改 SQLite、application/domain 或新增 crate。

### 手机数据保留

- 同一次安装前后停包备份 files/cache 双目录；只在电脑临时目录提取并比较 SQLite 行，未写回旧库。
- 最终 `android-final.tar` SHA256 `52F9B0F540BC8B09DA3B538B403E201CB103DD0A387B5E177A1E5BEC3B8E7418`，比较结果 `phone-data-comparison-final.json`。
- files/cache 两套库完整性检查均 `ok`。两处 5000 条文章、5000 条正文、迁移记录、同步配置和 GC 状态逐行一致，文章里的已读/收藏/归档状态未变；shell prefs 完全相同。files 的差异仅为 `app_settings.updated_at` 和 `feeds.last_fetched_at/updated_at`，设置值、来源名称/URL 及其他字段一致；cache 所有表逐行相同。
- 最后通过界面恢复 `source` 分组与 `05000` 搜索并关闭搜索栏，保留安装后的最新测试包。
- 清理了本任务专用 Windows 测试进程、Chrome 调试实例、8119 预览服务及 9262 ADB 转发；未清理 C 盘工作区或用户浏览器。

## 结果、风险与后续事项

- 目录功能修复已通过 Rust、Chrome 和上述 Windows/Android 原生路径。真实 IME、Windows 有效前台性能、Linux/macOS 与发布签名产物未验。
- 下一轮优先给目录定义可变高度行的可见范围模型：Rust 管文章身份、树展开、当前路径与目标页；宿主提供行高/视口/滚动，组件只挂载可见区与必要缓冲。先覆盖键盘焦点、未挂载目标定位、手动滚动恢复和跨页跳转，再替换渲染。不要用写死行高、切掉文章标题或重新查询来缩小节点数。
- 本轮没有直接引入虚拟化：测量定位已完成，保留已修复的文章层级和现有交互；下一步是局部前端渲染优化，不重新分配业务层职责，不迁 v0.2。
- 源码只在 E 盘当前阅读分支修改。保留无关 dirty / deleted / untracked 与其他 agent 的 Penpot 文档。

## 给下一位 Agent 的备注

- Windows 测量不可引用为有效响应时间；若用户仍在使用电脑，继续用后台 Playwright/CDP，不能为了测试抢系统输入。
- Android 同包名同签名 `install -r`，保留数据；Windows 使用隔离测试副本和已有测试数据。不得触碰 C 盘受保护的旧基线工作区。

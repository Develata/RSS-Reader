# 任务 1：原生、Android 与浏览器契约补验

- 日期：2026-09-25
- 作者 / Agent：Codex
- 分支：main
- 当前实现 HEAD：d490f35
- 相关 commit：实现 `d490f35`；本补验记录为所在的 `docs: record task1 platform validation` 本地提交。
- 相关 tag / release：N/A
- 状态：`draft`（部分平台运行验收仍受环境阻塞）

## 工作摘要

用户授权使用 Android Studio 配套模拟器补验，并继续原生全量验证和 subscription wasm 契约测试。原生测试、严格 clippy、Android ARM64 check 与真实 Chrome 契约测试已通过；桌面完成真实界面部分验收。Android APK 未完成构建安装，不能宣称模拟器功能通过。

## 影响范围

- 模块：只更新本记录与原实现 handoff 的后续状态链接；没有产品代码变更。
- 平台：Linux/WSLg 桌面、Web Chrome、Windows Android 工具链。
- 额外影响：本地测试工具与隔离 fixture；未改数据库 schema、Cargo 依赖、公开函数/trait/端口/data-* 接口，也没有 push、tag、release。

## 关键变更

### 验证环境

- 原生首次补验使用 `/tmp/rssr-native-deps` 解包的 GTK/WebKit 开发库，通过环境变量指定路径，没有系统安装。环境中断后该目录丢失；桌面运行库重新解包到忽略目录 `target/task1-platform-validation/deps/`。
- 固定 wasm-bindgen-test-runner 0.2.126 匹配 Cargo.lock；ChromeDriver 151.0.7922.34 匹配已有 Playwright Chrome 151。仅当前 harness 进程去掉代理变量后，通过真实浏览器执行 4 项契约测试。
- 使用现有 Windows Android SDK、NDK 28.2.13676358、JDK 21、Rust 1.97.0；`git archive HEAD` 的源码快照位于 `D:\rssr-task1\source`，构建产物位于 `D:\rssr-task1\build`。使用固定 DX 0.7.9，未升级产品依赖。
- 使用 Android Studio 配套 emulator CLI，创建隔离 AVD `rssr-task1-api37`（API 37 / x86_64），数据位于 `D:\rssr-task1\rssr-task1-api37.avd`，ini 位于 `E:\android-avd\rssr-task1`。未操作用户原有 AVD。
- 原生 GUI 使用独立数据库 `target/task1-platform-validation/native/RSS-Reader/`、合成 feed/entry 991 与 `TZ=America/New_York`，没有访问用户 RSS 数据。通过 bwrap 挂载命名空间提供 WebKit 子进程库路径，未覆盖系统文件。

## 验证与验收

### 自动化验证

日志与截图位于本机忽略目录 `target/task1-platform-validation/`。前一批全部验收命令及 Web 专项结果见 [原始记录](2026-09-25-reader-metadata-source-unread.md)。产品代码未变，以下结果补充或替代先前的环境阻塞结论。

| 命令 | 退出码 | 真实结果 |
| --- | ---: | --- |
| `cargo fmt --all --check` | 0 | 本次重新运行通过，`fmt.log`。 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 加载原生依赖环境后通过，`native-clippy.log`。 |
| `cargo test --workspace` | 0 | 293 passed、0 failed、2 ignored；DST 子进程另输出 1 passed，不重复计数。`native-tests.log`。 |
| `cargo build -p rssr-app` | 0 | 原生可执行文件构建成功，`native-build.log`。 |
| `cargo.exe +1.97.0 check --locked -p rssr-app --target aarch64-linux-android` | 0 | Windows 工具链检查当前实现快照通过，`android-check-windows.log`；不是 APK 链接或设备运行通过。 |
| `bash scripts/run_wasm_contract_harness.sh wasm_subscription_contract_harness` | 0 | 4 passed，含新增 reader metadata / global unread / failed flag writes 契约，`wasm-subscription-retry.log`。 |
| `dx.exe build --locked --platform android --package rssr-app --target x86_64-linux-android --debug-symbols false` | 未保留外层退出码 | 构建失败：Windows `pastey` proc-macro 链接器 LNK1108，无法写临时文件；日志保留内部 rustc 退出码 1。不能编造 DX 外层退出码。`android-build.log`。 |
| `adb.exe -s emulator-5586 shell getprop sys.boot_completed`（中断后重试） | 1 | Windows 执行报 `Invalid argument`；最早受限环境尝试为 WSL socket failed。恢复工具权限后仍无法执行 Windows 程序。 |
| `git diff --check` | 0 | 本次文档无空白错误。 |

harness 复现环境：

```bash
env -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY \
  -u http_proxy -u https_proxy -u all_proxy \
  PATH="/home/deve/.local/share/rssr-test-tools/bin:/home/deve/.local/share/rssr-test-tools/wasm-bindgen-0.2.126/wasm-bindgen-0.2.126-x86_64-unknown-linux-musl:$PATH" \
  bash scripts/run_wasm_contract_harness.sh wasm_subscription_contract_harness
```

未重新运行：`cargo check -p rssr-app --target wasm32-unknown-unknown` 与对应 `clippy -- -D warnings`；两条已在实现提交前退出 0，本次没有产品代码变化。未运行 refresh/config exchange harness：未改对应能力。

### 手工 / 真实界面验收

- Linux/WSLg，原生 GTK/WebKit，1280×900：实际启动并查看截图，非 Web bundle 替代。通过 X11 输入操作当前测试进程。
- 列表归档关闭时无可见 fixture 文章，但来源显示未读 1；开启归档后显示文章，来源仍为 1，验证计数不依赖当前归档筛选。
- 点击文章进入阅读页：来源 `Desktop verification source`、作者 `Task 1 Author`、发布时间 `2026-03-08 03:00 UTC-04:00` 与独立“打开原文”按顺序显示，正文正常。原时间戳为 `2026-03-08T07:00:00Z`，符合纽约 DST。卡片与日期分组均为 `2026-03-08`。
- 点击“打开原文”后原阅读页保留；未观察到系统浏览器成功打开。当前 WSL 无法执行 `cmd.exe` / `powershell.exe`，系统默认浏览器入口未通过验收，不能用页面保留代替打开成功。
- 阅读页标已读 → 点击 R 返回列表：来源数为 0；列表标未读后数恢复 1，无手动刷新。对应截图 `native-return-read.png`、`native-unread-restored.png` 已查看。
- Web 360×800、1280×800 的布局与真实新标签打开：沿用实现提交的 Playwright 通过证据；本次新增 ChromeDriver harness 4 项通过，不将契约测试当作移动 UI 验收。
- Android：确认 Windows WHPX 可用，并实际启动独立模拟器；尝试中出现内存不足、低内存启动后 system_server 重启。最后一次 2 GB 无窗口启动未取得稳定运行验收；当前 Windows 互操作不可用，无法继续 ADB。APK 构建未完成，**没有安装运行当前应用**，没有 Android 360×800 或外部链接验收结果。

## 结果

- 已验证：原生 workspace 全量测试与严格 lint、Android ARM64 编译检查、浏览器 subscription 4 项契约、桌面元信息与计数 1→0→1。
- 推断：LNK1108 可能与当时 Windows 盘空间或内存压力有关；未确认根因。当前磁盘空闲已变化，不能把先前错误直接归因于磁盘已满。
- 未验证：Android APK/模拟器功能与实机、桌面系统默认浏览器成功打开、Windows/macOS 原生界面。不能宣称完整跨平台验收完成。
- 只提交文档，不修改产品公开签名或行为；本批交付不发布远端。

## 风险与后续事项

- Windows 互操作恢复后先检查独立 AVD 状态，重试 DX 构建；必要时只为测试进程设置独立 TEMP/TMP，保留原始 LNK1108 日志。不要删除用户数据或修改全局权限来绕过环境故障。
- APK 构建成功后仍需运行 `prepare_android_bundle.py`、Gradle、ADB 安装，再验证原文外部处理、历史时区与未读返回；ARM64 check 不能替代这些步骤。
- Android 使用 NDK 28.2；若发布流程固定其他 NDK，需要对应构建验证。
- 测试工具、AVD、依赖包和日志保留在上述隔离路径，未纳入 Git；未进行工作区清理。
- 本次恢复运行的桌面测试进程已关闭；当前 Windows 互操作不可用，未能再次通过 ADB 确认模拟器状态。

## 给下一位 Agent 的备注

- 产品实现入口、公开契约与全部改动文件清单见原始 handoff 和提交 `d490f35`。
- 本次 Windows 构建脚本、模拟器启动脚本及桌面启动辅助脚本位于 `target/task1-platform-validation/`；临时产物不是可发布构建。
- 用户允许任务范围内本地 commit；未授权 push、tag 或 release。

# UI 批次 2 审查修复、Android 主题导出与 safe-area 预演

- 日期：2026-08-02
- 原作者 / Agent：opencode (k3-256k)
- 审查与补全：Codex
- 分支：main
- 当前 HEAD：`a86885eae44056ed9d90c8fd80d21ef4af252f9d`
- 相关 commit：pending
- 相关 tag / release：`v0.1.14`（计划发布）
- 状态：`validated on Web and API 37; v0.1.14 release pending; exact API 35 run pending`

## 结论

原 handoff 的三个 UI 改动方向正确，但“validated / 可合并”证据不足：目录 mask 在滚动中点会提前离散切换为 `none`，360×800 只有手工记录，Shell 文件在 Windows 棡出中仍可能因 CRLF 失败，且 `title`/按钮数量的表述过强。本轮已修复这些问题，并补完 C7 Android SAF 导出和 C20 safe-area 全量样式适配。

实现与现有自动化均已通过；Android 设备矩阵使用现有 API 37 模拟器验证，未在本机缺失的 API 35 system image 上精确复跑。因此本文不恢复为无条件 `validated / 可合并`。

## 审查问题与处理

### P1：目录横滑提示过早消失

- 原因：scroll timeline 把 `mask-image` 从渐变过渡到 `none`；两者不可连续插值，浏览器会在中途离散切换。
- 修复：动画时间函数改为 `steps(1, jump-end)`。
- 自动验收：11 个月目录在 0%、50%、99% 都保持右缘渐隐，100% 为 `none`；单月目录始终为 `none`。

### P2：360×800 未自动化

- 新增 `mobile-ui-overflow`、`mobile-ui-short` 两套仓库 fixture；不读取开发机既有 localStorage。
- 新增 `scripts/browser/cdp_session.mjs` 复用 CDP 会话能力，`rssr_small_viewport_assertions.mjs` 执行 computed-style、几何、可访问性、console 和错误 overlay 断言。
- `run_static_web_small_viewport_smoke.sh` 默认固定 `360×800`、DPR 3、mobile/touch emulation，失败返回非零并保留 JSON、DOM、日志和截图。
- `run_release_ui_regression.sh --with-fixed-smokes` 会继承 debug/release profile 并自动调用该断言。

### P2：Windows Shell CRLF

- 新增 `.gitattributes`：`*.sh text eol=lf`。
- 现有 16 个受 Git 管理的 Shell 文件已机械规范化为 LF。
- Windows Git Bash 实跑 release smoke 通过；`git check-attr` 返回 `eol: lf`，字节扫描为 0 个含 CR 的 Shell 文件。

### P3：证据语义过强

- 来源 chip 同时保留完整 `title` 并增加 `aria-label`；文档只承诺桌面 hover 与可访问名称，不承诺所有移动 WebView 的长按 tooltip。
- 主题动词按 `data-action` 与语义验收，不再固定“13 个按钮”这一非契约数量。

## 实现摘要

实现依据：[Android SAF](https://developer.android.com/training/data-storage/shared/documents-files)、[Android edge-to-edge](https://developer.android.com/develop/ui/views/layout/edge-to-edge)、[WebView window insets](https://developer.android.com/develop/ui/views/layout/webapps/understand-window-insets)、[Dioxus 0.7.9 schema](https://raw.githubusercontent.com/DioxusLabs/dioxus/v0.7.9/packages/cli/schema.json) 与 [Google Play target API 要求](https://developer.android.com/google/play/requirements/target-sdk)。

### 1. UI 与 smoke

- 来源 chip 保持单行、内部文本省略且不撑高；完整名称由 `title`/`aria-label` 暴露。
- fixture 覆盖超长来源、超长 feed/entry/reader 标题、11 个月目录和单月目录。
- 自动断言根文档无横向溢出、标题边界、移动按钮碰撞、触控目标、键盘提示、主题“应用”语义及桌面 `1280×800` 回归。
- SPA fixture server 为移动 fixture 提供同源确定性 RSS，避免自动刷新引入 CORS/远端波动。

### 2. C7 Android 系统保存器

- `Dioxus.toml` 通过 Dioxus 0.7.9 的 `application.android_main_activity` 指向仓库自有 `android/MainActivity.kt`；生成目录未作为源码修改。
- `MainActivity` 继承 `WryActivity`，使用 `ActivityResultContracts.CreateDocument("text/css")`，默认名 `rss-reader-theme.css`，通过 SAF 以 `wt`（write + truncate）模式写 UTF-8；不申请存储权限。
- `wt` 是有意的 provider 兼容性约束：Android 官方说明单独的 `w` 是否截断由 provider 决定，`wt` 才明确请求覆盖写；即便 `ACTION_CREATE_DOCUMENT` 正常返回新文档，也不依赖 provider 的可选行为。
- Android-only Rust 适配使用 `jni`、`ndk-context` 调 Activity；JNI 回调状态为成功 0、取消 1、失败 2，保持 `save_css_file(&str) -> Result<bool>` 的 `Ok(true)` / `Ok(false)` / `Err` 语义。
- Rust 与 Kotlin 两侧都保护单一在途导出；重复请求明确失败。Activity 销毁会清理待处理内容；JNI callback 用 `catch_unwind` 阻止 panic 跨越 FFI。
- Android 文件导入仍保留原有 bail，本批只补导出。

### 3. C20 safe-area

- `target_sdk` 保持 34；自定义 Activity 只在 debug 且 API 35+ 调用 `enableEdgeToEdge()`。
- 基础 token 新增 `--safe-area-top/right/bottom/left`。
- body 吸收左右 inset；app shell 的宽度相对可用内容宽度并吸收上下 inset。
- 顶部导航、导航展开按钮、桌面目录 rail 和 Atlas sidebar 的 sticky top 加入顶部 inset；目录 rail 最大高度扣除上下 inset。
- reader bottom bar 保留底部 inset，并移除绕过左右安全区的 `100vw` 计算。
- `amethyst-glass` 的固定全屏伪元素只是不交互背景，允许延伸；legacy CSS 仅作旧主题识别，用户自定义 CSS 不纳入保证。

## 验证证据

### Rust / Web / 脚本

- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets`：通过。
- `cargo test --workspace`：通过，全部 suite 0 failed。
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过。
- `cargo check -p rssr-app --target aarch64-linux-android`：通过（NDK 28.2 toolchain）。
- `dx build --platform web --package rssr-app --release`：命令成功并生成 release bundle；本机 `wasm-opt` 子进程曾以 `0xc0000409` 退出，dx 保留未优化 bundle 后成功结束，随后 release 浏览器 smoke 通过。
- Node 三个 runner `node --check`：通过。
- 8 个新增 fixture JSON：解析通过。
- 21 个受 Git 管理的 Shell 文件：Windows Git Bash `bash -n` 全部通过，全部为 LF；其中 5 个 `.specify/scripts/bash/` 文件也已机械规范化，避免 `.gitattributes` 契约与工作树字节不一致。

Windows Git Bash release smoke：

```text
bash scripts/run_static_web_small_viewport_smoke.sh \
  --release --skip-build --port 8202 \
  --log-dir target/static-web-small-viewport-smoke/implementation-20260802-release-git-bash-run2
```

- 结果：30/30 assertions 通过。
- browser console errors：0；ignored console errors：0；error overlay：0。
- 产物：`target/static-web-small-viewport-smoke/implementation-20260802-release-git-bash-run2/`。

### Android 构建

- x86_64 debug Android 完整构建：通过。
- aarch64 release-native Android bundle：通过。
- 生成工程的真实 `:app:assembleRelease`（含 lintVital、R8）：JDK 22 下通过；JDK 25.0.4 会使 Android Lint 仅报版本号 `25.0.4` 后失败，确认是本机工具链兼容问题。
- release unsigned APK：`target/dx/rssr-app/release/android/app/app/build/outputs/apk/release/app-release-unsigned.apk`。
- `aapt`：minSdk 24、targetSdk 34、launch activity 为 `dev.dioxus.main.MainActivity`；manifest 只有 INTERNET 和 Android 自动生成的动态 receiver 权限，无存储权限。

### Android C7 设备验收

设备：API 37 / Android 17，Android System WebView `145.0.7632.218`。

- 取消系统保存器：页面返回“已取消导出 CSS 文件。”，对应 `Ok(false)`。
- 成功保存：`/sdcard/Download/rss-reader-theme.css` 为 9685 字节，与 `assets/themes/atlas-sidebar.css` 逐字节一致；两者 SHA-256 均为 `484DE38E360647885C256CD350DD9A9CBDA94EB79FCD134892EE4F3579B16842`。
- 重复导出：第一份保存器仍在前台时通过 WebView CDP 再触发一次，页面明确报告已有导出在途，原 picker 仍保持前台。
- 不可写 provider：安装仅用于 QA 的临时 `BlockedDocumentsProvider`，其 `openDocument` 有意失败；页面报告“导出 CSS 文件失败”，应用无崩溃。
- 证据：`target/android-emulator-qa/20260802/` 下的 picker XML、导出文件、日志与截图。

`wt` 修正后的发布前复验仍使用 API 37 / WebView `145.0.7632.218`：

- 取消：页面返回“已取消导出 CSS 文件。”。
- 不可写 provider：页面返回 `Intentional unwritable provider failure`，应用无崩溃。
- 成功：系统为同名文件生成 `rss-reader-theme (1).css`，9685 字节，与 `assets/themes/atlas-sidebar.css` 的 SHA-256 同为 `484DE38E360647885C256CD350DD9A9CBDA94EB79FCD134892EE4F3579B16842`。
- 重复导出：第二次请求未打开新 picker，原 DocumentsUI Activity 保持 top-resumed。
- 新证据：`target/android-emulator-qa/20260802-release/`。

### v0.1.14 发布前 Android 产物

- 干净 arm64 生成目录运行 `dx bundle --platform android --package rssr-app --target aarch64-linux-android --release --debug-symbols false`：通过。
- JDK 22 下以 `v0.1.14` 运行真实 `assembleRelease bundleRelease`：通过，包含 R8 与 lintVital。
- unsigned APK：16,334,725 字节；AAB：16,658,699 字节；两者只含 `arm64-v8a/libmain.so`。
- APK 内为 `versionCode=114`、`versionName=0.1.14`、targetSdk 34、launch activity `dev.dioxus.main.MainActivity`；无读写或管理外部存储权限。
- 首次本地复跑曾因生成目录残留 x86_64 库得到 31.5 MB 多 ABI 包；发布 workflow 本就会清理该目录。移动旧生成目录后干净复跑确认并非代码包体回归。
- 干净 Gradle 首轮曾因旧 Gradle daemon PID 44936 持有 R8 `classes.dex` 失败；Windows Restart Manager 精确确认锁持有者，`gradlew --stop` 后相同任务串行复跑通过，属于本机生成目录生命周期问题。

### Android C20 设备验收

- API 37 debug 路径确实启用 edge-to-edge；WebView 145 支持 `env(safe-area-inset-*)`。
- 手势导航竖屏：系统 status/navigation inset 为 63/63 px，CSS token 为 24/24 px（density 2.625）。
- 三键导航竖屏：navigation inset 为 126 px，CSS bottom token 为 48 px。
- 三键导航横屏：status bar 63 px、右侧 navigation bar 126 px，CSS token 为 top 24 px / right 48 px；页面右缘小于 `innerWidth - safeRight`。
- 手势导航横屏：CSS top/bottom token 为 24/24 px，页面无横向溢出。
- 浅色、深色均截图检查；IME frame 为 `[0,1048][1080,1920]`，聚焦的自定义 CSS 编辑区被 resize 保持在 IME 上方。
- 浏览器 smoke 另覆盖 reader bottom bar、目录、长标题、移动按钮与桌面 rail 的真实页面结构。
- 未发现应用级 `FATAL EXCEPTION`、JNI `UnsatisfiedLinkError` 或 panic。

## 剩余边界与缺口

- 本机没有 API 35 system image，且 C 盘空间不足以安装新的多 GB image；本轮用更高的 API 37 执行 API 35+ debug edge-to-edge 分支。合并前若要求逐字满足矩阵，应在 API 35 模拟器复跑同一套姿态/导航/IME 检查。
- Windows Git Bash 已在当前未提交工作树真实通过，但没有另建 fresh checkout；未来 checkout 的 LF 行为由 `.gitattributes`、`git check-attr eol=lf` 和全脚本 0 CR 字节扫描共同约束。
- Android 主题文件导入仍未实现。
- targetSdk 升级不在本批；正式升级时按当时 Play 要求评估直接升 API 36，而不是机械停在 35。
- C1 返回按钮、C15 激进撤除快捷入口仍不在本批。

## 工作区与交接

- HEAD 仍为 `a86885eae44056ed9d90c8fd80d21ef4af252f9d`；所有实现均未提交。
- 未 push、未打 tag、未创建 release。
- 工作区原有其他未提交修改均保留；不要用 reset/checkout 清理。
- 只有补完 API 35 精确设备复跑（或由维护者明确接受 API 37 替代证据）后，才把状态改回无条件 `validated / 可合并`。

# 目录窗口化的筛选、分页与返回边界

- 日期：2026-10-27
- 作者 / Agent：Codex
- 分支：feat/reading-workspace-20261024
- 当前 HEAD：915b9888（修复提交，文档补记前）
- 相关 commit：915b9888
- 相关 tag / release：无
- 状态：validated（交互验收完成；手机逐表数据核对受阻）

## 工作摘要

继续复查窗口化目录在筛选切换、跨页定位和正文返回时的边界。当前阅读分支独立推进，不迁入 v0.2。

## 影响范围

- rssr-app 的目录 DOM 生命周期和待执行定位请求；Web / Windows / Android 共用。
- 不改存储格式、业务用例、查询分组和多选范围。

## 关键变更

- 窗口适配器在发送、测量和执行回复前检查 DOM 连通性、目录 epoch 与实例所有权；退役窗口清理监听、测量和等待，迟到回复不能操作替换后的目录。
- 定位请求从用户点击时即绑定原页、目标页、上下文和请求序号。等 Rust 接受且目标页提交后执行；新页面或上下文取代请求后，迟到回复不能重建它。
- 优先按现有架构定位根因：问题发生在 DOM 替换、提交等待和异步回复的生命周期，不在 Rust 的分组、目标页映射或窗口几何规则。没有新增业务状态、crate 或修改持久化语义；设计契约补充退役窗口和失效定位请求的执行约束。

## 验证与验收

- P2 基线复现：延迟 End 回复后搜索 Window 5，111 篇新目录 scrollTop 0 → 5079；换页/分组则 0 → 32478。
- P2 基线复现：目录请求定位第 2 页，回复到达前用户转到第 3 页；旧请求一直等待，之后返回第 2 页会再次设置旧锚点并滚动。
- 新回归脚本 `scripts/browser/rssr_directory_window_boundaries.cjs`：11 个场景通过。覆盖旧焦点回复遇到搜索、来源筛选、换页、分组、空结果再恢复、正文返回；跨页请求被翻页或查询取代；先返回目标页再收到旧回复；来源/时间模式下来源筛选后末页正文返回。
- 返回恢复取实际 click 捕获边界的文章卡片位置，并在返回后比较同一个卡片：来源模式 scrollY 1792、卡片 top 425.6875；时间模式 scrollY 1754、卡片 top 425.71875；前后均相同。筛选仍为只选 Bravo，页码均为 6。最初测试错误比较标题与卡片的 top，已修正测量对象，不是产品缺陷；该素材没有长正文，长正文范围见原生验收。
- `NODE_PATH=C:/Users/QQ/node_modules` 下，其余最终源码的隔离无头 Chrome 回归：
  - `node scripts/browser/rssr_directory_offline_regression.cjs`：150 项通过。
  - `node scripts/browser/rssr_directory_window_acceptance.cjs`：22 项通过。
  - `node scripts/browser/rssr_directory_window_review.cjs`：8 场景通过。
  - `node scripts/browser/rssr_directory_transport_acceptance.cjs --offline-fixture true --artifact-dir target/directory-window-boundaries-20261027/transport`：5 种坏消息及其后正常请求通过。
- 以上测试通过 Playwright 路由提供本轮 Web 构建和隔离素材；没有启动 HTTP 服务。证据目录 `target/directory-window-boundaries-20261027/`，包含 before/acceptance.json（旧测试版）、after/acceptance.json（最终 11 场景）、contracts/、window/、review/、transport/。
- `cargo test -p rssr-app --locked`：159 单元 + 4 集成通过，1 项显式性能测试忽略。`cargo clippy -p rssr-app --all-targets --locked -- -D warnings`、`cargo fmt --all --check`、本轮文件的 `git diff --check` 和新脚本 `node --check` 通过。
- `dx build --platform web --package rssr-app --locked --release`：产出本轮可运行 Web 包。已有 dx 0.7.10 / Dioxus 0.7.9 不匹配提示及可选 wasm-opt 退出 0xc0000409 仍存在；dx 继续完成。不将优化器记为成功，未改工具链或依赖。

### 最新原生包

- Windows：`cargo build -p rssr-app --release --locked --target-dir target/reading-baseline-20261025/build-b`，独立目录复制 5000 篇测试库，用新 EXE 的 CDP 验收 38 项通过，包括跨页、多选保持、显隐恢复、连续三次长文返回。证据 `windows-acceptance/acceptance.json`；通过 `Start-Process -WindowStyle Hidden` 启动，仅操作测试 WebView。
- Windows EXE SHA256：`0B148E2B1986CFE6BD8D0F36D90C943069B4195B6B3D129679697333413EB303`，位于 `windows/rssr-app.exe`。
- Android：`dx build --locked --platform android --package rssr-app --target aarch64-linux-android --release --debug-symbols false` 输出客户端构建成功；dx 命令的等待会话未自行结束，确认无 dx/cargo 进程且已完成独立打包和真机验收后终止该等待，不把会话退出码记为构建成功。随后 `python scripts/prepare_android_bundle.py target/dx/rssr-app/release/android/app/app/src/main`、JDK 21 的 Gradle `assembleDebug --no-daemon` 成功。
- Xiaomi 12 / Android 15 / WebView 152：40 项通过，包括目录层级、窗口化键盘首末篇、跨页、多选、Android Back 收起恢复、连续三次长文及图片返回。证据 `android-acceptance/acceptance.json`。
- APK SHA256：`97F9A1360A6B24F9758725C902B8E549D422475AF8635A7C80BC52D9F77A25A4`，覆盖安装后读回一致，`android-install/receipt.json`。生成的 libmain.so SHA256：`093CCC34F4B10A170C9649D50A5C44E5250D44439C06D094316972FF96218ED5`；本轮未完成 APK 内库的独立解压比对。Rust release + 可调试 wrapper，并非发布签名包。
- 迟到回复的精确故障注入在 Chrome 完成；上述原生通过记录是正常交互回归，不能混写为原生故障注入通过。未测真实系统 IME、Linux/macOS 或性能。

### 手机数据与清理

- 覆盖安装前与测试后停包备份 files/cache。前备份 `android-before.tar` SHA256 `ACB7EAA39DEC15DF7D8EC3929BE5B487A653244420F093CCE17717E10F9CAE96`；后备份 `android-after.tar` SHA256 `7DEADFAE5F3A33A011F732B00ACCBA0A6AECC562D4253DEFAA2C6C38D2114895`。没有卸载、清数据或回写旧数据库。
- 手机界面验收确认 5000 篇素材存在；脚本恢复 source 分组及搜索 05000，搜索栏收起。前后文件已提取到本轮 `phone-before/`、`phone-after/`。
- `verify_artifacts.py` 原计划在电脑只读打开这两份 SQLite 副本、比较表与设置、汇总哈希；自动审批拒绝执行，只返回 `approval required by policy, but AskForApproval is set to Never`，未提供具体原因。因此不能声称本轮已完成逐表数据、SQLite 完整性或 APK 内库核对。脚本及前后备份保留，未换执行渠道重试。
- 本轮 Chrome、独立 Windows 测试进程与 Android 9262 转发已关闭；手机已停包。其他任务的 9243 / 9252 转发保持原样。

## 结果、风险与后续事项

- 修复提交：915b9888。随后文档提交仅补记源码身份，不改变上述已经验收的生产代码或安装包。
- 只使用隔离无头 Chrome 和本任务原生调试连接，不操作用户桌面鼠标键盘。
- 保留任务外删除、旧交接修改与 Penpot 文档，不改 C 盘基线工作区。
- 另一任务在本轮期间新增了文档提交 8dad44c7，已核对只涉及 `2026-10-27-native-preparation-next-step.md`，没有生产代码变更；完整保留。
- 原有行为验收通过：同页定位、目录浏览不清空选择；跨页/筛选/分组清空选择；阅读返回恢复位置与筛选。当前未发现额外的返回恢复产品缺陷，不从本轮功能回归推断性能改善。

## 给下一位 Agent 的备注

- 入口：directory/adapter.js、window_adapter.js、新边界回归脚本。
- 准备返回验收时，应在实际打开文章的 click 捕获边界记录位置，避免把尚未完成的滚动误当作恢复失败。

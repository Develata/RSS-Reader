# Reader 并发问题复现、修复与三端验收

- 日期：2026-10-30
- 作者 / Agent：Codex
- 分支：`codex/reader-race-fixes`
- 开工 HEAD：`0c5e31a6cca607c5ba6f8783b2fc3923c0eb1160`（当时 main）
- 相关 commit：本记录所在提交；提交前为 `pending`
- 相关 tag / release：无；不发布
- 状态：`validated`（本地候选及所列实际宿主；合并状态另见 PR）

## 工作摘要

按“测试并修复发现的 bug”要求，在已整合的 Reader 上补做异步时序测试。原 main 的全 workspace 测试通过，但真实服务器 Web 复现两类 P2 问题。本次单列行为修复，没有继续正文模块提取，也不将修复混写为此前等价重构。

## 影响范围

- 模块：`pages/reader_page/` 的状态、reducer、session、facade、图片 hook 与 WebView 消息；Reader 共用宿主场景和正式 Web 回归运行器。
- 平台：共享代码影响 Web、桌面及 Android；实际运行正式 Web、Windows WebView2、Android 15 真机。
- 保持七 crate、`Backend` / `LocalBackend`、rusqlite 单库、现有正文和渲染器。没有数据库、公共 API、CSS、依赖或 workflow 改动。
- 复用既有 E 盘工作树。主 checkout 的分支、HEAD、状态、index 和 41 个文件/缺失路径逐项核对未变。

## 复现与关键变更

### P2：切文后旧图片消息打开旧图片

同一 DOM 事件轮次依次激活 A 的正文图片和“下一未读”按钮，应用执行真实导航与异步消息。新文章 B 已加载后，旧消息仍打开 A 的图片查看器。修复前报告中 `activation-then-navigation` 失败，实际浮层数量为 1。

图片消息现在携带现有 `load_generation` 的文本值；普通 Rust 校验代次，hook 同时检查当前正文就绪。复用既有 Reader 状态，既不另建访问计数器，也不复制浮层状态。文本传输避免将 u64 交给 JavaScript Number；无效值、A→B→A 和最大 u64 有普通 Rust 回归。

共用宿主脚本在 Web、Windows、Android 执行新增时序。另有 12 轮切文和 16 轮快速关闭/重开补充检查。这里是受控事件顺序，未声称物理手势复现；快速重开本身在基线已通过，没有针对它修改产品行为。

### P2：同种标记写入旧响应覆盖新状态

对真实服务器第一笔 PUT 仅扣留已完成写入的响应；继续执行同值第二笔，再执行相反值第三笔，最后释放第一笔。已读和收藏都出现请求数 3、UI 为 true、数据库为 false。没有替换 Backend、页面状态或写入实现。

现有 `ReaderPageState` 分别持有已读/收藏 pending。session 在 spawn 前同步设置，按钮与快捷键共用门禁；同种在途输入忽略，另一种仍可操作。成功和失败后释放门禁，按钮显示 disabled / `aria-busy`。写入继续由 App 持有；旧访问仍走原 Shell 提示与重读，不能修改或解锁新访问。没有跨页排队或跨客户端全局排序。

新增普通 reducer 回归检查门禁独立、正文 Arc 保留和新访问重置；Dioxus 集成回归检查下一次 render 前重复输入、失败后重试及旧访问完成不能解锁新访问。Web 慢响应回归直接比较 UI 与真实存储，并检查另一种标记可操作、下一次明确操作可用及正文 DOM 保留。

## 验证与验收

原始日志、截图、失败复现、脚本及 APK 在工作树 `target/reader-bugcheck-20261030/`；可提交摘要见[验收回执](../testing/evidence/2026-10-30-reader-race-fixes/receipt.json)。失败目录保留，不以成功结果覆盖。

### Rust 与构建

| 命令 / 身份 | 实际结果 |
| --- | --- |
| 修复前 main：`cargo test --workspace --locked` | 1129 通过、4 忽略、0 失败；这是基线，不作为修复后全 workspace 结果 |
| 候选：`cargo test -p rssr-app --locked` | 428 通过、1 忽略；396 单元、27 独立规则、2 主题、3 样式 |
| `python scripts/check_reader_rule_boundary.py --target-dir target/reader-bugcheck-20261030/rules-build` | 同一生产规则 27/27，通过且依赖无 Dioxus / WebView |
| `cargo fmt --all --check`、`git diff --check` | 通过 |
| `cargo clippy -p rssr-app --all-targets --locked -- -D warnings` | 通过 |
| `cargo check -p rssr-app --target wasm32-unknown-unknown --locked` | 通过 |
| `cargo build -p rssr-app -p rssr-web --locked` | 通过；候选 EXE / 服务器用于下方实际验收 |
| dx 0.7.9：`build --platform web --package rssr-app --locked --release --debug-symbols false` | 正式 Web 构建通过 |
| 独立 Android 快照：dx 0.7.9 `build --platform android --package rssr-app --target aarch64-linux-android --locked` | 构建通过；已有 `LOCAL_DATA_DIR_NAME` 未使用警告保留。编译结果与真机结果分别记录 |

完整 app 测试包含同一独立检查，不能将 27 项再次累加到 428。

### 实际宿主

| 环境 / 入口 | 实际结果与身份 |
| --- | --- |
| 正式 Web：`node scripts/browser/rssr_reader_host_acceptance.cjs` | 1280 / 390，6/6；真实服务器和隔离 SQLite，Chrome 155.0.8059.40，build `dev-0c5e31a6cca6-dirty-6de697eefeee` |
| Windows：`run-windows.ps1 -RunName windows-candidate-01` | 实际 EXE / WebView2 156.0.4314.50，1280 / 390，8/8；原 main 的正常宿主基线另为 8/8 |
| Android：`run-android.ps1 -Variant candidate -RunName android-race-candidate-01` | 真机 `caa0c283`，Android 15 / WebView 156.0.8078.47，392 宽，4/4；独立测试包，包含实际 `adb keyevent 4` 系统返回 |
| 补充图片探测：`probe.cjs`，`PROBE_IMAGES_ONLY=true` | 2/2：切文 12 轮、快速关闭/重开 16 轮 |

本地启动脚本位于本批证据目录，调用已入库的 `rssr_back_navigation_acceptance.cjs --reader-only true --reader-host true`，不依赖人工点击。各报告 pageErrors / consoleErrors 为空。Web 新增慢响应场景随既有 reading-workspace CI 运行器执行；Windows / Android 未做 HTTP 延迟注入，不能据正常标记检查称其也覆盖了该注入。

Web 产物由 `RSSR_WEB_BIN`、`RSSR_FIXTURE_BIN`、`PUBLIC_DIR` 指向本次构建，`ARTIFACT_DIR` 使用全新目录；本机 Playwright 从 Codex 已有依赖目录加载。Windows / Android 使用 100 篇文章、6 个 loopback 来源的合成资料库，fixture 服务位于 9299。截图只作所列场景检查，没有全平台逐像素一致性结论。

### Android 产物与资料保全

- 仅安装 `io.github.develata.rssreader.backpolicytest`，build `reader-race-0c5e31a6-ba02c31a44b5`。安装后读回 APK 哈希匹配。
- 产品输入 464 个，清单 SHA-256 `ba02c31a44b5a69460d27b45d9cc47079c9a568c442b6c79a13308b1087433c4`。独立源码仅调整测试包 identifier / BuildConfig 引用，独立 target；生成 Activity 和依赖路径核对匹配。
- APK SHA-256 `a5f6b4966be1abf66eda9b0d22b4743874c06846bbece3a63b975d5c7d19955f`；APK、JNI 目录及 Cargo 输出的 native library SHA-256 均为 `efa9f4551d18400f100f214d52de55c9efc7cba1549a5ef2131512510c743f15`。
- 安装前备份旧测试 APK 及原资料库目录；归档逐文件哈希与设备相同。结束后恢复原目录，6 个文件的路径集合及哈希全相同；日常包 APK 未变，原有 forward / reverse 映射未变。测试候选包和合成运行目录保留，不清理历史证据。

### 测试基础设施中的失败

首次 Web 探测因 Windows 绝对路径 ESM import 失败，改用 file URL 后再跑；不计产品缺陷。首次 Windows 包装脚本因空 diff 未生成补丁文件而中止，并遗留自有测试进程；核对 EXE 路径后停止，第二次端口占用被拒绝，修正包装脚本后第三次正常通过。Android 首次运行目录名撞到旧设备备份名，保护检查在移动资料前拒绝；确认原文件未变后改用本批唯一名称执行成功。没有删除旧备份或覆盖旧证据。

## 结果

两类已复现问题已修复，代码自审及所列测试通过；不是第三方独立 review。提交范围仅本批代码、回归和文档。远端 CI 需在推送后的精确 head 核对，最终远端回执保存于本批 `target/` 并在 PR 描述记录；本地通过不能代替 CI。尚未合入 main，没有新 tag 或 release。

## 风险与后续事项

- 同种标记在等待中忽略输入，完成后再明确切换；这是为修复已复现乱序新增的行为门禁。
- 未覆盖物理键盘、系统 IME、辅助技术、macOS/Linux GUI、正式签名 Android 升级及跨设备写入排序。不从编译或合成事件推定这些验收通过。
- 修复收口后，下一条模块化边界仍为 Reader 命令装配对页面 `support.rs` 正文选择的反向引用，见[正文边界](../design/reader-rendering-boundary.md#下一批建议)。本批未实施它。

## 给下一位 Agent 的备注

先核对当前分支和 main。复现入口为本批 `probe-baseline-source.cjs` 与 `web-probe-baseline-02/`；候选规则入口为 `image_viewer_state.rs`、`state.rs`、`session.rs`。`collect-receipt.py` 会核对产品输入、实际报告、Android 原资料及主 checkout 保全后生成已提交摘要。不要拿历史构建目录或基线全 workspace 成功替代当前候选验证。

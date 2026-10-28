# Feeds 快照查询与刷新提示规则提取

- 日期：2026-10-28
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 当前 HEAD（实施基线）：`8ea613cdd92c8bb4dba22d71e6253e5baea8fa2b`
- 相关 commit：与本记录同批本地提交；记录生成时 `commit: pending`。提交后用 `git log -1 -- docs/handoffs/2026-10-28-feeds-request-lifecycle.md` 定位。
- 相关 tag / release：N/A；未 push、打 tag 或发布。
- 状态：`validated`

后续复查：`2b551537` 的 Rust 规则未发现行为回归；验收脚本在启动失败时遗漏失败报告与资源回收的问题已在 [Feeds review 交接](./2026-10-28-feeds-lifecycle-review.md) 中复现并修复。后续运行以修正后的脚本为准。

## 工作摘要

继续原生界面前置拆分，将 `FeedsPageSession` 的命令归属、快照查询身份、单来源刷新提示时长和清除条件移入页面内纯 Rust 模块。保留当前行为、Dioxus 页面任务与 Shell 全局刷新边界，不引入新状态源或原生渲染器。

## 影响范围

- 模块：`feeds_page/request_policy.rs`、`session.rs`、`reducer.rs`、`mod.rs`。
- 平台：共用页面影响 Web、Windows、Android、Linux、macOS；实际运行验收覆盖 Web 与隔离 Windows WebView2，Android 为编译覆盖。
- 文档与测试：原生前置边界、页面职责、前端命令参考、新增 Feeds Playwright / CDP 验收脚本。
- 无 application/domain/host/infra 行为、依赖、crate、schema、迁移或 workflow 变更。

## 关键变更

### 纯规则与适配边界

- `FeedsRequestPlan` 区分 Shell 手动刷新、快照查询、单来源刷新及其他命令。只有 `LoadSnapshot` 分配下一查询代次，沿用 `wrapping_add`；generation signal 与 hook 顺序不变。
- session 同步发布代次后构造命令 Future，再启动原页面作用域任务；查询返回时先判身份，过期结果整批丢弃，包括全局状态与 reload。接纳结果继续保序投影、移动，不复制列表。后来查询不会丢弃早先写入的完成结果。
- “刷新全部”继续直接进入 Shell 的同步去重入口和 App 作用域，单来源刷新仍属于页面。
- `RefreshFeedback` 在单来源刷新的整批结果发布后捕获当前 revision、消息和 tone；精确 `error` 为 6 秒，其余为 3 秒。真实等待留在适配器。
- reducer 使用纯函数同时比较 revision 与消息，再仅清空提示正文。删除确认可能只改消息而不递增 revision，因此双条件必须保留；不改 tone、revision 和其他页面状态。
- 新增、编辑、删除、导入导出的 reducer 状态转移和后续重取 effect 不变。页面销毁继续取消自己的未完成任务及等待，不代表回滚已提交写入。

### 测试与验收方式

- 新增 6 个纯规则测试、7 个 Dioxus 会话测试。覆盖命令分类、代次环绕、写入不受查询失效、同步构造顺序、过期整批结果、Shell 分流、整批完成后捕获提示、相同消息但新 revision、相同 revision 但新消息、普通命令不启动计时、页面释放取消等待。
- 保留既有快照乱序成功/失败、添加去重/失败重试和编辑弹窗测试；Feeds 相关测试共 31 个通过。
- 浏览器脚本以真实 Rust/WASM 或 Windows 程序执行页面操作；成功刷新使用回环 304，失败使用回环非法 RSS，不访问外部订阅。文章标记按全体记录比较，不仅检查少数样本。
- Windows 使用复制 EXE、独立 WebView profile 与复制数据库。仅将复制库中的来源 URL 指向脚本自己的回环服务；脚本导航或编辑前先只读核对 5000 个连续文章 ID/固定标题、当前 Entries 页已挂载文章，以及全部有效订阅 URL 的回环来源。

## 验证与验收

证据目录：`target/feeds-lifecycle-20261028/`（忽略产物，未提交）。正式验收脚本与本记录入库。

### 自动化验证

- `cargo fmt --all --check`：通过，`fmt.log`。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：通过，`clippy.log`。
- `cargo test -p rssr-app --locked feeds_page`：31 passed，`feeds-tests.log`。
- `cargo test --workspace --locked`：526 passed / 0 failed / 2 ignored，`workspace-tests.log`；忽略项为既有 Entries 投影与 refresh adapter 手动性能探针。
- `cargo check -p rssr-app --target wasm32-unknown-unknown --locked`：通过，`wasm-check.log`。
- `cargo check -p rssr-app --target aarch64-linux-android --locked`：通过，`android-check.log`。使用本机 NDK 28.2.13676358 的 API 24 clang / llvm-ar；不代表 Android 设备验收。
- `node --check scripts/browser/rssr_feeds_lifecycle_acceptance.cjs`、本批范围 `git diff --check`：通过。

### 构建、产物身份与同路径验收

- 基线复用 Settings 批次的已验证构建，实际生产源码与本批基线 HEAD 一致；`dda9aefa..8ea613cd` 仅改文档和 Settings 验收脚本。已重新核对源码和实际 index → JS → WASM 引用链，见 `baseline-identity.json`。
- Web 构建：`target/release-ui-tools/dioxus-0.7.9/dx.exe build --platform web --package rssr-app --locked --release --debug-symbols false`。匹配 Dioxus 0.7.9 工具，构建成功，无优化失败，日志 `web-build.log`。
- Windows 构建：`cargo build -p rssr-app --release --locked --target-dir target/reading-baseline-20261025/build-b`，通过，`native-build.log`。
- Web：设置 `NODE_PATH=C:/Users/QQ/node_modules`、`PUBLIC_DIR=<该版本 public>` 后运行 `node scripts/browser/rssr_feeds_lifecycle_acceptance.cjs --artifact-dir <证据目录>`。基线 `baseline-web/acceptance.json` 与候选 `candidate-web/acceptance.json` 均 2/2 通过。
- Windows：本地 `run-native.ps1` 校验 EXE、空闲端口并准备独立副本；调用同脚本 `--cdp http://127.0.0.1:9276 --database <复制库> --feed-origin http://127.0.0.1:9277 --artifact-dir <证据目录>`。基线 `native-baseline/write/acceptance.json` 与候选 `native-candidate/write/acceptance.json` 均 2/2 通过；结束时只关闭本次进程，两个端口已释放。
- 每个平台使用相同的 1536×844 / 390×844 视口、相同路径与步骤。覆盖初始快照、单来源成功/失败提示消退、错误提示超过普通提示时长仍保留、旧计时器不清除删除确认、全局刷新 busy 释放、添加/删除后统计重取。Web 全部 72 篇、Windows 全部 5000 篇文章标记及原有有效订阅 ID/URL 保持不变；pageErrors / consoleErrors 均为空。
- Web 两张截图与基线 SHA-256 完全一致，已检查实际图片：1536 宽 `752FACB58EDD72C6C273375A2AC5FA5BAB6DF237675220B7BD42B2888C2969F7`；390 宽 `AFAA3557AFDE179E1A79A9767D6897A0CB8FC32D2689C31EFDEE78CFF3FF5CF6`，见 `visual-equivalence.json`。
- 基线 WASM SHA-256 `5bf3880a1f2a24aeb07ad2b1aa74ffbbb5df9e27483a282dcfa263f5cb336171`；候选 `9905a36017e8fd0c64c762368a4cb3330d3096d7aa3bbd3a3cc28b32abf16b50`。候选引用链见 `candidate/web-identity.json`。
- 基线 EXE SHA-256 `957F53A0C4774C0A0161567B0440716328EABF6E4328CCF7397EADFBD2AFE83C`；候选 `8AD0653A863ED11FF9625D23962D6CDE255EB84E045C85B6FF4B88A72ED92A54`，运行副本见各原生目录 `identity.json`。
- 本批最终源码、测试总数与脚本 SHA-256 见 `validation-identity.json`；基线和候选运行同一验收脚本。

### 手工与未执行覆盖

- Web 使用真实无头 Chrome 并检查宽/窄视口图片；不声称人工逐项点击。Windows 使用隐藏隔离进程的 DOM / CDP 与只读 SQLite，未以隐藏窗口截图声明原生视觉通过。
- 快照乱序、相同消息的新 revision 和离页取消使用 Dioxus 可控 Future 测试，未在实机上注入这些精确调度。
- 未运行 Android 设备交互、Linux/macOS 原生 UI、Windows 前台输入法、托管 `rssr-web` 服务端模式。
- 未扩测编辑保存、配置/OPML 导入、真实外网抓取或认证；本批未改这些用例。

## 结果

- 规则提取、测试、基线/候选同路径运行验收、契约文档与交接已完成，按本批范围本地提交；提交身份见开头定位命令。
- 未发现本批行为回归。仍是既有界面的模块边界整理，不宣称性能收益或原生渲染替换完成。

## 风险与后续事项

- 快照查询与编辑/写入使用不同归属规则，不应为接口统一而把 query generation 套到全部命令上。
- 提示消息正文是清除身份的一部分，不能只按 revision 清除或在结果批次完成前捕获。
- 后续原生适配仍需实际宿主的计时、页面任务取消、焦点与输入验证；本次纯规则测试不能代替这些验证。

## 给下一位 Agent 的备注

- 从 `feeds_page/request_policy.rs` 和 `session.rs` 测试进入；边界记录在 `docs/design/native-ui-preparation.md` 的 Feeds 小节。
- 运行原生脚本必须先准备独立副本、profile、复制库与回环 URL；脚本的内容核对不能替代调用方的进程隔离。不把用户库作为 fixture。
- 初始工作区包含任务外 `.agents/skills/speckit-*`、`.specify/*` 删除、两份 10/03 handoff 改动和其他未跟踪文件；与 `initial-status.txt` 对照保持不变，仅按精确路径提交本批。
- 授权仅覆盖本地提交；无 push、远端合并、tag 或发布。

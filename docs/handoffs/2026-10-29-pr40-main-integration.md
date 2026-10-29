# PR #40 与 main 整合：保留逐字段保存和主线架构

- 日期：2026-10-29
- 作者 / Agent：Codex
- 分支：本地 `codex/pr40-main-integration`；推送目标 `codex/native-ui-preparation-20261029`
- 当前 HEAD：整合起点 `58157a1a90796f8a17624bf9958b0be75bb15bf4`
- 相关 commit：包含本记录的普通合并提交（父提交为 PR 起点与 main；提交前状态 pending），见 PR #40 分支历史
- 相关 tag / release：无
- 状态：`validated`（本地自动化范围；远端检查记录在 PR #40 的当前 head/run 回执中）

## 工作摘要

将 PR #40 的私有 Rust 规则模块和原生控件前置修复接入当前 main，不恢复旧分支的宿主层、BrowserStore 或整份 Settings 草稿保存。最初合入 main `6a201d5a`；处理中 main 又合入 PR #41 到 `9b17a4d5`，同时保留其订阅/设置独立内容面板、样式、选择器和设计记录。使用普通合并，不改写历史。

## 影响范围

- 模块：Entries/Reader/Feeds/Settings 页面、HTML dialog、原生 WebView 文档初始化、主题文件适配、浏览器验收和 CI。
- 平台：Windows 原生、正式 Web；Android 编译。其他原生平台未做本轮 GUI 验收。
- 架构：保持 main 的七 crate、`Arc<dyn Backend>`、LocalBackend、rusqlite 单库 `library.sqlite3`、正式 Web 同源 HTTP Backend；application/domain/infra 实现不作回退。
- 工作树：`C:/Users/QQ/.codex/worktrees/pr40-main-integration/RSS-Reader`。原 `E:/gitclone/RSS-Reader` 的 dirty/staged/untracked 内容未改动。

## 关键变更

### 主线语义与模块化

- Entries 写入规则输出有序发布和 `Reread`；后者仍调用主线完整恢复读取。偏好保存沿用已加载准入、串行队列和恢复编辑规则。
- Reader 仅抽取结果身份检查；保留主线只读刷新代次和 App 所有的标记写入。
- Feeds 抽取编辑状态与请求/提示规则；保留 App 作业、配置指纹、偏好/URL 写入顺序、离页回执。结果未知和错误的刷新提示均保留 6 秒。
- Settings 抽出主线的 `Edits`/`Writes` 与保存准备/回执折叠。唯一投影仍为 stored → submitted → pending；仅发送实际改变字段，CSS 校验和页大小归一化仅处理补丁中的字段。保存失败恢复 submitted 到后续编辑下方。
- 保留最新读取、成功写入版本、写入期间暂存读取、跨刷新/登录草稿恢复、下载确认和 App 作业规则。

### 适配边界

- 数字提示使用独立字段投影代次：等值回填也作废旧提示，A→B→A 不复活；仍有 pending/submitted 的字段保留提示。解析失败不构造 SettingsPatch。
- 主题导入在选择器打开时取得身份；后续编辑、保存、下载准入或实际 CSS 投影变化使旧读取失效。验证、提示、草稿变化和保存之前检查并消费身份。
- 文件严格按 UTF-8 解码；Web 保持去掉开头 BOM 的既有行为，原生保留原文。导出保留点击时内容及 CRLF；DOM textarea 的换行归一化不能用作导出字节基准。
- 原生仅修正锁定 Dioxus 解释器的字符串表解码器以保留 BOM，不改全局 TextDecoder。保留主线关闭时的数据库 checkpoint。
- HTML dialog 的原生 cancel 也交给 Rust 关闭 gate；保存中不因焦点移出而绕过 gate。

### 验收迁移与文档

- 新增 `rssr_native_preparation_acceptance.cjs`，使用正式 Web、真实 rssr-web/SQLite 和回环 WebDAV；在 1536/390 两种宽度验证逐字段保存、保存期间编辑、等值回填、下载与字段提示、主题文件字节和过期读取。加入主线 reading-workspace CI。
- 保留 main 的 workspace/entries/server/Pages 验收。退役 10 个依赖旧 BrowserStore 或整份草稿语义的 PR 专用脚本，不将历史结果充当新版本验证。
- 更新原生适配契约、验收入口、前端命令和用户指南。旧分支交接保留为历史，详见 `docs/testing/native-control-acceptance.md`。

## 验证与验收

### 自动化验证

- `cargo test --workspace --locked`：最终布局复验 1051 通过、0 失败、4 项既有忽略。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：最终布局复验通过。
- `cargo fmt --all --check`、`git diff --cached --check origin/main`：通过。相对旧 PR 父提交检查会显示 main 已有 vendor/Cargo.toml 空白，不修改这些任务外内容。
- Dioxus 0.7.9 `dx build --platform web --package rssr-app --locked --release --debug-symbols false`：通过。
- `cargo check -p rssr-app --target aarch64-linux-android --locked`：通过；使用本机 NDK 28.2.13676358，存在主线已有 `LOCAL_DATA_DIR_NAME` dead_code 警告，未将编译冒充设备验收。
- `node --test scripts/browser/api_traffic.test.mjs scripts/browser/server_acceptance.test.mjs`：10/10 通过。
- 最终同构建正式 Web：新增场景 8/8、main workspace 18/18、entries_lifecycle 8/8 全通过；无页面异常，新增场景与 Entries 无意外控制台错误。命令分别为 `node scripts/browser/rssr_native_preparation_acceptance.cjs`、`node scripts/browser/rssr_workspace_acceptance.cjs`、`node scripts/browser/rssr_entries_lifecycle_acceptance.cjs`，设置 `NODE_PATH`、`CHROME_BIN` 和独立 `ARTIFACT_DIR`。原先一次导出断言错误地与 DOM 归一化换行比较，已改为与原始导出快照字节比较。
- `node scripts/browser/rssr_native_transport_acceptance.cjs --process-file target/native-pr40-current/process.json --artifact-dir target/pr40-native-transport-current --baseline false`：13/13 通过，真实 Windows WebView2/Rust/SQLite，无页面或控制台错误；5000 篇标题、全部标志和 6 个订阅地址保持。数据库关闭后 `PRAGMA integrity_check` 为 ok。此项完成于接入 PR #41 布局之前；最终布局后的再次启动命令被自动审批拒绝（仅返回 blocked by policy，未给具体理由），没有绕过；不能声称最终布局已在原生复跑。

### 证据与环境记录

- 当前工作树 `target/workspace-tests.log`、`target/clippy.log`、`target/android-check.log`、`target/browser-tools-tests.log` 保存详细输出；最终结果使用带 `final` 的日志和报告。
- 初次高并行编译触发 Windows 内存不足及一个生成 PDB 损坏；降到 `CARGO_BUILD_JOBS=2`，只移除确切损坏的生成 PDB 后重跑通过，不改产品代码或降低验收要求。
- 旧开发夹具带未发布 migration 8，主线正确拒绝导入；未改 migration 或原库。原生成功验收改用当前 fixture 工具生成独立单库；不是旧库升级通过的证据。
- 改动源码后，旧 server 与新 Web 的 build-id 不一致，被主线写入保护拒绝；使用相同最终源码重建二者再验，不绕过版本检查。
- 旧 head `58157a1a` 的手动 CI `40864200130` 不能代表本次 main 整合或自动 PR CI。推送后的真实 `pull_request` 运行以 PR #40 的最终 head 和检查记录为准。

### 手工验收

- 本轮全部使用 CLI、Rust 测试及浏览器/CDP 自动化，没有 Computer Use。
- Windows 使用隔离 debug EXE、独立数据库与 WebView profile；真实 OS 文件选择器、真实 IME、Android 真机、macOS/Linux GUI、正式安装包未验收。
- 浏览器截图和 390px 视口不是 Android 设备验收。

## 结果

- 本地最终自动化复验通过，按用户授权提交并 push 到 PR #40。推送后的 CI 通过 PR 当前 head 的自动 `pull_request` 运行核验，固定 head、run 链接与最终结果记录在 [PR #40](https://github.com/Develata/RSS-Reader/pull/40) 描述；本记录不把旧手动运行作为新结果。PR 保持草稿，不合并 main、不发布。
- 合并包含 main 的架构和逐字段保存语义，私有规则模块不构成新的跨渲染器 SDK。

## 风险与后续事项

- 原生 BOM 兼容依赖 Dioxus 0.7.9 / sledgehammer 0.6.5 的解释器字段和启动时序，升级时须重跑实际传输验收。
- App 等待、页面回执和已提交存储不同，取消等待不得推导为事务回滚。
- 后续工作从当前适配契约和真实后端场景继续，不恢复已退役的 BrowserStore 测试。

## 给下一位 Agent 的备注

- 先读 `docs/design/native-control-adapter-contracts.md`、`docs/testing/native-control-acceptance.md`。
- 保留本次工作树便于检查本地证据；不能把旧 head 的 574 项测试、90 项表单场景或手动 workflow_dispatch 结果作为当前结论。
- 提交后通过 PR #40 的最新 head 核对自动 CI；不以“无冲突”推导 CI 已通过。

# PR #40 与 main 整合：保留逐字段保存和主线架构

- 日期：2026-10-29
- 作者 / Agent：Codex
- 分支：本地 `codex/pr40-main-integration`；推送目标 `codex/native-ui-preparation-20261029`
- 当前提交基线：`e16df3660bdc03beb7fe8c4878035e6cd1f9ab70`；合入 main `8932a5ac` 的普通 merge 与本记录同一提交，见 PR #40 分支历史
- 相关 commit：`a7107b8b`（PR `58157a1a` + main `6a201d5a`）；`72e0516b`（普通合并 main `9b17a4d5`，只有交接元数据变化）；`67a9596f`（Settings 私有状态转换封装）
- 相关 tag / release：无
- 状态：`validated`（本地自动化范围；远端检查记录在 PR #40 的当前 head/run 回执中）

## 当前原生验收状态（2026-10-29 更新）

用户要求重试后，产品提交 `50af7c99` 的 Windows 原生复验已通过 13/13，关闭后的独立主库校验通过；详见[最新原生复验记录](./2026-10-29-pr40-native-retry.md)。下文的旧布局成功结果和自动审批拒绝保留为当时的执行历史，不再代表当前验收缺口。

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

## 用户追加：Settings 高内聚低耦合优化

- 基线：72e0516b 的自动 [CI 40875772257](https://github.com/Develata/RSS-Reader/actions/runs/40875772257) 与 [Pages 40875772185](https://github.com/Develata/RSS-Reader/actions/runs/40875772185) 全部成功，37 项成功 / 1 项预期部署跳过。以下追加改动须另验，不能沿用该运行作为新 head 的 CI。
- 问题：原 request_lifecycle 只是集中保存 Edits/Writes 字段，session 仍直接维护提交恢复、计数器、读取身份和暂存结果，规则与副作用耦合。
- 改进：draft_edits 独占补丁记录、提交、成功清除和失败恢复；request_lifecycle 独占读写门禁、不可拆分的读取身份及暂存结果。内部状态字段均私有，session 只调用转换方法，负责命令、主题和 Shell 投影。
- 保持行为：仍按实际字段构造补丁，保留后来编辑；只有携带已存储设置的写入作废旧读；后发读取即使失败，也不复活较早暂存结果；全局主题版本判断仍由 session 处理。
- 新增 6 项无 Dioxus/I/O 的不变量测试，覆盖编辑回退、失败恢复与重试、保存归一化及字段归属、后发失败读取、成功写入前后到达的旧读、失败写入释放一次暂存结果。既有 Dioxus 接线测试保留。
- 验证：`cargo test -p rssr-app --locked settings_page` 60/60；`cargo test --workspace --locked` 1057 通过、0 失败、4 既有忽略；fmt、Clippy 全工作区检查通过；Web release 与 rssr-web 重建通过；`rssr_native_preparation_acceptance.cjs` 8/8 通过，无页面/控制台异常。日志为 target/settings-cohesion-tests.log、workspace-cohesion-tests.log、clippy-cohesion.log、web-cohesion-build.log、server-cohesion-build.log、settings-cohesion-web.log，JSON 为 target/pr40-web-cohesion/acceptance.json。
- 提交范围仅 Settings 的两个规则模块、session 接线及相关文档，已作为 `67a9596f` 单独 commit/push。原生复跑的审批限制与未覆盖平台仍适用；不以基线原生结果声称本次追加的原生交互验收。

## 自动 PR CI 发现的验收同步问题

- `67a9596f` 的自动 [CI 40878790163](https://github.com/Develata/RSS-Reader/actions/runs/40878790163) 在 `reading-workspace` 中失败于主线已有的 `preferenceOrdering` 场景；汇总门禁随之失败。其余 35 项成功，Pages 部署 1 项按草稿 PR 预期跳过；Rust、Android、Web、WASM 和主题模块均通过。
- 根因：筛选从“全部”切到“仅未读”再切回“全部”时，最终目标值恰好等于初始存储值。旧脚本在释放请求后只轮询该值，可以在第一次写入提交之前满足条件并刷新页面，取消尚未完成的最终写入。`route.continue()` 只代表请求已继续，不能当作存储回执。
- 修正：`preferenceOrdering` 在操作前监听真实 `PUT /api/v1/workspace` 回执，匹配最终 all/time 请求，断言成功且响应完整后才检查服务器存储并刷新；监听器在 finally 中撤除。保留全部原有持久化断言，不使用固定延时，不改变产品写入语义。
- 回归前提：在第一笔写入仍被拦截时，明确断言服务器已返回初始 all/time，且最终写入尚无回执，验证旧轮询条件存在提前成立的窗口。
- 验证：`node --check scripts/browser/rssr_workspace_acceptance.cjs`、`git diff --check` 通过；Entries 生命周期 8/8 通过，无页面或意外控制台异常，证据在 `target/preference-fix-entries.log` 和 `target/pr40-ci-preference-fix-entries/acceptance.json`。`node scripts/browser/rssr_workspace_acceptance.cjs` 完整工作区 18/18 通过，无页面异常，日志为 `target/preference-fix-workspace.log`，隔离服务器证据在 `target/pr40-ci-preference-fix-workspace/`。
- 此次只改验收脚本和本记录，沿用 `67a9596f` 对应的同源码 Web/server 构建；Rust 产品代码未变。推送后重新核对新 head 的自动 PR CI，结果以 PR 描述的固定 head/run 回执为准。

## 继续整合 PR #42 后的 main

- `e16df366` 自动 [CI 40881410041](https://github.com/Develata/RSS-Reader/actions/runs/40881410041) 与 [Pages 40881410023](https://github.com/Develata/RSS-Reader/actions/runs/40881410023) 已全绿，37 成功 / 1 预期跳过。下载的阅读工作区 artifact `12900278982` SHA-256 为 `d570c89062a59debf6a6ea024db3d04de64493b99533063b63364e799d76a17f`，其中 workspace 18、Entries 8、Settings 8 场景均通过。本地归档为 `target/pr40-e16df366-reading-workspace.zip`。
- 收尾时 main 从 `9b17a4d5` 前进到 `8932a5ac`（PR #42），产生新的 Settings 冲突；上述 CI 只代表旧主线整合，不作为这次整合后的结果。
- 保留 PR #42 全部主题预览、统一显式保存、离页清理、删除确认、移动端操作、视觉样式和既有测试。主题文件读回后先消费导入身份，再走主线 preview_custom_css；导入、卡片和恢复默认均不触发保存。预览尝试先失效旧文件，即使 CSS 校验失败也不允许旧结果覆盖后来操作。
- `Edits::has_unsaved_changes` 提供纯读取，不让 session 重新访问私有 submitted/pending 字段；其余补丁与读写生命周期封装保留，未增加跨层依赖。
- 新增 2 项真实 Dioxus 会话测试：成功文件导入只预览且不保存其他字段、后来有效/无效预览都永久淘汰迟到文件。main 的预览保存及离页测试保留，Settings 合计 66 项通过。
- 新增 Web 验收适配主线保存反馈位置和折叠 CSS 区域，并断言导入 0 次 PATCH、导出草稿字节、显式保存后才落库及旧文件回执不复活。设计契约和用户说明同步删除“导入自动保存”的旧表述。
- 最终本地验证：`cargo test -p rssr-app --locked settings_page` 66/66；`cargo test --workspace --locked` 1067 通过、0 失败、4 既有忽略；`cargo clippy --workspace --all-targets --locked -- -D warnings`、fmt 检查通过；`cargo build -p rssr-web --locked` 和 Dioxus Web release 构建通过。证据为 `target/pr42-settings-tests.log`、`pr42-workspace-tests.log`、`pr42-clippy.log`、`pr42-server-build.log`、`pr42-web-build.log`。
- 同构建浏览器验收：`rssr_native_preparation_acceptance.cjs` 8/8、`rssr_workspace_acceptance.cjs` 18/18、`rssr_entries_lifecycle_acceptance.cjs` 8/8；页面异常均为零，Settings/Entries 无意外控制台错误。证据为 `target/pr42-web-settings/`、`pr42-web-workspace/`、`pr42-web-entries/` 和对应 `pr42-browser-*.log`。
- 主线验收：`node scripts/browser/rssr_server_acceptance.mjs --only settings-preview-explicit-save,build-mismatch-keeps-edits,draft-storage-unavailable,restored-draft-then-sign-in`（配合匹配产物、隔离 server 8121、headless Chrome CDP 19341）4/4 通过，无意外控制台异常，证据 `target/pr42-main-settings-acceptance/assertions.json`。所有专用服务器/浏览器均按运行器清理。
- 原生传输脚本也更新折叠 CSS 区、已退役选择器和显式保存步骤，`node --check` 通过；未执行原生复跑。此前自动审批限制仍适用，不用语法检查或旧 13 项结果代替当前原生运行。
- 这次普通合并不改写历史；提交并 push 后，必须核对新 head 的自动 CI，最终固定 head/run 回执写入 PR 描述。

## 风险与后续事项

- 原生 BOM 兼容依赖 Dioxus 0.7.9 / sledgehammer 0.6.5 的解释器字段和启动时序，升级时须重跑实际传输验收。
- App 等待、页面回执和已提交存储不同，取消等待不得推导为事务回滚。
- 后续工作从当前适配契约和真实后端场景继续，不恢复已退役的 BrowserStore 测试。

## 给下一位 Agent 的备注

- 先读 `docs/design/native-control-adapter-contracts.md`、`docs/testing/native-control-acceptance.md`。
- 保留本次工作树便于检查本地证据；不能把旧 head 的 574 项测试、90 项表单场景或手动 workflow_dispatch 结果作为当前结论。
- 提交后通过 PR #40 的最新 head 核对自动 CI；不以“无冲突”推导 CI 已通过。

# Entries 单条/批量写入与偏好保存规则提取

- 日期：2026-10-28
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 验证基线 HEAD：`6109380b1563ddb1a8812a654980729a9eb9ac3b`
- 相关 commit：本文与实现同批本地提交，以 `git log -1 -- docs/handoffs/2026-10-28-entries-write-lifecycle.md` 定位；记录生成时 `commit: pending`
- 相关 tag / release：N/A
- 状态：`validated`（本地源码及下述验收范围）

## 工作摘要

承接[第一批初始化/查询提取](./2026-10-27-entries-request-lifecycle.md)，将 Entries 的单条/批量写入准入、批量结果身份、完成后的动作顺序和偏好保存资格，独立为页面内普通 Rust 规则。保留现有用户行为、唯一页面状态、reducer 和应用服务持久化语义，为未来原生界面复用提供明确边界。

## 影响范围

- 生产：`crates/rssr-app/src/pages/entries_page/{mod,session,write_lifecycle,preferences_policy}.rs`。
- 文档：`docs/design/native-ui-preparation.md`、`docs/design/ui-shell-bus-page-facade.md`。
- 验收：扩展 `rssr_workspace_acceptance.cjs` 和 `rssr_entries_lifecycle_acceptance.cjs`；新增 `scripts/browser/rssr_entries_writes_native.cjs`。
- 平台：共用页面逻辑影响 Web/desktop/Android；本轮实际运行静态 Web 与 Windows WebView2，Android 仅编译检查。
- 无新增 crate、依赖、公共命令、存储字段或平台分支；application/domain/infra/host、页面样式、DOM 桥接及现有查询 effect 未修改。未 push、打 tag 或发布。

## 关键变更

### 写入规则与任务执行分离

- `EntryWrite::prepare` 读取已有忙碌状态，同一文章重复操作与批量操作期间的新单条操作被拒绝；不同文章仍可并发。session 同步发布 `EntryWriteStarted` 后才启动原页面作用域的任务。
- 单条完成结果保持原顺序发布，随后发 `EntryWriteFinished` 解锁，最后仅在存在已读变更时刷新来源摘要。失败、空结果和仅收藏变更的解锁规则不变。普通迭代器表达顺序，不额外收集结果、克隆文章或分配执行队列。
- `BulkWrite::selection` 捕获明确所选 ID；空选择不产生命令。批量准入仍拒绝现有批量操作或任何待完成单条写入，不自动排队。
- `BeginBulk`、`CancelBulk`、成功/失败状态转移继续由既有 reducer 管理；模型在开始状态发布后捕获其已分配的代次，不复制 busy 集合或再建一套代次状态。旧批量成功/失败被拒绝且不遍历载荷。
- 正在提交的批量操作不会因筛选变化而取消。成功先发布 `SelectionApplied`，再刷新来源摘要；失败释放 busy，保留当时当前选择，不将旧请求选择快照写回页面。revision 与重查仍沿既有路径处理。
- `WriteEffect` 仅表示本页面的状态发布与来源刷新；session 负责执行，未引入通用页面框架。

### 偏好保存资格

- `preferences_policy::can_schedule_save` 同时要求捕获的就绪标记与当前 Loaded 状态；`is_current_save` 在任务开始时再检查当前就绪、分组、归档、已读、收藏及来源 URL 列表。
- 旧选择任务不进入保存；读取失败的回退默认值不能覆盖原存储。URL 顺序比较保持现状，不新增规范化或状态镜像。
- 共享 `EntriesWorkspaceService` 继续串行执行比较和写入，页面不新增队列、去重缓存或持久化职责。失败错误继续正常发布，后续选择仍可保存。
- 页面销毁仍取消其未完成 UI 任务；这不代表回滚已提交的数据。

### 新增验证

- 14 个普通 Rust 测试：9 个写入生命周期场景、5 个偏好资格场景；覆盖并发互斥、失败解锁、选择快照、批量身份与取消、发布顺序、过期载荷、就绪变化和各偏好字段。
- 4 个 Dioxus 集成测试：同步批量互斥及失败重试、旧批量结果不干扰新批次、排队偏好任务重验就绪、页面销毁取消单条/批量/偏好任务。原有集成测试保留。
- 4 组新增真实浏览器场景：不同文章并发写入；批量已读/归档失败后的重试；单条/批量写入期间切换未读筛选；偏好保存失败后的重试及重开。已加入现有工作区主入口与生命周期入口。
- Windows 原生脚本对独立进程执行标记、批量已读/归档和偏好切换，以只读 SQLite 查询确认提交；关闭进程后重新启动同一测试库，再验偏好和文章状态。脚本使用本机 Node 24 的 `node:sqlite`，不添加产品依赖。

## 验证与验收

本轮证据保存在 `target/entries-writes-20261028/`（ignored，不随源码提交）。基线与候选使用相同脚本、fixture 和操作路径。

### 自动化验证

- `cargo test -p rssr-app --locked`：基线 173 个单元测试 + 4 个集成测试通过；候选 191 + 4 通过。各有 1 个原有性能测试 ignored；新增共 18 项。
- `cargo test --workspace --locked`：489 通过、0 失败、2 ignored（原有 presenter 与 refresh adapter 性能测量）。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo fmt --all --check`：通过。
- `cargo check -p rssr-app --target wasm32-unknown-unknown --locked`：通过。
- `cargo check -p rssr-app --target aarch64-linux-android --locked`：通过；仅为子进程设置 NDK 28.2.13676358 的 clang/clang++/llvm-ar/linker 路径，未安装到设备。
- `target/release-ui-tools/dioxus-0.7.9/dx.exe build --platform web --package rssr-app --locked --release --debug-symbols false`：基线和候选均通过，无 ERROR / wasm-opt failure。工具 SHA256 已核对为 `7395c0bbbe17e6aa86215634cf511387fd1ff8afba58661d0ac81fab28eb9fed`，沿用 [现有构建修复](./2026-10-27-wasm-opt-release-build.md)。
- `cargo build -p rssr-app --release --locked --target-dir target/reading-baseline-20261025/build-b`：基线和候选均通过，随后分别复制 EXE 固定身份。
- 三个本批浏览器脚本的 `node --check`：通过。
- `NODE_PATH=C:/Users/QQ/node_modules`，分别指定 `PUBLIC_DIR` 为 `baseline/public-optimized` / `candidate/public`（均位于本轮证据目录），运行 `node scripts/browser/rssr_entries_lifecycle_acceptance.cjs`：基线、候选各 12 组通过，无 page/console error；输出分别在 `baseline/browser-writes`、`candidate/browser`。
- 候选 `node scripts/browser/rssr_directory_offline_regression.cjs`：150 项通过，无 console error。
- 候选 `node scripts/browser/rssr_directory_window_boundaries.cjs`：11 组通过，无 page error，覆盖迟到回复、被替换导航及筛选后的末页正文返回。
- Windows 真实宿主：使用 `run-native-writes.ps1` 启动隐藏的独立进程、独立 profile 和 5000 篇测试库；调用 `rssr_entries_writes_native.cjs --mode write`，结束该进程后重新启动并调用 `--mode reopen`。基线与候选各 11 + 9 项通过，无未捕获页面错误。输出在 `native-baseline-final`、`native-candidate`。只关闭本轮创建且可执行文件路径匹配的进程，验收端口 9276 已释放。
- 本批路径的 `git diff --check`：通过。提交范围按精确文件路径核对。

### 界面与未覆盖范围

- 已查看基线 1536×864 与 390×844 截图；候选同位置截图逐字节相同。桌面 PNG SHA256 为 `ebb194f25453bd01ee65469173895294c016b9db8650adfc609f3cf3dba7e063`，窄屏为 `61c65e1579ef6250be990ea31b076ee041160bc809e51691804ff77221b2e448`。该结论限于捕获页面。
- Windows 为真实 WebView2 与 SQLite 的自动化验收，不是人工全功能验收。未驱动用户桌面鼠标/键盘，未访问个人数据目录。
- Android 设备、真实系统 IME、Linux/macOS、`rssr-web` 服务端 UI 端到端、签名发布/安装本轮未验；工作区测试的服务端用例不等于实际服务端 UI 验收。
- 浏览器采用离线 feed fixture；Windows 使用独立的既有阅读 fixture。没有外网订阅、性能计时、内存或原生渲染器能力的新结论。

### 验证中修正的脚本/命令问题

- 初次基线误用了旧裸 `dx ... --release` 命令，重现已记录的 DWARF/wasm-opt 故障。日志和未优化产物保留在 `baseline/web-build.log`、`baseline/public`；随后按现有修复使用匹配的 dx 0.7.9 与 `--debug-symbols false` 重建，最终基线对照只使用 `baseline/public-optimized`。
- 新原生验收脚本首次基线未展开“列表选项”，因此找不到隐藏的归档复选框。补齐真实的展开/关闭操作后重跑；失败记录在 `native-baseline`，最终通过记录在 `native-baseline-final`。没有为此修改产品行为或跳过断言。

### 源码与产物身份

- 基线 Web 和 Windows 均由 `6109380b` 在本批生产源码修改前构建；候选为该基线加本批四份生产文件。`candidate/source.json` 与原生 `identity.json` 记录源码/EXE SHA256；验收后已复核源码未变。
- Web 的 `web-identity.json` 从 index 实际引用关系定位 JS/WASM，不以输出目录残留的旧 hash 文件推断当前产物。
- 基线 WASM：`assets/rssr-app_bg-dxhc19a0d6265d0f.wasm`，SHA256 `106a0f5aa66533435bb0cdd05aee0e1d6121b31d741e32dd09322238f3094533`。
- 候选 WASM：`assets/rssr-app_bg-dxh9cea6624cb23ae7.wasm`，SHA256 `2b8e5416b386c8b1b78db888e29d57fe945d541c69da1726c40483e026010474`。
- Windows 基线 EXE：SHA256 `e7eeeba7aab7da15cb9702f90e18ee76d39d947533cb580258f0093907574188`；候选 EXE：`ced555f575e192c0e7eb3d7211ead1351e55f8b1cb5741047f2414e4376d1b8c`。

## 结果

第二批规则提取完成，生产入口全部接入，既有行为在上述测试与同路径宿主验收范围内保持一致。实现、设计边界、验收脚本与本记录一起本地提交；不包含推送、tag 或发布。

## 风险与后续事项

- 同日[复核与验收修正](./2026-10-28-entries-write-review.md)发现并修复 Web 批量重试断言竞态、Windows 仅抽查文章标记的漏报；新增完整标记比对与负向对照。未发现本批 Rust 提取的新增行为问题。
- 下一候选是 Reader 的异步结果归属，先复用其既有 reducer 与访问代次，再判断具体提取边界；不因两页都有 session 就引入通用运行时。
- 单条/批量写入的任务作用域、取消语义及持久化串行边界本批均保持现状。未来改变这些行为需单独论证，不能以“规则提取”名义静默修改。
- 首次测试时已有 `.agents/skills/speckit-*`、`.specify/` 删除，两份旧交接修改，以及 Penpot/Figma/PR 评估等未跟踪文件，全部保留并排除在本批提交之外。

## 给下一位 Agent 的备注

- 从 `write_lifecycle.rs`、`preferences_policy.rs` 的普通 Rust 测试进入，再看 `session.rs` 的执行适配和集成测试。批量取消和 busy/revision 转移仍在原 reducer，偏好串行比较/写入仍在 application。
- [原生界面前置模块](../../../design/native-ui-preparation.md) 已记录规则边界及三类验收入口。实际运行证据和编译检查应分别报告；后续源码变化不能直接沿用本次回执。

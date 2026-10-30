# Entries 初始化与查询生命周期规则提取

- 日期：2026-10-27
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 验证基线 HEAD：`c22a58d94c12a125329c3a076c25858cbb73c1d1`
- 相关 commit：本文与实现同批本地提交，以 `git log -1 -- docs/handoffs/2026-10-27-entries-request-lifecycle.md` 定位；记录生成时 `commit: pending`，候选为上述基线加本文所列改动
- 相关 tag / release：N/A
- 状态：`validated`（本地实现及本记录明确范围内的验收完成）

## 工作摘要

落实[第一批建议](./2026-10-27-native-preparation-next-step.md)：把 `EntriesPageSession` 中初始化和查询的准入、结果归属、写入后重查及返回页码顺序，提取成页面内可独立测试的普通 Rust 模块。现有 Dioxus session 继续执行命令、管理页面任务并发布状态。生产行为及七个 crate 的职责保持现状。

## 影响范围

- 生产模块：`crates/rssr-app/src/pages/entries_page/{mod,session,request_lifecycle}.rs`。
- 验收脚本：新增 `scripts/browser/rssr_entries_lifecycle_acceptance.cjs`，修正既有 `rssr_workspace_acceptance.cjs` 的偏好保存完成判据。
- 设计文档：`docs/design/native-ui-preparation.md`、`docs/design/ui-shell-bus-page-facade.md`。
- 平台：共用 UI 源码；本轮实际运行 Web 与 Windows WebView2，Android 仅交叉编译检查。未修改 application/domain/infra/host、数据库格式、公共命令、CSS 或宿主事件桥接。
- 工作流、依赖、tag、发布：无变化；未 push。

## 关键变更

### 页面内的具体生命周期模型

- `EntriesRequestLifecycle` 仅持有初始化与查询的两个独立代次；请求记录捕获代次、已有写入 revision 和返回页码等标量。不建立文章副本或另一套页面状态，不依赖 `Signal`、`spawn`、DOM 或存储。
- 初始化根据当前偏好状态决定是否加载偏好；旧初始化的成功或失败均不能覆盖新结果。初始化与查询可并行，互不使对方失效。
- Pending 偏好拒绝启动查询；Unavailable 仍允许读取，并保留错误提示和禁止写回回退偏好的原规则。
- 查询完成先检查身份，再检查 `flags_revision`。已完成写入使当前查询失效时，只有 `reload_revision` 尚未变化才请求重查；旧查询不重复重查，不恢复旧行或发布旧错误。
- 接纳的 intent 保持原顺序，仅在 `SetEntries` 后插入捕获的返回页码，让既有 reducer 按新列表约束页码。拒绝的载荷不遍历、不复制，输出也不额外构建 `Vec`。
- session 使用非订阅读取、同步取得请求身份，再启动原页面作用域的任务；结果判定借用在发布前结束。现有归档时钟、查询 effect、位置存储读取、单条/批量写入及偏好保存编排保留。

### 测试与浏览器验收

- 新增 10 个普通 Rust 场景测试，覆盖准入、独立代次、旧成功/失败、查询和已完成写入交错、重复重查抑制、返回顺序/页码收缩、空列表和拒绝载荷不遍历。
- 保留并适配原有 session 集成测试；另增 2 个 Dioxus 集成场景：批量写入已有重查时拒绝迟到列表，以及页面销毁取消未完成的初始化/查询任务。
- 新浏览器入口复用既有离线 fixture 与工作区场景，分别对基线和候选运行选择、来源筛选、查询稳定性、待完成写入/失败重试、偏好重开、来源身份，以及桌面/窄屏搜索、刷新、正文返回。

### 偏好验收脚本的完成判据

初次候选验收在快速“全部 → 仅未读 → 全部”后立即重开时失败。保留原始失败记录，没有据此修改生产偏好逻辑。原脚本只等待 Web Locks 空闲 150ms，不能保证后续保存任务已完成。

隔离诊断对基线和候选各运行 10 次，记录刷新前已提交的偏好与锁状态：基线 4 次、候选 5 次在原等待条件结束后仍观察到中间的 `unread_only` 和正在持有的存储锁；再等待最终值，4–14ms 内均提交为 `all`，20 次最终均通过。证据为 `preference-settle-observed.json`；初次未增加观测等待的失败保存在 `preference-diagnosis.json` 和候选初次 `browser/acceptance.json`。

脚本现在同时要求已提交的偏好为最终 All/Time，且存储锁完成静默等待，再刷新验证恢复；未增加固定 sleep，未延长超时，未跳过恢复断言。修正后同一脚本的基线、候选全部通过。此修正只涉及验收屏障，不扩大本批生产重构范围。

## 验证与验收

全部本地证据位于 `target/entries-lifecycle-20261027/`（ignored，未随源码提交）。以下为本轮实际执行结果，不能作为发布或完整跨平台验收。

### 自动化验证

- `cargo test -p rssr-app --locked`：基线 159 个单元测试 + 4 个集成测试通过；候选 171 + 4 通过。各有 1 个原有性能测试默认 ignored。本轮新增共 12 个测试。
- `cargo test --workspace --locked`：469 通过、0 失败、2 ignored（原有 presenter 与 refresh adapter 性能测量），含工作区单元、集成及文档测试；见 `workspace-test.log`。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：通过。
- `cargo fmt --all --check`：通过。
- `cargo check -p rssr-app --target wasm32-unknown-unknown --locked`：通过。
- `cargo check -p rssr-app --target aarch64-linux-android --locked`：通过；仅为当前进程设置 NDK 28.2.13676358 的 clang/clang++/llvm-ar/linker 路径，未安装到设备。
- `dx build --platform web --package rssr-app --locked --release`：基线及候选均完成可运行包；两次均提示 dx 0.7.10 与 Dioxus 0.7.9 版本不同，且 `wasm-opt` 以 `0xc0000409` 退出后打包继续成功。不能声称 wasm-opt 优化成功，也未用本轮构建推导性能收益。
- `cargo build -p rssr-app --release --locked --target-dir target/reading-baseline-20261025/build-b`：候选 Windows release 构建通过。
- `node --check scripts/browser/rssr_entries_lifecycle_acceptance.cjs`、`node --check scripts/browser/rssr_workspace_acceptance.cjs`：通过。
- 本批路径的 `git diff --check`：通过；提交仅暂存本文列出的生产文件、验收脚本和文档。
- `NODE_PATH=C:/Users/QQ/node_modules` 下，分别设置 `PUBLIC_DIR=target/entries-lifecycle-20261027/{baseline,candidate}/public`、`ARTIFACT_DIR=target/entries-lifecycle-20261027/{baseline,candidate}/browser-final`，运行 `node scripts/browser/rssr_entries_lifecycle_acceptance.cjs`：各 8 组通过，无 page/console error。
- 候选同样指定离线产物运行 `node scripts/browser/rssr_directory_offline_regression.cjs`：150 项通过，无 console error。
- `node scripts/browser/rssr_directory_window_boundaries.cjs`：11 组通过；覆盖迟到焦点、替换的导航、来源筛选后末页正文返回。
- Windows 实际宿主自动化：基线与候选各 38 项通过，5000 篇独立测试库，覆盖时间/来源目录、选择、跨页、显隐和连续正文返回。使用隐藏启动的独立进程、独立 WebView2 profile 和 CDP；不驱动用户桌面。运行器与回执在 `run-native.ps1`、`native-{baseline,candidate}/`，只关闭本轮创建的进程。

### 界面与手工验收边界

- 已查看基线桌面/窄屏截图；最终候选与基线相同位置的 PNG SHA256 完全相同：1536×864 为 `989adcb3cf76b2b38da924be87eebdebea5a52b0936930030a286f95dab16ec4`，390×844 为 `61c65e1579ef6250be990ea31b076ee041160bc809e51691804ff77221b2e448`。只说明这些捕获页面视觉一致。
- Windows 为真实 WebView2 自动化，不是人工全功能验收。Android 设备、真实系统 IME、Linux/macOS、服务端 `/api/v1` 模式及发布包安装本轮未验；静态 Web fixture 使用本地 feed 响应，不代表外网订阅行为。
- 性能计时、内存和完整交互覆盖本轮未测；后台宿主样本不作为前台性能依据。

### 产物与源码身份

- Web 基线构建来自 `c22a58d9`。保留 `baseline/public/` 与 `candidate/public/`，`web-identity.json` 只记录 index 实际引用的 JS/WASM，避免把输出目录残留的旧 hash 文件误认为当前产物。
- 基线 WASM：`assets/rssr-app_bg-dxhcef277a23f9636b5.wasm`，SHA256 `64d7dc31e6d730c488675daa69b16ae573659c99937920290f0e5e942c3d454c`。
- 候选 WASM：`assets/rssr-app_bg-dxh2bae4127cbbbd6cc.wasm`，SHA256 `ac09d1aff6f9d2fa039b313aa4de97de42d1bb5732d6cd4b707599552a4f4f95`。
- Windows 基线 EXE：SHA256 `0b148e2b1986cfe6bd8d0f36d90c943069b4195b6b3d129679697333413eb303`，复用已核对的 `915b9888` 构建；该提交到本轮 `c22a58d9` 只有交接文档差异，生产源码相同。本轮重新运行实际宿主验收，未把旧回执当成新结果。
- Windows 候选 EXE：SHA256 `6ee3110c97db01a908a0595d69dd15f8ecb76769ce505d162996b92ea985b9d1`。回执同时记录三份本批生产源码的 SHA256；产物构建后只调整测试脚本和文档。

## 结果

初始化与查询规则已从 Dioxus 会话执行器独立出来，实际入口均已接入，全工作区测试及同路径 Web 与 Windows 验收通过。本地提交范围限于本文列出的生产文件、验收脚本和文档；不涉及推送、tag 或发布。

## 风险与后续事项

- 下一批可按相同边界处理 Entries 的单条/批量写入准入和偏好保存策略，仍保留现有失败解锁、选择范围及应用服务串行保存语义。
- 未来原生 UI 应提供任务生命周期、状态发布和位置存储适配，继续使用本模块的同一规则；当前工作不等于已接入原生渲染器。
- 保留本轮暴露的验收屏障教训：UI 已显示最终选择、存储锁暂时空闲，均不能单独证明异步保存已经提交。
- 任务开始时已有 `.agents/skills/speckit-*`、`.specify/` 删除、两份旧交接修改和 Penpot/Figma 等未跟踪文件；均不属于本批，保留且不纳入提交。

## 给下一位 Agent 的备注

- 先读 `request_lifecycle.rs` 的普通 Rust 测试，再读 `session.rs` 的执行适配及集成测试；`mod.rs` 的查询/偏好 effect 未迁移。
- 当前设计契约见 [原生界面前置模块](../../../design/native-ui-preparation.md) 与 [shell/facade 边界](../../../design/ui-shell-bus-page-facade.md)。不要把页面交互状态下沉为 application/domain 的业务语义。
- 本批证据属于上述源码与产物身份，后续改动应按影响重新验证，不能沿用本记录宣称新候选已经验收。

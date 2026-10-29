# 文章工作区会话的高内聚、低耦合重构

- 日期：2026-10-27
- 作者 / Agent：Codex
- 分支：`codex/review-workspace-20261027`
- 基线 HEAD：`7c49781c`，包含上一轮切页竞态修复 `66d4a1f5`
- 验证源码 HEAD / 重构 commit：`8de67b8de7b614dadd467fb9638efddf1a585dcf`
- 交接记录：另行提交本文件，验证身份以上述重构 commit 为准。
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

按查询、偏好保存、文章写入划清文章页会话的内部职责，同时减少跨层的偏好字段拆装。
本轮是现有前端内部的行为保持重构；继续使用 facade/session → UiCommand → Backend 的既有链路。

## 影响范围

- 模块：文章页装配、`EntriesPageSession`、查询生命周期、写入提示投影、UI 偏好保存命令与 runtime。
- 平台：共享 Rust UI，涉及 Web、Windows / Linux / macOS、Android。
- 额外影响：模块维护说明及架构文档；不新增依赖、crate、数据库迁移或远端 API。

## 关键变更

- `session.rs` 从 423 行生产代码收拢为 99 行入口：装配、状态访问、发布与使旧读取失效。
  `session/queries.rs`、`workspace.rs`、`articles.rs` 分别负责查询、偏好保存、文章写入；原回归迁至 `session/tests.rs`。
- `WorkspaceSaveQueue` 归保存模块持有，内部 `Rc<RefCell<...>>` 不再暴露给读取协调器。
  页面内待保存展示与 App 任务所需队列保留各自生命周期；卸载页面后仍完成最后一次选择。
  `EntriesRequestLifecycle` 回到只维护初始化／查询身份的职责。
- `use_entries_page_session` 统一创建页面实例所需状态，页面组件不再了解内部队列如何装配。
- `EntriesWorkspaceState` 整体经过 effect、会话方法、`SaveBrowsingPreferences` 和 Backend，省去五个偏好字段在中间层反复拆装。
  偏好就绪标记仍独立捕获，保留拒绝过早保存默认值的规则。
- `write_feedback.rs` 负责文章写入提示的纯投影；session 负责投递，reducer 负责页面状态。
  回执提示复用原字符串所有权，不为搬移文案额外克隆。
- 批量写入入口只接受明确的批量动作，不再借由任意 EntriesCommand 加 writing 布尔值组合。
- 新增 `session/README.md`：按修改场景给出入口、职责和生命周期约束；架构说明同步当前 Backend 边界。

## 验证与验收

证据目录：`E:/gitclone/RSS-Reader/target/cohesion-workspace-20261027/`。
复用既有独立缓存与真实服务器 fixture，`CARGO_BUILD_JOBS=1`；
`RSSR_BUILD_ID=pr32-integration-20261027` 用于本地前后端兼容，不作为新源码身份。

### 自动化验证

- `cargo test -p rssr-app --locked`：278 passed、1 ignored（既有手工性能测量），另 4 项样式契约通过，见 `app-tests.log`。
- 迁移前后对照：27 个 session 测试名称／顺序与 104 条断言全部保留；仅调整构造和聚合参数调用。
  UI runtime 的既有行为快照测试通过，未更新其预期数据。
- `cargo fmt --all --check`、`git diff --check`：通过。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：通过，见 `clippy.log`。
  迁移后未使用的测试 import 已清理，最终静态检查无 warning。
- DX 0.7.9：`dx build --platform web --package rssr-app --locked --release --debug-symbols false --verbose`：完整通过（137.33 s）。
  实际执行 Binaryen 129 `wasm-opt -Oz ... --strip-debug` 并完成优化，见 `web-release.log`。
- `node scripts/browser/rssr_workspace_acceptance.cjs`：16 组全部通过、页面异常 0，见 `browser.log`。
  沿用原有脚本和断言，覆盖来源身份、筛选保存、快速切换、错误后重试、编辑与批量操作、响应式界面及切页。
- Chrome CDP 增加 150 ms 网络延迟：`preferencesSurviveNavigation`、`reopeningDuringSourceEdit`、
  `articleWritesSurviveNavigation` 全部通过、页面异常 0，见 `browser-latency.log`。
- 证据目录的 `verification-receipt.json` 记录源码身份及当前入口 JS／引用 WASM 的 SHA-256。

### 手工验收

- 本轮未执行 Android 实机或桌面应用全流程手工验收；不把共享 Rust／浏览器测试等同实机验收。

## 结果

职责拆分和参数聚合已完成，现有测试验证上一轮的写入顺序与离页行为仍保持。
代码和模块说明已本地提交；本交接记录单独提交。尚未推送，也未触发本分支远端 CI。

## 风险与后续事项

- 本轮优化目标是维护边界，不宣称耗时或内存有测量到的改善。
- 写入仍属于 App，后续只读任务仍属于原页面；结果未知不自动重试。这里的写入顺序只覆盖当前 App 实例。
- 目录、归档时钟、搜索／分页查询依赖和公开 DOM 契约不做重设计；原生端需要各自的实机验收。
- 没有引入跨页面的通用任务框架；今后的跨页抽象应有独立复用证据。

## 给下一位 Agent 的备注

- 先读 `crates/rssr-app/src/pages/entries_page/session/README.md`；按要修改的行为进入对应模块。
- 保留 `state.workspace_wanted` 与写入队列的生命周期差别，不能因字段相似就删除其中一个。
- 对完整偏好值进行保存；不要把来源 URL、筛选状态等拆回多个独立保存参数。
- 当前 worktree：`C:/Users/QQ/.codex/worktrees/integrate-reading-workspace/RSS-Reader`；原主检出的任务外修改保持原样。

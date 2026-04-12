# Entries Workspace Service Consolidation

- 日期：2026-04-12
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：910f8a8
- 相关 commit：910f8a8
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

将文章列表页启动快照与工作区偏好保存判断从 UI runtime 收敛到 application use case，继续执行新宪章下的壳核分离与 use case 边界收敛。

## 影响范围

- 模块：
  - `crates/rssr-application/src/entries_workspace_service.rs`
  - `crates/rssr-application/src/composition.rs`
  - `crates/rssr-application/src/lib.rs`
  - `crates/rssr-app/src/ui/runtime/entries.rs`
  - `crates/rssr-app/src/ui/runtime/services.rs`
- 平台：
  - Linux
  - Web
  - 桌面端共享 application/runtime 路径
- 额外影响：
  - release UI regression 记录：`target/release-ui-regression/20260412-codex-entries-workspace/summary.md`

## 关键变更

### Application Use Case

- 新增 `EntriesWorkspaceService`，统一承载文章列表页 bootstrap 读取设置、工作区状态、订阅摘要的应用层流程。
- 新增 `save_workspace_if_changed`，由 application 层读取当前工作区状态并决定是否持久化，避免 UI runtime 持有业务状态比较逻辑。
- 保留 `feed_id` 记忆为 best-effort 行为，兼容既有 UI runtime 中忽略记忆失败、不阻塞页面启动的语义。

### UI Runtime

- `EntriesCommand::Bootstrap` 改为调用 application 的 `EntriesBootstrapInput`，仅负责把结果映射为页面 intent。
- `SaveBrowsingPreferences` 改为调用 `save_workspace_if_changed`，UI 不再直接读写工作区状态以判定变更。
- `EntriesPort` 删除设置、工作区、订阅列表的零散直通方法，改为暴露文章页 bootstrap 与保存偏好用例入口。

### Composition

- `AppUseCases` 注入 `entries_workspace_service`，与既有 `startup_service`、`subscription_workflow`、`import_export_service` 并列成为 application 层用例。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo fmt --check`：通过
- `git diff --check`：通过
- `cargo test -p rssr-application`：通过，28 tests
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-cli`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo test --workspace`：通过
- `bash scripts/run_release_ui_regression.sh --no-serve --with-rssr-web --port 8348 --web-port 18848 --log-dir target/release-ui-regression/20260412-codex-entries-workspace`：通过

### 手工验收

- 未执行独立手工 UI 点击验收；本次依赖 release UI 自动门禁与 rssr-web browser feed smoke。

## 结果

- 本次交付可合并；文章列表页启动与偏好保存职责已从 UI runtime 下沉到 application use case。
- `rssr-web browser feed smoke` 本轮通过，未复现此前关注的超时问题。

## 风险与后续事项

- 下一步可继续收敛 reader 查询/加载相关 use case，但正文渲染、展示格式和导航 UI intent 属于表现层，不应不加区分地下沉。
- 当前 bootstrap 错误在 UI 层使用统一外层文案，application 层保留具体 context；若后续需要更细粒度页面错误展示，可增加结构化错误/结果模型。
- push 仍依赖 GitHub HTTPS 凭据；本地验证通过不代表远端已更新。

## 给下一位 Agent 的备注

- 阅读入口：`crates/rssr-application/src/entries_workspace_service.rs` 与 `crates/rssr-app/src/ui/runtime/entries.rs`。
- 若继续推进 application use case 收敛，优先检查 `EntriesCommand::LoadEntries`、reader 页面加载、条目状态切换是否仍有 UI runtime 直接编排业务流程。

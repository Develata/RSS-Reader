# Reader Service Consolidation

- 日期：2026-04-12
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：84b9eba
- 相关 commit：84b9eba
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

将阅读页的文章加载快照与读/收藏状态切换收敛到 application use case，继续减少 UI runtime 对业务流程的直接编排。

## 影响范围

- 模块：
  - `crates/rssr-application/src/reader_service.rs`
  - `crates/rssr-application/src/composition.rs`
  - `crates/rssr-application/src/lib.rs`
  - `crates/rssr-app/src/ui/runtime/reader.rs`
  - `crates/rssr-app/src/ui/runtime/services.rs`
- 平台：
  - Linux
  - Web
  - 桌面端共享 application/runtime 路径
- 额外影响：
  - release UI regression 记录：`target/release-ui-regression/20260412-codex-reader-service/summary.md`

## 关键变更

### Application Use Case

- 新增 `ReaderService`，统一承载阅读页文章实体读取与导航快照加载。
- 新增 `ToggleReadInput` / `ToggleReadOutcome` 和 `ToggleStarredInput` / `ToggleStarredOutcome`，由 application 层执行从当前状态到目标状态的状态变化。
- 保留阅读导航为 best-effort：文章实体加载成功时，导航读取失败仍返回默认导航，兼容此前 UI runtime 的 `unwrap_or_default()` 行为。

### UI Runtime

- `ReaderCommand::LoadEntry` 改为调用 `reader_service.load_entry`，runtime 只负责把 domain entry 映射为 `ReaderPageLoadedContent`。
- 正文 HTML/Text 选择、发布时间展示 fallback、原文链接展示 fallback 和快捷键/按钮文案仍留在 UI 层，避免把呈现规则混入 application。
- `ToggleRead` / `ToggleStarred` 改为调用 application toggle use case，并使用 outcome 生成页面状态文案。

### Composition

- `AppUseCases` 注入 `reader_service`，与 `entries_workspace_service`、`startup_service` 等 use case 并列。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo fmt --check`：通过
- `git diff --check`：通过
- `cargo test -p rssr-application`：通过，33 tests
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-cli`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo test --workspace`：通过
- `bash scripts/run_release_ui_regression.sh --no-serve --with-rssr-web --port 8349 --web-port 18849 --log-dir target/release-ui-regression/20260412-codex-reader-service`：通过

### 手工验收

- 未执行独立手工 UI 点击验收；本次依赖 release UI 自动门禁覆盖 reader smoke 和 rssr-web browser feed smoke。

## 结果

- 本次交付可合并；阅读页加载和状态切换流程已进入 application 层。
- `rssr-web browser feed smoke` 本轮通过，未复现超时。

## 风险与后续事项

- `EntriesCommand::LoadEntries` 仍通过 UI runtime 直接调用 `entry_service.list_entries`，是下一处适合收敛的 use case。
- entries 列表页的读/收藏切换仍直接调用 `entry_service`，可与列表查询一起收敛为 entries list use case。
- reader 正文选择和日期格式化目前保留在 UI 层；若后续需要多前端共享展示规则，应先明确 presentation contract，再决定是否新增独立 presenter 层。
- push 仍依赖 GitHub HTTPS 凭据；本地验证通过不代表远端已更新。

## 给下一位 Agent 的备注

- 阅读入口：`crates/rssr-application/src/reader_service.rs` 与 `crates/rssr-app/src/ui/runtime/reader.rs`。
- 继续推进 application use case 收敛时，建议优先处理 `crates/rssr-app/src/ui/runtime/entries.rs` 中 `LoadEntries` 与列表页读/收藏状态切换。

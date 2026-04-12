# Entries List Service Consolidation

- 日期：2026-04-12
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：6601292
- 相关 commit：6601292
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

将文章列表页的条目查询与读/收藏状态切换收敛到 application use case，继续减少 UI runtime 对业务流程和状态变化的直接调用。

## 影响范围

- 模块：
  - `crates/rssr-application/src/entries_list_service.rs`
  - `crates/rssr-application/src/composition.rs`
  - `crates/rssr-application/src/lib.rs`
  - `crates/rssr-app/src/ui/runtime/entries.rs`
  - `crates/rssr-app/src/ui/runtime/services.rs`
- 平台：
  - Linux
  - Web
  - 桌面端共享 application/runtime 路径
- 额外影响：
  - release UI regression 记录：`target/release-ui-regression/20260412-codex-entries-list-service/summary.md`

## 关键变更

### Application Use Case

- 新增 `EntriesListService`，承载文章列表页条目查询流程。
- 新增 `ToggleEntryReadInput` / `ToggleEntryReadOutcome` 和 `ToggleEntryStarredInput` / `ToggleEntryStarredOutcome`，由 application 层决定从当前状态到目标状态的变化并执行持久化。
- 为列表查询和状态切换补 application 层错误 context，UI runtime 不再拼接重复的底层业务错误前缀。

### UI Runtime

- `EntriesCommand::LoadEntries` 改为调用 `entries_list_service.list_entries`，runtime 只负责把 outcome 映射为 `EntriesPageIntent::SetEntries`。
- `ToggleRead` / `ToggleStarred` 改为调用 application toggle use case，并用 outcome 生成页面文案与 reload intent。
- 保留条目标题插入、状态提示文案和页面 intent 组装在 UI 层，避免 presentation 规则进入 application。

### Composition

- `AppUseCases` 注入 `entries_list_service`，与 `entries_workspace_service`、`reader_service` 等 use case 并列。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo fmt --check`：通过
- `git diff --check`：通过
- `cargo test -p rssr-application`：通过，36 tests
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-cli`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo test --workspace`：通过
- `bash scripts/run_release_ui_regression.sh --no-serve --with-rssr-web --port 8350 --web-port 18850 --log-dir target/release-ui-regression/20260412-codex-entries-list-service`：通过

### 手工验收

- 未执行独立手工 UI 点击验收；本次依赖 release UI 自动门禁覆盖 entries 页面和 rssr-web browser feed smoke。

## 结果

- 本次交付可合并；文章列表页查询与条目状态切换已进入 application 层。
- `rssr-web browser feed smoke` 本轮通过，未复现超时。

## 风险与后续事项

- `FeedsPort::list_entries` 仍直接调用 `entry_service.list_entries`，需要先确认 feeds 页面查询语义是否与 entries 列表一致，再决定复用 `EntriesListService` 或新增 feeds-specific use case。
- `EntryService` 仍公开底层直通能力给 composition 内部使用；后续不应盲目删除，reader 和 entries list use case 仍依赖它作为 application 内部基础能力。
- push 仍依赖 GitHub HTTPS 凭据；本地验证通过不代表远端已更新。

## 给下一位 Agent 的备注

- 阅读入口：`crates/rssr-application/src/entries_list_service.rs` 与 `crates/rssr-app/src/ui/runtime/entries.rs`。
- 下一步建议审查 `crates/rssr-app/src/ui/runtime/services.rs` 中剩余直通方法，优先处理 feeds 页面查询和设置页保存是否需要 application use case 包装。

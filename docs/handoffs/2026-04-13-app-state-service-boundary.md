# App State Service Boundary

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：e696328
- 相关 commit：e696328
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

保留 `AppStateService`，但去掉它对完整 `AppStateSnapshot` 的泛化透传接口，只保留按稳定语义字段读写的入口，并补上对应单元测试。

## 影响范围

- 模块：
  - `crates/rssr-application/src/app_state_service.rs`
- 平台：
  - desktop
  - Android
  - Web
  - CLI
- 额外影响：
  - application state 边界
  - app-state service 语义收口

## 关键变更

### Remove Generic Snapshot Pass-through

- `AppStateService` 的公开 `load()` / `save()` 已移除。
- 内部保留私有 `load_snapshot()` / `save_snapshot()` helper。
- 对外现在只保留稳定语义入口：
  - `load_entries_workspace`
  - `save_entries_workspace`
  - `load_last_opened_feed_id`
  - `save_last_opened_feed_id`

### Add Focused Tests

- 新增单元测试覆盖：
  - workspace slice 读取
  - 保存 workspace 时不污染 `last_opened_feed_id`
  - 保存 `last_opened_feed_id` 时不污染 workspace

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo test -p rssr-application`：通过
- `cargo test -p rssr-app`：通过
- `cargo check --workspace`：通过
- `git diff --check`：通过

### 手工验收

- 未执行；本次为 application 边界收口，依赖 application/app 自动化验证。

## 结果

- 本次变更已验证，可合并。
- 结论：`AppStateService` 有稳定 application 语义，应保留；但它不应继续暴露完整快照透传 API。

## 风险与后续事项

- `AppStateService` 当前仍有两类语义：
  - entries workspace
  - last-opened feed
- 这两类都属于当前产品骨架内的稳定 app-state slice，短期内不需要再拆。
- 下一步若继续 application naming/boundary 收敛，更值得转向 `EntryService`：它现在仍是典型 repository façade，需要判断是否应该继续存在，还是完全被 `EntriesListService` / `ReaderService` 吸收。

## 给下一位 Agent 的备注

- 如果继续审 application 空包装/边界，优先看 `crates/rssr-application/src/entry_service.rs`。
- `AppStateService` 这一步之后，不建议再把完整 `AppStateSnapshot` 公开回 application use case 外层。

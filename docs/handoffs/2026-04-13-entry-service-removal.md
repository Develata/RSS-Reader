# Entry Service Removal

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：0a210e1
- 相关 commit：0a210e1
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

删除 `rssr-application::EntryService`，把 `EntriesListService` 与 `ReaderService` 直接对齐到 `EntryRepository`，去掉一个纯 repository façade。

## 影响范围

- 模块：
  - `crates/rssr-application/src/entries_list_service.rs`
  - `crates/rssr-application/src/reader_service.rs`
  - `crates/rssr-application/src/composition.rs`
  - `crates/rssr-application/src/lib.rs`
  - `crates/rssr-application/src/entry_service.rs`
- 平台：
  - desktop
  - Android
  - Web
  - CLI
- 额外影响：
  - application service 边界
  - entry query/reader 用例 wiring

## 关键变更

### Remove Repository Façade

- 删除 `crates/rssr-application/src/entry_service.rs`。
- `EntriesListService` 改为直接持有 `Arc<dyn EntryRepository>`。
- `ReaderService` 改为直接持有 `Arc<dyn EntryRepository>`。
- `AppUseCases` 不再暴露 `entry_service`。

### Composition Simplification

- `AppUseCases::compose()` 不再构造中间 `EntryService`。
- `entries_list_service` 和 `reader_service` 直接接收 `entry_repository`。

### Test Wiring Cleanup

- `EntriesListService` 与 `ReaderService` 测试直接传入 repository stub，不再额外包一层 `EntryService`。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo test -p rssr-application`：通过
- `cargo test -p rssr-app`：通过
- `cargo check --workspace`：通过
- `git diff --check`：通过

### 手工验收

- 未执行；本次改动为 application façade 清理，依赖 application/app 自动化验证。

## 结果

- 本次变更已验证，可合并。
- 结论：`EntryService` 与前面删除的 `ShellService`、`SettingsPageService` 同类，缺乏独立业务语义，保留只会增加一层误导性的抽象。

## 风险与后续事项

- `ReaderService` 与 `EntriesListService` 现在都直接依赖 `EntryRepository`，这是更干净的结构，但也意味着后续若要引入 entry 级共享规则，需要明确落在哪个 use case，而不是再回头恢复一个泛化 `EntryService`。
- 下一步若继续做 application 收敛，更值得审查的是 `SettingsSyncService`：它很薄，但至少承载了 remote pull 后重新加载 settings 的语义，需要判断这个语义是否已经足够独立。

## 给下一位 Agent 的备注

- 如果继续沿“删除纯 façade”推进，下一站优先看 `crates/rssr-application/src/settings_sync_service.rs`，其次再看是否需要给现有 query/command use case 做更一致的命名。
- 不建议再恢复一个通用 `EntryService`；entry 路径现在已经按 reader/list 两个稳定 use case 分开。

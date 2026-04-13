# Settings Page Service Removal

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：fec4462
- 相关 commit：fec4462
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

删除 `rssr-application::SettingsPageService`，把 settings 页面相关调用直接对齐到 `SettingsService` 与 `SettingsSyncService`，去掉一个没有独立 application 语义的 page-shaped 空包装层。

## 影响范围

- 模块：
  - `crates/rssr-application/src/composition.rs`
  - `crates/rssr-application/src/lib.rs`
  - `crates/rssr-app/src/ui/runtime/services.rs`
  - `docs/design/application-use-case-consolidation-plan.md`
- 平台：
  - desktop
  - Android
  - Web
- 额外影响：
  - application service 命名基线
  - settings runtime 调用路径

## 关键变更

### Remove Empty Page Wrapper

- 删除 `crates/rssr-application/src/settings_page_service.rs`。
- `AppUseCases` 不再暴露 `settings_page_service`。
- settings 页面相关 runtime 调用改为直接使用：
  - `settings_service.load()`
  - `settings_service.save()`
  - `settings_sync_service.apply_remote_pull(...)`

### Naming Baseline Sync

- 更新 `application-use-case-consolidation-plan.md`，移除已删除的 `SettingsPageService` 分类。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo test -p rssr-application`：通过
- `cargo test -p rssr-app`：通过
- `cargo check --workspace`：通过
- `git diff --check`：通过

### 手工验收

- 未执行；本次改动为 application 空包装清理，依赖 application/app 自动化验证。

## 结果

- 本次变更已验证，可合并。
- 结论：`SettingsPageService` 与 `ShellService` 属于同类问题，都不具备稳定 application 语义，删除比继续维持 page 命名更干净。

## 风险与后续事项

- 现在 settings 相关 application 服务剩余：
  - `SettingsService`
  - `SettingsSyncService`
- 这组边界已经比原来更直接，但后续如果 settings 读写流程继续增长，仍要警惕再长出新的 page-shaped façade。
- 下一步更值得审查的是 `AppStateService`：它现在既有原子读写，也有 entries workspace / last-opened feed convenience 方法，需要判断这些 helper 是否仍然是稳定 application 语义。

## 给下一位 Agent 的备注

- 若继续命名与边界收敛，优先看 `crates/rssr-application/src/app_state_service.rs`。
- 对 settings 路径，不要再因为 UI 页面名字方便就重新引入 page-shaped application façade。

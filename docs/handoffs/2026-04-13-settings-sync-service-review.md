# Settings Sync Service Review

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：f75cc87
- 相关 commit：f75cc87
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

审查 `SettingsSyncService` 是否属于应删除的 application 空包装，结论是保留，并把这个判断补入 application use case 收敛计划。

## 影响范围

- 模块：
  - `docs/design/application-use-case-consolidation-plan.md`
- 平台：
  - desktop
  - Android
  - Web
  - CLI
- 额外影响：
  - application 命名与边界基线

## 关键结论

### 保留理由

- `SettingsSyncService` 虽然很薄，但它不是 generic repository façade。
- 它承载的语义是：
  - remote config pull 已经完成
  - 判断结果是 `NotFound` 还是 `Imported`
  - 若已导入，重新读取当前有效 settings
  - 生成给 host/UI 使用的稳定 outcome

### 不保留会发生什么

- 若删除它，这个语义只能回流到：
  - UI/runtime
  - CLI
  - 或 `ImportExportService`
- 前两者会重新把应用语义抬回壳层；后者会把“导入配置”和“导入后重载 settings 展示”混成一层。

### 与已删除 façade 的区别

- `ShellService` 只是 `SettingsService::load()` 的重命名包装。
- `SettingsPageService` 只是 `SettingsService + SettingsSyncService` 的 page-shaped 包装。
- `EntryService` 只是 `EntryRepository` 的方法透传。
- `SettingsSyncService` 则定义了一个稳定的 remote-pull 应用结果，因此保留更合理。

## 验证与验收

### 自动化验证

- `git diff --check`：通过

### 手工验收

- 代码与调用链审查：
  - `crates/rssr-application/src/settings_sync_service.rs`
  - `crates/rssr-app/src/ui/runtime/services.rs`
  - `crates/rssr-cli/src/main.rs`
  - `docs/design/application-use-case-consolidation-plan.md`

## 结果

- 本次审查已完成，可合并。
- 结论：`SettingsSyncService` 保留，并作为 application 收敛计划中的显式例外说明。

## 风险与后续事项

- 保留它的前提是它继续只承载“remote pull applied outcome”语义；如果后续开始混入 endpoint/path 选择、远端 store 构造或页面展示策略，就需要重新审查。
- 下一步更值得继续看的不是 `SettingsSyncService`，而是 `RefreshService` / `SubscriptionWorkflow` 是否需要进一步统一命名规则说明。

## 给下一位 Agent 的备注

- 若继续 application naming baseline 审查，`SettingsSyncService` 现在默认视为保留项，不要因为它“很薄”就机械删除。
- 继续推进时，优先关注是否还有 generic façade，而不是带稳定 outcome 语义的窄 use case。

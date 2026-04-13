# Import Export Service Review

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：3e4ade4
- 相关 commit：3e4ade4
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

审查 `ImportExportService` 是否已经跨出单一 capability service 的边界，结论是暂时保留为 `Service`，并把拆分触发条件补入 application use case 收敛计划。

## 影响范围

- 模块：
  - `docs/design/application-use-case-consolidation-plan.md`
- 平台：
  - desktop
  - Android
  - Web
  - CLI
- 额外影响：
  - application workflow/service 分类基线
  - config exchange 边界说明

## 关键结论

### 保留为 Service 的理由

- 当前 `ImportExportService` 覆盖的动作虽然多，但仍属于同一 capability family：
  - config package export/import
  - OPML export/import
  - remote config push/pull
- 这些动作共享：
  - feed/settings truth sources
  - 配置交换规则
  - 导入后清理缺失 feed 的语义
  - config exchange outcome 模型

### 为什么现在不拆 Workflow

- 它还没有像 `SubscriptionWorkflow` 那样跨越多个不相干的产品动作。
- 当前 remote push/pull 仍然只是 config exchange 的 transport 入口，不包含额外 host lifecycle 编排。
- 若现在为“步骤多”而拆 workflow，只会制造更多中间层和命名噪音。

### 未来拆分的触发条件

只有当下列情况出现时，才值得把它拆成更窄 service 或 workflow：

- 某一路径开始引入明显不同的生命周期管理
- 某一路径需要额外的 side-effect coordination，已不再属于统一配置交换边界
- 远端同步开始带入独立的冲突处理、回滚、异步队列或阶段性状态推进

## 验证与验收

### 自动化验证

- `git diff --check`：通过

### 手工验收

- 代码与调用链审查：
  - `crates/rssr-application/src/import_export_service.rs`
  - `crates/rssr-application/src/import_export_service/rules.rs`
  - `crates/rssr-cli/src/main.rs`
  - `crates/rssr-app/src/ui/runtime/services.rs`
  - `docs/design/application-use-case-consolidation-plan.md`

## 结果

- 本次审查已完成，可合并。
- 结论：`ImportExportService` 当前仍应视为单一 config-exchange capability service，而不是 workflow。

## 风险与后续事项

- 它是 application 层里最容易继续膨胀的 service 之一，后续必须盯住是否开始混入远端同步生命周期和更复杂的副作用协调。
- 下一步更值得继续的不是立即拆它，而是把“哪些 service 已删除、哪些保留”的判定回写到总览文档，减少重复审查成本。

## 给下一位 Agent 的备注

- 若继续 application 边界审查，`ImportExportService` 目前默认视为保留项。
- 真正下一步更适合转向文档总览或 repo 级清单更新，而不是再继续局部点杀 service 名字。

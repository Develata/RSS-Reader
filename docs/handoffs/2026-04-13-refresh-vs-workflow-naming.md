# Refresh vs Workflow Naming

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：1eff490
- 相关 commit：1eff490
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

把 `RefreshService` 与 `SubscriptionWorkflow` 的命名边界补入 application use case 收敛计划，明确为什么一个保留 `Service`、另一个保留 `Workflow`。

## 影响范围

- 模块：
  - `docs/design/application-use-case-consolidation-plan.md`
- 平台：
  - desktop
  - Android
  - Web
  - CLI
- 额外影响：
  - application 命名基线
  - workflow/service 分类规则

## 关键结论

### SubscriptionWorkflow 保留为 Workflow

- 它跨越多个应用动作与状态效果：
  - feed subscription mutation
  - optional first refresh
  - remove 后清理 last-opened-feed app state
- 这已经超出单一 capability service 的边界，保留 `Workflow` 更准确。

### RefreshService 保留为 Service

- 它虽然内部有多步执行，但这些步骤都属于同一 capability family：
  - resolve refresh target
  - call refresh source/store ports
  - produce stable refresh outcomes
- 它不是在跨越多个不相干的产品动作做流程编排，因此不应因为“内部步骤多”就升级成 workflow。

### 规则补充

- 不是所有多步实现都该叫 workflow。
- 只有当一个 application unit 真正在编排多个不同 use case / side effect 并形成业务分支时，才应提升为 workflow。

## 验证与验收

### 自动化验证

- `git diff --check`：通过

### 手工验收

- 阅读校对：
  - `crates/rssr-application/src/refresh_service.rs`
  - `crates/rssr-application/src/subscription_workflow.rs`
  - `docs/design/application-use-case-consolidation-plan.md`

## 结果

- 本次文档变更已完成，可合并。
- 后续若再出现 “这个 service 步骤很多，要不要改叫 workflow” 的争议，可以直接按该基线判断。

## 风险与后续事项

- 这一步只定义了命名与分类规则，不会自动消除所有历史命名张力。
- 下一步更值得继续的是审查 `ImportExportService` 是否仍属于单一 capability service，还是未来会长成需要拆 workflow 的对象。

## 给下一位 Agent 的备注

- 若继续 application naming baseline 审查，优先看 `crates/rssr-application/src/import_export_service.rs`。
- 判断标准应继续沿用这次补入的规则：多步不等于 workflow，跨 use case/side effect 的业务编排才是 workflow。

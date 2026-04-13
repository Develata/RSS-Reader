# Application Naming Baseline

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：c1abd7b
- 相关 commit：c1abd7b
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

更新 `application-use-case-consolidation-plan`，把当前已经落地的 application use case 收敛状态、query/command/workflow 边界规则和服务命名基线写成正式文档，作为后续收敛的约束入口。

## 影响范围

- 模块：
  - `docs/design/application-use-case-consolidation-plan.md`
- 平台：
  - desktop
  - Android
  - Web
  - CLI
- 额外影响：
  - application 架构约束
  - 后续命名与边界收敛

## 关键变更

### Current Status 补齐

- 文档不再把 subscription workflow 当成“第一步计划”。
- 明确记录当前主线已经完成的 application 收敛：
  - `SubscriptionWorkflow`
  - `RefreshService` outcome summary
  - `ImportExportService` outcome consolidation
  - `EntriesWorkspaceService`
  - `FeedsSnapshotService`
  - `FeedCatalogService`
  - `FeedService` summaries query 剥离

### Boundary Rules

- 明确写入：
  - query use case 可以直接依赖 repository
  - query 不应借 command service 做透传
  - command service 不应继续长出小查询 helper
  - workflow 可以编排多个 use case，但 host 不应重建同一套业务分支

### Naming Baseline

- 明确当前项目不做立刻的 workspace 级 mass rename。
- 固化命名规则：
  - `*Workflow` 用于多步业务动作
  - `*Service` 继续作为 application use case 命名
  - query-oriented service 用返回视图/表面命名
  - command-oriented service 用被作用的领域对象命名
- 补充当前分类基线，作为后续改动的审查参照。

## 验证与验收

### 自动化验证

- `git diff --check`：通过

### 手工验收

- 阅读校对 `docs/design/application-use-case-consolidation-plan.md`：通过

## 结果

- 本次文档变更已完成，可合并。
- 后续若继续做 application use case 收敛，可以直接以该文档为命名与边界判断入口，而不是临时口头约定。

## 风险与后续事项

- 这一步只冻结规则，不自动消除现有名称里所有历史包袱。
- 下一步更值得继续的是审查：
  - `SettingsPageService`
  - `ShellService`
  - `StartupService`
  是否仍有“页面方便命名”和“稳定 application 语义”之间的偏差。

## 给下一位 Agent 的备注

- 继续 application 层审查时，先看
  `docs/design/application-use-case-consolidation-plan.md`。
- 若要重命名 service，必须先证明该名字已经持续导致边界误用；不要为了形式对称做机械 churn。

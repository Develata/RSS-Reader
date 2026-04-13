# Startup Query Narrowing

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：d7eaaa2
- 相关 commit：d7eaaa2
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

继续 application use case 收敛审查，收窄启动恢复 last-opened feed 的存在性查询，并记录为什么本轮不做宽泛 service-to-repository 重写。

## 影响范围

- 模块：
  - `crates/rssr-application/src/startup_service.rs`
  - `docs/design/application-use-case-consolidation-plan.md`
- 平台：
  - Linux
  - desktop / Android / Web 共用 application 层
- 额外影响：
  - docs

## 关键变更

### StartupService

- `resolve_startup_target()` 使用 `FeedRepository::get_feed(feed_id)` 判断 last-opened feed 是否仍存在。
- 不再通过 `FeedRepository::list_summaries()` 拉取全量 feed summary 后做本地 `any()`。
- 测试桩同步实现 `get_feed()`，保持 existing / missing feed 两条路径的覆盖。

### Application use case consolidation plan

- 记录 `Startup Query Narrowing 2026-04-13` 审查结论。
- 明确 `AppStateService` 继续保留局部 snapshot 读改写语义；直接在调用方使用 repository 会复制状态切片规则。
- 明确 `EntriesWorkspaceService` 保留 feed summaries 查询，因为其 bootstrap 输出本身可能需要返回 feed list。
- 明确 `SettingsSyncService` 虽薄但仍表达 remote pull/import 后重新暴露有效 settings 的应用语义。

## 验证与验收

### 自动化验证

- `cargo fmt --check`：通过
- `cargo test -p rssr-application startup_service`：通过，3 passed
- `cargo test -p rssr-application`：通过，43 passed
- `git diff --check`：通过

### 手工验收

- `crates/rssr-application/src/startup_service.rs`：已复核，启动恢复只需要单个 feed 存在性判断。
- `docs/design/application-use-case-consolidation-plan.md`：已复核，记录本轮边界判断和不做宽泛拆分的理由。

## 结果

- 本次交付可合并。
- 启动恢复路径减少一次不必要的 feed summary 聚合查询；对外行为不变。

## 风险与后续事项

- 本次没有执行 `cargo check --workspace` 或 UI/browser smoke；改动仅限 application 层启动查询与文档。
- 后续如果 `FeedRepository::get_feed()` 语义变化，需继续保持“已删除 feed 不视为存在”的约束。

## 给下一位 Agent 的备注

- 继续 application use case 收敛时，先看 `docs/design/application-use-case-consolidation-plan.md` 的 `Startup Query Narrowing 2026-04-13`。
- 不要把所有 service-to-service 调用都机械拆成 repository 直连；优先判断该 service 是否承载稳定状态切片或 outcome 语义。

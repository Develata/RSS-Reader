# Application Boundary Checklist

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：fe36659
- 相关 commit：fe36659
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

完成一轮 application use case 主线验证，审查 `ImportExportService` 是否需要拆分，并新增可复用的 application 边界检查清单。

## 影响范围

- 模块：
  - `docs/design/application-use-case-boundary-checklist.md`
  - `docs/design/application-use-case-consolidation-plan.md`
  - `docs/design/README.md`
  - `crates/rssr-application/src/import_export_service.rs`
  - `crates/rssr-application/src/import_export_service/tests.rs`
- 平台：
  - Linux
  - desktop / Android / Web 共用 application 层
  - CLI
- 额外影响：
  - docs

## 关键变更

### 主线验证

- 执行 application use case 收敛后的主线 Rust 验证。
- 本轮未改生产代码；验证用于确认前序 application / runtime / infra 变更仍保持基线健康。

### ImportExportService boundary check

- 复核 `ImportExportService` 当前职责：
  - JSON config export/import
  - OPML import/export
  - remote config push/pull
  - config import 导致的 removed feed cleanup
- 结论：暂不拆分。当前这些分支仍属于同一 config-exchange 能力族；remote push/pull 只负责传输同一 config package payload，OPML 是同一 feed membership 数据的互操作视图。
- 记录未来拆分触发条件：continuous sync、conflict resolution、account identity、background scheduling、独立 OPML subscription management surface。

### Boundary checklist

- 新增 `docs/design/application-use-case-boundary-checklist.md`。
- 覆盖：
  - skeleton first
  - runtime entry
  - query / command boundary
  - workflow versus service
  - thin service review
  - `ImportExportService` review
  - verification gate
- 更新 `docs/design/README.md`，把 checklist 加入设计文档索引。

## 验证与验收

### 自动化验证

- `cargo fmt --check`：通过
- `cargo test -p rssr-application`：通过，43 passed
- `cargo test -p rssr-app`：通过，30 passed
- `cargo test -p rssr-infra`：通过
- `cargo test -p rssr-cli`：通过，0 tests
- `cargo check --workspace`：通过
- `git diff --check`：通过

### 手工验收

- `crates/rssr-application/src/import_export_service.rs`：已复核，未发现超出 config-exchange 能力族的当前职责。
- `crates/rssr-application/src/import_export_service/tests.rs`：已复核，已有 JSON / OPML / remote config / removed feed cleanup 行为覆盖。
- `crates/rssr-app/src/ui/runtime/services.rs` 与 `crates/rssr-cli/src/main.rs`：已复核，调用方仍经 `AppUseCases` 进入 `ImportExportService`。

## 结果

- 本次交付可合并。
- 新增 checklist 可作为后续 application service、runtime port、CLI command、host capability 修改前的审查入口。

## 风险与后续事项

- 本轮未跑 browser smoke，因为没有浏览器可见行为或 refresh 行为变更。
- 后续如果 remote config 从手动交换演化成连续同步，必须先做骨架级审查，不应直接扩张 `ImportExportService`。

## 给下一位 Agent 的备注

- 继续 application use case 收敛时，先看 `docs/design/application-use-case-boundary-checklist.md`。
- `ImportExportService` 当前不拆；只有在 config-exchange 之外长出新生命周期或新系统身份时再拆。

# Runtime Boundary Check

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：77bc838
- 相关 commit：77bc838
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

复核 application use case 收敛后的 runtime 边界，确认 UI runtime 仍通过 `AppUseCases` 进入应用层，没有重新引入已删除的纯 facade service 或 repository 直连。

## 影响范围

- 模块：
  - `docs/design/application-use-case-consolidation-plan.md`
  - `crates/rssr-application/src/composition.rs`
  - `crates/rssr-app/src/ui/runtime/services.rs`
- 平台：
  - Linux
  - desktop / Android / Web runtime 边界文档说明
- 额外影响：
  - docs

## 关键变更

### Application use case consolidation plan

- 记录已删除的纯 facade service：`ShellService`、`SettingsPageService`、`EntryService`。
- 新增 2026-04-13 runtime boundary check：
  - `AppUseCases::compose()` 仍是应用层组合入口。
  - UI runtime ports 仍调用 `AppUseCases`，不直接持有 repository。
  - host capabilities 仍限定在宿主行为：auto-refresh lifecycle、refresh capability、remote config transport、clipboard access。
  - 当前命名与前序 classification baseline 一致。

## 验证与验收

### 自动化验证

- `git diff --check`：通过

### 手工验收

- `crates/rssr-application/src/composition.rs`：已复核，未发现已删除 facade service 回流到组合入口。
- `crates/rssr-app/src/ui/runtime/services.rs`：已复核，runtime ports 未直接依赖 repository。
- `rg` 检查 `ShellService` / `SettingsPageService` / `EntryService`：已执行，未发现生产路径重新引入这些 service。

## 结果

- 本次交付为文档化边界审查，不包含生产代码变更。
- 当前结论：不需要为 runtime 边界立即改代码；下一步 application 层工作应继续由真实边界压力驱动，而不是批量重命名。

## 风险与后续事项

- 本次只复核当前 runtime composition 与服务入口，没有重新跑完整 workspace 测试。
- 后续新增 UI runtime port 或 host capability 时，应继续检查是否绕过 `AppUseCases` 或把应用层编排下沉到 UI runtime。

## 给下一位 Agent 的备注

- 先看 `docs/design/application-use-case-consolidation-plan.md` 的 `Runtime Boundary Check 2026-04-13`。
- 如果继续 application use case 收敛，优先从真实跨边界压力入手，不要只为命名一致性移动代码。

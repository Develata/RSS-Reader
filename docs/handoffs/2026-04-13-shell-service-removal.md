# Shell Service Removal

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：357164e
- 相关 commit：357164e
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

删除 `rssr-application::ShellService`，把 shell 初始化读取设置的路径直接对齐到 `SettingsService`，去掉一个没有独立业务语义的空包装层。

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
  - application service 边界
  - runtime shell 初始化路径

## 关键变更

### Remove Empty Wrapper

- 删除 `crates/rssr-application/src/shell_service.rs`。
- `AppUseCases` 不再暴露 `shell_service`。
- shell 初始化读取设置改为直接调用 `settings_service.load()`。

### Naming Baseline Sync

- 更新 `application-use-case-consolidation-plan.md` 的分类基线，移除已删除的 `ShellService`。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo test -p rssr-application`：通过
- `cargo test -p rssr-app`：通过
- `cargo check --workspace`：通过
- `git diff --check`：通过

### 手工验收

- 未执行；本次改动是 application 空包装清理，依赖 application/app 自动化验证。

## 结果

- 本次变更已验证，可合并。
- 结论：`ShellService` 不具备稳定 application 语义，删除比保留并等待未来用途更干净。

## 风险与后续事项

- `SettingsPageService` 仍然是 page-shaped service，但它当前至少承载了 remote pull 应用结果和 settings 页面快照，比 `ShellService` 更有独立语义。
- 下一步更值得继续审查的是：`SettingsPageService` 是否仍应保持 page 命名，还是应在未来收敛成更中性的 settings query/command 组合。

## 给下一位 Agent 的备注

- 如果继续沿 application naming baseline 推进，优先看 `crates/rssr-application/src/settings_page_service.rs`。
- `StartupService` 当前有明确“解析启动目标”语义，短期内不建议为形式对称改名。

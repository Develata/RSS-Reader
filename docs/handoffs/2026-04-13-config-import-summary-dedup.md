# Config Import Summary Dedup

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：7ec485a
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

继续推进 P3 文案与结果汇总重复清理，把 `config_import_summary(...)` 三处重复实现收敛到 `ConfigImportOutcome::summary_line()`，降低 UI 与 CLI 导入结果文案漂移风险。

## 影响范围

- 模块：
  - `crates/rssr-application/src/import_export_service.rs`
  - `crates/rssr-application/src/import_export_service/tests.rs`
  - `crates/rssr-app/src/ui/runtime/feeds.rs`
  - `crates/rssr-app/src/ui/runtime/settings.rs`
  - `crates/rssr-cli/src/main.rs`
- 平台：
  - Linux
  - Desktop / native
  - Web / wasm32
  - CLI
- 额外影响：
  - config import status text
  - CLI config import output

## 关键变更

### Summary Helper

- 新增 `ConfigImportOutcome::summary_line()`。
- 保留 UI / CLI 外层上下文文案，例如：
  - `配置包已导入：...`
  - `已从 WebDAV 下载并导入配置：...`
  - `配置已导入：...`
- 删除这些重复 helper：
  - `rssr-app/src/ui/runtime/feeds.rs::config_import_summary`
  - `rssr-app/src/ui/runtime/settings.rs::config_import_summary`
  - `rssr-cli/src/main.rs::config_import_summary`

### Tests

- 新增 `config_import_outcome_summary_line_formats_settings_state`，覆盖设置已更新 / 未变化两种 summary 文案。

## 验证与验收

### 自动化验证

- `cargo test -p rssr-application`：通过，44 passed
- `cargo test -p rssr-app`：通过，29 unit tests + 2 theme contract tests passed
- `cargo test -p rssr-cli`：通过，0 tests
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo fmt --check`：通过
- `git diff --check`：通过

### 手工验收

- `rg 'fn config_import_summary|config_import_summary\\(' crates/rssr-app crates/rssr-cli crates/rssr-application -g '*.rs'`：通过，无旧 helper / 调用残留
- 静态代码复核：通过，只有公共结果片段下沉到 outcome，页面和 CLI 的上下文前缀仍在 presentation 层

## 结果

- config import 的结果汇总文案现在只有一个实现点。
- UI feeds/settings runtime 与 CLI 不再各自维护同一段导入/清理/设置更新摘要。

## 风险与后续事项

- `summary_line()` 是用户可见中文片段，后续如果做 i18n，应从 outcome 方法迁出到 locale-aware presentation 层。
- 目前 P0/P1/P2/P3 代码债务主线已基本收口；剩余发布矩阵项主要是手工视觉/真实环境 smoke。

## 给下一位 Agent 的备注

- 入口文件：
  - `crates/rssr-application/src/import_export_service.rs`
  - `crates/rssr-app/src/ui/runtime/feeds.rs`
  - `crates/rssr-app/src/ui/runtime/settings.rs`
  - `crates/rssr-cli/src/main.rs`
- 当前同一 worktree 里还存在未提交的 auth、fetch response classification 与 app-state cleanup port unification 增量。

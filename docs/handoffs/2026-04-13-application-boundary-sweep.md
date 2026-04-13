# Application Boundary Sweep

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：042ccc8
- 相关 commit：042ccc8
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

按新 application boundary checklist 做一轮现有代码审查，确认 UI/runtime/CLI 入口和 application 查询面没有新的真实改代码压力，并补充 native image localization host worker 的边界例外说明。

## 影响范围

- 模块：
  - `docs/design/application-use-case-boundary-checklist.md`
  - `docs/design/application-use-case-consolidation-plan.md`
  - `crates/rssr-app/src/bootstrap/native.rs`
  - `crates/rssr-app/src/bootstrap/web.rs`
  - `crates/rssr-cli/src/main.rs`
  - `crates/rssr-application/src`
- 平台：
  - Linux
  - desktop / Android / Web 共用 application 层
  - CLI
- 额外影响：
  - docs

## 关键变更

### Boundary checklist sweep

- 使用 `rg` 检查 `crates/rssr-app/src` 和 `crates/rssr-cli/src` 中 repository 类型引用。
- 使用 `rg` 检查 `crates/rssr-application/src` 中 `list_feeds`、`list_summaries`、`get_feed`、`list_entries`、`get_entry` 调用面。
- 结论：
  - UI runtime 和 CLI command handler 仍通过 `AppUseCases` 进入 application 层。
  - native/web/CLI 中 repository 构造集中在 bootstrap composition wiring。
  - application 查询方法与当前输出面匹配，没有发现需要继续收窄的查询调用。

### Native image localization exception

- 复核 `crates/rssr-app/src/bootstrap/native.rs` 中 `ImageLocalizationWorker` 直接持有 `SqliteEntryRepository` 的原因。
- 结论：这是 native host worker 的 hash-checked background HTML localization writeback，符合既有设计中“native image localization remains a host worker”的边界。
- 已在 checklist 中补充该例外，同时明确不能把它泛化为 UI runtime repository access。

## 验证与验收

### 自动化验证

- `git diff --check`：通过

### 手工验收

- `crates/rssr-app/src/bootstrap/native.rs`：已复核，具体 repository 持有点只用于图片本地化 worker。
- `crates/rssr-app/src/bootstrap/web.rs`：已复核，repository 构造只在 web composition 初始化中出现。
- `crates/rssr-cli/src/main.rs`：已复核，repository 构造只在 CLI services composition 初始化中出现，命令行为经 `AppUseCases`。
- `crates/rssr-application/src` 查询调用面：已复核，当前 `list_*` / `get_*` 使用符合 checklist。

## 结果

- 本次交付为边界审查与文档修订，不包含生产代码变更。
- 当前不建议继续机械性拆 service 或替换 repository 调用；下一步应转向更具体的风险点或新的功能需求。

## 风险与后续事项

- 本轮未重复运行完整 cargo 测试；前一轮已通过 `cargo fmt --check`、`cargo test -p rssr-application`、`cargo test -p rssr-app`、`cargo test -p rssr-infra`、`cargo test -p rssr-cli`、`cargo check --workspace`。
- 后续如果图片本地化 worker 扩张为通用后台内容处理，需要重新做 host/application 边界审查。

## 给下一位 Agent 的备注

- 如果继续做 application use case 收敛，不要把 `ImageLocalizationWorker` 例外复制到 UI runtime。
- 优先寻找真实边界压力，例如新的跨端用例、状态迁移、配置包结构变化或 browser-visible refresh 行为变更。

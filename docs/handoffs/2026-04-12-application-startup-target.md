# Application Startup Target 收敛

- 日期：2026-04-12
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：fa6ec86
- 相关 commit：fa6ec86 / handoff pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

继续 application use case 收敛后的收尾审查，将 shell runtime 中“启动到全部文章还是上次打开订阅”的状态判断收进 `rssr-application`，UI 只负责把 application target 映射成 router route。

## 影响范围

- 模块：
  - `crates/rssr-application/src/startup_service.rs`
  - `crates/rssr-application/src/composition.rs`
  - `crates/rssr-application/src/lib.rs`
  - `crates/rssr-app/src/ui/runtime/shell.rs`
  - `crates/rssr-app/src/ui/runtime/services.rs`
- 平台：
  - Web
  - Desktop / Android shared app runtime
  - CLI build surface
- 额外影响：
  - release UI regression / handoff docs

## 关键变更

### Startup Service

- 新增 `StartupService` 与 `StartupTarget`。
- `StartupService::resolve_startup_target()` 统一读取：
  - durable settings 中的 `startup_view`
  - app-state 中的 `last_opened_feed_id`
  - 当前 feed summary 列表
- 当 `startup_view = all` 时返回 `StartupTarget::AllEntries`。
- 当 `startup_view = last_feed` 且 last feed 仍存在时返回 `StartupTarget::FeedEntries { feed_id }`。
- 当 last feed 缺失、app-state 读取失败或 feed 列表读取失败时保持既有保守行为，回退到 `AllEntries`。
- application 不依赖 `AppRoute`，避免把 router 类型下沉到应用层。

### Shell Runtime

- `ResolveStartupRoute` 不再直接组合 settings / last-opened feed / feed list。
- Shell runtime 现在只调用 `ShellPort::resolve_startup_target()`，再把 `StartupTarget` 映射为 `AppRoute`。
- `ShellPort` 删除不再需要的 `load_last_opened_feed_id()` 与 `list_feeds()`。

## 验证与验收

### 自动化验证

- `cargo fmt --check`：通过。
- `git diff --check`：通过。
- `cargo test -p rssr-application`：通过，25 tests。
- `cargo check -p rssr-app`：通过。
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过。
- `cargo check -p rssr-cli`：通过。
- `cargo test --workspace`：通过；包含 `test_webdav_local_roundtrip`。
- `bash scripts/run_release_ui_regression.sh --no-serve --with-rssr-web --port 8347 --web-port 18847 --log-dir target/release-ui-regression/20260412-codex-startup-target`：通过；重新构建 debug web bundle，自动化门禁、`rssr-web` HTTP smoke、`rssr-web browser feed smoke` 均通过。

### 手工验收

- 未执行。该轮改动由 application 单测、workspace 测试和 release UI smoke 覆盖。

## 结果

- 本次改动可合并。
- startup route 的核心状态判断已经归入 application use case。
- UI shell 仍保留正确边界：负责启动 shell、自动刷新 capability 启动、用户可见错误和 route 映射。

## 风险与后续事项

- `EntriesPort` 仍在 runtime 中组合 settings、entries workspace、feed list 和 entry list，用于页面 bootstrap；这是下一处可评估的 query/snapshot use case。
- `ReaderCommand::LoadEntry` 仍在 runtime 中组合 entry body selection、navigation 和页面展示模型；其中 body selection/formatting 当前更偏 UI 呈现，不建议直接下沉到 application。
- push 预计仍会被当前 GitHub HTTPS 凭据缺失阻止，除非用户配置凭据或切换 remote。

## 给下一位 Agent 的备注

- 继续收敛时优先看 query/snapshot 类型的只读组合，避免把 router、页面 intent、文案或 HTML 呈现策略下沉。
- 若做 entries bootstrap snapshot，application 应返回 domain/application 数据结构，runtime 再转换为 `EntriesPageIntent`。

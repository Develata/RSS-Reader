# Feeds Confirmation Gates

- 日期：2026-04-12
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：c629dd1
- 相关 commit：c629dd1
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

把 feeds 页删除订阅和配置导入的二次确认门禁从 runtime command 执行层收回 reducer，减少页面状态决策与 service 执行路径的交错。

## 影响范围

- 模块：
  - `crates/rssr-app/src/pages/feeds_page/reducer.rs`
  - `crates/rssr-app/src/ui/commands/feeds.rs`
  - `crates/rssr-app/src/ui/runtime/feeds.rs`
- 平台：
  - Linux
  - Web
  - wasm32 / native feeds page command path
- 额外影响：
  - release UI regression 记录：`target/release-ui-regression/20260412-codex-feeds-confirmation-gates/summary.md`

## 关键变更

### Reducer Confirmation Gate

- `RemoveFeedRequested` 第一次触发只写入 `pending_delete_feed` 和提示状态，不发 `FeedsCommand`。
- `ImportConfigRequested` 第一次触发只写入 `pending_config_import` 和覆盖风险提示，不发 `FeedsCommand`。
- 第二次确认时 reducer 才发出实际执行命令。

### Runtime Command Shape

- `FeedsCommand::RemoveFeed` 移除 `confirmed` 字段，只表达已确认的删除执行请求。
- `FeedsCommand::ImportConfig` 移除 `confirmed` 字段，只表达已确认的配置导入执行请求。
- `ui/runtime/feeds.rs` 删除确认提示分支，只负责调用 service 并映射执行结果。

### Regression Coverage

- 新增 reducer 单测覆盖删除订阅与配置导入的第一次点击和确认点击。
- 测试使用模式匹配断言 command payload，没有为全局 command enum 引入 `PartialEq`。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo test -p rssr-app`：通过，18 个 app 测试通过
- `cargo test --workspace`：通过
- `git diff --check`：通过
- `bash scripts/run_release_ui_regression.sh --no-serve --with-rssr-web --port 8360 --web-port 18860 --log-dir target/release-ui-regression/20260412-codex-feeds-confirmation-gates`：通过

### 手工验收

- 未执行独立手工 UI 点击验收；本次依赖 reducer 单测、workspace 自动化测试和 release UI 门禁覆盖。

## 结果

- 本次交付可合并；删除订阅和配置导入的确认状态现在由页面 reducer 统一决定。
- `rssr-web browser feed smoke` 本轮通过，未复现超时。

## 风险与后续事项

- 这次没有改变删除订阅或配置导入的 application service 语义，只收敛 UI command 边界。
- 下一步可继续审查 feeds runtime 剩余分支，判断 export/import OPML 或 clipboard 是否需要更明确的页面 workflow 边界；优先保持 command 只表达已决策的执行请求。

## 给下一位 Agent 的备注

- 阅读入口：`crates/rssr-app/src/pages/feeds_page/reducer.rs`、`crates/rssr-app/src/ui/runtime/feeds.rs`
- 本次分层规则：页面意图是否需要二次确认属于 reducer/page state 决策；runtime 只执行已经确认的命令并把 service outcome 转成 intents。

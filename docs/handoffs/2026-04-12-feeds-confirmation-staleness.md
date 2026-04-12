# Feeds Confirmation Staleness

- 日期：2026-04-12
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：cb6371d
- 相关 commit：cb6371d
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

收紧 feeds 页危险操作确认态的生命周期，避免删除订阅或配置覆盖导入的二次确认在其它用户动作后残留。

## 影响范围

- 模块：
  - `crates/rssr-app/src/pages/feeds_page/reducer.rs`
- 平台：
  - Linux
  - Web
  - feeds page reducer / UI state path
- 额外影响：
  - release UI regression 记录：`target/release-ui-regression/20260412-codex-feeds-confirmation-staleness/summary.md`

## 关键变更

### Confirmation Lifetime

- 新增 reducer 内部 helper `clear_pending_confirmations()`，统一清理 `pending_config_import` 与 `pending_delete_feed`。
- 普通用户动作会取消已有危险操作确认态，包括输入变更、添加订阅、刷新、导出配置、OPML 导入导出、剪贴板粘贴。
- 删除订阅请求会取消配置导入确认态；配置导入请求会取消删除订阅确认态。
- `ConfigTextExported` 会清理配置导入确认态，避免导出覆盖文本后下一次导入绕过确认。

### Regression Coverage

- 新增 reducer 单测覆盖：
  - 其它用户动作清除删除确认态
  - 其它用户动作清除配置导入确认态
  - 导出配置文本结果清除配置导入确认态
  - 在删除确认与配置导入确认之间切换时清除旧确认态

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo test -p rssr-app`：通过，22 个 app 测试通过
- `cargo test --workspace`：通过
- `git diff --check`：通过
- `bash scripts/run_release_ui_regression.sh --no-serve --with-rssr-web --port 8361 --web-port 18861 --log-dir target/release-ui-regression/20260412-codex-feeds-confirmation-staleness`：通过

### 手工验收

- 未执行独立手工点击验收；本次依赖 reducer 单测、workspace 自动化测试和 release UI 门禁覆盖。

## 结果

- 本次交付可合并；危险操作确认只对下一次同类危险动作有效，不再跨其它用户动作残留。
- `rssr-web browser feed smoke` 本轮通过，未复现超时。

## 风险与后续事项

- 这次只改变页面 reducer 的确认态生命周期，不改变 application service 或 runtime command 语义。
- 后续如果新增危险动作，应复用同一确认态生命周期规则：第一次只写页面状态，第二次才发执行命令，其它用户动作清理旧确认。

## 给下一位 Agent 的备注

- 阅读入口：`crates/rssr-app/src/pages/feeds_page/reducer.rs`
- 当前页面确认态规则已经由 reducer 单测锁住；runtime 不应重新引入确认状态判断。

# Entries Page Clock Helper

- 日期：2026-04-12
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：1109c1b
- 相关 commit：1109c1b
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

将 entries 页面主组件文件中的当前时间获取逻辑移入页面局部 `clock` 模块，减少主文件里的平台条件代码。

## 影响范围

- 模块：
  - `crates/rssr-app/src/pages/entries_page/mod.rs`
  - `crates/rssr-app/src/pages/entries_page/clock.rs`
- 平台：
  - Linux
  - Web
- 额外影响：
  - UI internal organization

## 关键变更

### Entries Page Clock

- 新增 `entries_page::clock::current_time_utc`。
- `mod.rs` 改为从局部 clock 模块读取当前时间，不再直接引用 `time::OffsetDateTime` 或 `js_sys::Date`。
- wasm 与 native 的时间获取实现保持原样搬移，未改变 entries 展示逻辑。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo test -p rssr-app pages::entries_page`：通过，2 个匹配测试通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo test -p rssr-app`：通过，27 个测试通过
- `git diff --check`：通过

### 手工验收

- release UI regression：未重复执行；本次只搬移页面局部 helper，未修改组件行为。上一轮完整 release UI 已在 `target/release-ui-regression/20260412-codex-application-clock-port/summary.md` 通过。

## 结果

- 本次变更已验证，可合并。
- entries 页面主组件文件的职责更集中，平台时间细节被收敛到独立 helper。

## 风险与后续事项

- `clock.rs` 仍是页面局部 helper，不是全局时钟端口；当前选择是为了避免为了 UI 展示时间引入跨模块依赖。
- 若后续多个 UI 页面都需要同样的 wasm-safe 当前时间，再考虑提升到 `ui` 级 helper。

## 给下一位 Agent 的备注

- 入口文件是 `crates/rssr-app/src/pages/entries_page/clock.rs`。
- 此处和 application 层 `ClockPort` 不应直接合并；前者服务 UI 展示，后者服务 use case 导出时间，变化原因不同。

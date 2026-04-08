# Web browser backend 外移后回归与 wasm 导出修复

- 日期：2026-04-08
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：821b5e0
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

在 browser backend 外移到 `rssr-infra` 后做了一轮 Web 实际回归，并定位并修复了一处新的 wasm-only 回退：`ImportExportService::export_config()` 在 wasm 下直接调用 `OffsetDateTime::now_utc()`，会导致点击“导出配置 / 导出 OPML”时触发 `time not implemented on this platform` panic。

## 影响范围

- 模块：
  - `crates/rssr-application/src/import_export_service.rs`
  - `crates/rssr-application/Cargo.toml`
- 平台：
  - Web

## 关键变更

### wasm-safe 导出时间来源

- `ImportExportService::export_config()` 不再直接写死 `OffsetDateTime::now_utc()`
- 改为内部 helper：
  - wasm 下使用 `js_sys::Date::now()`
  - 其余平台继续使用 `OffsetDateTime::now_utc()`

### wasm 依赖补齐

- `crates/rssr-application/Cargo.toml`
  - 为 `wasm32` 增加 `js-sys`

## 回归结果

### Chrome MCP 实际回归

- `rssr-web` 登录：通过
- 新隔离上下文首次进入订阅页：通过
- 添加 `https://blogs.nvidia.com/feed/`：通过
- 首次刷新并生成文章：通过
- 进入文章页：通过
- 导出配置 JSON：通过
- 导出 OPML：通过

说明：
- console 中仍可看到一次旧 panic 记录，但这是修复前同一页面历史消息的 preserved log
- 修复后重新导出，页面文本框已正常写入 JSON / OPML 内容，说明当前 bundle 行为已恢复

## 验证与验收

### 自动化验证

- `cargo fmt --all`：通过
- `cargo check -p rssr-application`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo check -p rssr-app`：通过
- `git diff --check`：通过

## 对 mutations.rs / refresh.rs 的评估

### mutations.rs

- 当前只剩：
  - `set_read`
  - `set_starred`
  - `save_settings`
  - `remember_last_opened_feed_id`
- 这些都是直接面向 browser `PersistedState` 的本地状态写回
- 它们已经很薄，而且属于 Web 存储实现细节

结论：
- **暂时不值得继续外移**
- 如果后面再做，只应在 browser adapter 内部进一步吸收，而不应重新回流到 application

### refresh.rs

- 当前只剩 wasm 平台的自动刷新调度：
  - `spawn_local`
  - `gloo_timers`
  - 前台运行节奏控制
- 刷新行为本身已经走共享 `RefreshService`

结论：
- **暂时不值得继续收束**
- 它现在本质上就是 Web 生命周期胶水，留在 `rssr-app` 是合理的

## 给下一位 Agent 的备注

- 这轮之后，`mutations.rs` 和 `refresh.rs` 都已经接近合理终点
- 后续更值得继续收的是：
  - 再做一轮 Web 实际回归
  - 或继续推进 headless / command 层，而不是强行把这两个薄文件也抽走

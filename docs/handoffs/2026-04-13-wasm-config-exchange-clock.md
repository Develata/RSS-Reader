# Wasm Config Exchange Clock

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：ba046b6
- 相关 commit：ba046b6
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

修复 GitHub CI 中 `wasm_config_exchange_contract_harness` 在浏览器 wasm 测试环境调用默认 `SystemClock` 后触发 `time not implemented on this platform` 的失败。

## 影响范围

- 模块：
  - `crates/rssr-infra/tests/wasm_config_exchange_contract_harness.rs`
- 平台：
  - Web / wasm32-unknown-unknown
  - Linux CI
- 额外影响：
  - tests

## 关键变更

### Wasm config exchange harness

- 在 wasm config exchange contract harness 中新增 `FixedClock`，实现 `ClockPort` 并返回 `OffsetDateTime::UNIX_EPOCH`。
- `build_service()` 改用 `ImportExportService::new_with_feed_removal_cleanup_and_clock()`，避免测试路径使用默认 `SystemClock`。
- 在导出 JSON 测试中断言 `package.exported_at == OffsetDateTime::UNIX_EPOCH`，防止后续退回到 wasm 不可用的系统时间。

## 验证与验收

### 自动化验证

- `cargo fmt --check`：通过
- `bash scripts/run_wasm_contract_harness.sh wasm_config_exchange_contract_harness`：通过，3 passed
- `bash scripts/run_wasm_config_exchange_contract_harness.sh`：通过，3 passed
- `cargo test -p rssr-infra --test test_config_exchange_contract_harness`：通过，4 passed
- `cargo test -p rssr-infra --test test_config_package_io`：通过，3 passed
- `git diff --check`：通过

### 手工验收

- 已对照 CI 报错堆栈，失败点来自 `OffsetDateTime::now_utc()` 在 wasm32 浏览器测试环境下调用 `SystemTime::now()`。
- 已复核 Web runtime 生产路径仍由 `crates/rssr-app/src/bootstrap/web.rs` 注入 browser clock；本次只修复 harness 构造。

## 结果

- 本次交付可合并。
- 预期 GitHub CI 中 `bash scripts/run_wasm_contract_harness.sh wasm_config_exchange_contract_harness` 将恢复通过。

## 风险与后续事项

- 本次未改 `SystemClock` 的生产实现；native 路径仍使用系统当前时间。
- 后续新增 wasm harness 时，如果会调用带时间戳的 application service，应显式注入 wasm-safe 或 fixed clock。

## 给下一位 Agent 的备注

- CI 报错中的 `time not implemented on this platform` 不代表 browser adapter 读写失败，而是 harness 默认 clock 选择错误。
- 如果后续需要真实浏览器当前时间，应在 Web host 层继续使用 browser-specific clock，不要在 shared test harness 里调用 `SystemClock`。

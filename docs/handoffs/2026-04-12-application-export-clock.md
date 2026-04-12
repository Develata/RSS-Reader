# Application Export Clock Port

- 日期：2026-04-12
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：3c902b5
- 相关 commit：3c902b5
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

将配置导出的时间来源从 `rssr-application` 内部的 wasm 条件分支改为 application port 注入，移除 application crate 对 `js-sys` 的直接依赖。

## 影响范围

- 模块：
  - `crates/rssr-application/src/import_export_service.rs`
  - `crates/rssr-application/src/composition.rs`
  - `crates/rssr-app/src/bootstrap/web.rs`
  - `crates/rssr-app/src/bootstrap/native.rs`
  - `crates/rssr-cli/src/main.rs`
  - `Cargo.lock`
- 平台：
  - Linux
  - Web
  - desktop / CLI
- 额外影响：
  - application composition
  - config export timestamp

## 关键变更

### Application Clock Port

- 新增 `ClockPort` 与默认 `SystemClock`，`ImportExportService` 通过端口读取 `exported_at`。
- 新增 `ImportExportService::new_with_feed_removal_cleanup_and_clock`，让组合层可以显式传入时间来源。
- `AppCompositionInput` 增加 `clock` 字段，避免 application use case 在内部判断 wasm 环境。

### Platform Composition

- Web 组合层新增 `BrowserClock`，复用 `rssr_infra::application_adapters::browser::now_utc()`。
- native app 与 CLI 组合层传入 `SystemClock`。
- `rssr-application/Cargo.toml` 移除 wasm-only `js-sys` 依赖，`Cargo.lock` 同步去掉 `rssr-application` 的 `js-sys` 依赖边。

### Test Coverage

- `import_export_service` 测试增加固定时钟，断言导出的 `exported_at` 来自注入端口。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo test -p rssr-application import_export_service`：通过，7 个测试通过
- `cargo check --workspace`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo test --workspace`：通过
- `cargo clippy --workspace --all-targets`：通过
- `git diff --check`：通过
- `bash scripts/run_release_ui_regression.sh --no-serve --with-rssr-web --port 8365 --web-port 18865 --log-dir target/release-ui-regression/20260412-codex-application-clock-port`：通过

### 手工验收

- 未执行独立手工点击验收；本次依赖 release UI 自动门禁与 `rssr-web` browser feed smoke。

## 结果

- 本次变更已验证，可合并。
- `rssr-application` 的导出时间不再直接绑定 wasm/browser 细节，Web 导出时间仍使用浏览器可用的时间来源。
- release UI regression summary：`target/release-ui-regression/20260412-codex-application-clock-port/summary.md`

## 风险与后续事项

- release UI summary 内记录的 commit 是执行命令时的基线 `66aaddb`；命令运行时包含本次未提交 worktree，随后提交为 `3c902b5`，提交后未再次重复整轮 release UI。
- `EntriesPage` 主文件仍有一个页面级 `current_time_utc` helper，可后续移入局部 clock/browser helper，但优先级低于 application 层环境依赖清理。

## 给下一位 Agent 的备注

- 继续审查壳核边界时，优先用 `rg "js_sys|web_sys|document::eval|localStorage|rfd::" crates/rssr-application/src crates/rssr-app/src crates/rssr-infra/src -n` 定位剩余平台细节。
- 若继续清理时间来源，先区分 application use case 时间、infra 持久化时间、UI 展示当前时间，避免引入单个全局 clock 造成模块交错。

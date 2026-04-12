# Clipboard Host Capability

- 日期：2026-04-12
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：fecf407
- 相关 commit：fecf407
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

把 feeds 页粘贴订阅地址所需的系统剪贴板读取从 UI runtime 迁入 host capability，避免 runtime 直接绑定浏览器 `document::eval` 环境细节。

## 影响范围

- 模块：
  - `crates/rssr-app/src/bootstrap.rs`
  - `crates/rssr-app/src/bootstrap/native.rs`
  - `crates/rssr-app/src/bootstrap/web.rs`
  - `crates/rssr-app/src/ui/runtime/services.rs`
  - `crates/rssr-app/src/ui/runtime/feeds.rs`
- 平台：
  - Linux
  - Web
  - wasm32 / native feeds paste shortcut path
- 额外影响：
  - release UI regression 记录：`target/release-ui-regression/20260412-codex-clipboard-host-capability/summary.md`

## 关键变更

### Host Capability

- 新增 `ClipboardPort`，挂到 `HostCapabilities.clipboard`。
- Web bootstrap 通过 `dioxus::prelude::document` 和 `navigator.clipboard.readText()` 实现 `ClipboardPort::read_text()`。
- Native bootstrap 保持原行为：返回“当前平台不支持从系统剪贴板读取订阅地址”错误。

### UI Runtime

- `FeedsPort` 新增 `read_clipboard_text()`，通过 host capability 读取剪贴板。
- `ui/runtime/feeds.rs` 删除直接的 `document::eval` 和 target-specific clipboard helper。
- `FeedsCommand::ReadFeedUrlFromClipboard` 仍只映射成 `FeedUrlChanged` 或错误状态，行为保持不变。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo test -p rssr-app`：通过，22 个 app 测试通过
- `cargo test --workspace`：通过
- `git diff --check`：通过
- `bash scripts/run_release_ui_regression.sh --no-serve --with-rssr-web --port 8362 --web-port 18862 --log-dir target/release-ui-regression/20260412-codex-clipboard-host-capability`：通过

### 手工验收

- 未执行独立手工剪贴板粘贴验收；本次依赖 native/wasm 编译、workspace 自动化和 release UI 门禁覆盖。

## 结果

- 本次交付可合并；feeds runtime 不再直接依赖浏览器 API，外部环境读取通过 host capability 进入系统。
- `rssr-web browser feed smoke` 本轮通过，未复现超时。

## 风险与后续事项

- Web clipboard 权限和浏览器安全策略仍由浏览器决定；本次没有改变用户可见的授权/失败行为。
- 代码中仍有其它页面直接使用 `document::eval` 的位置，例如 entries controls，可在后续单独判断是否也应迁入 host capability。

## 给下一位 Agent 的备注

- 阅读入口：`crates/rssr-app/src/bootstrap.rs`、`crates/rssr-app/src/bootstrap/web.rs`、`crates/rssr-app/src/ui/runtime/feeds.rs`
- 当前约束：UI runtime 可以调用 `FeedsPort`，但不应直接绑定浏览器或 native 环境 API。

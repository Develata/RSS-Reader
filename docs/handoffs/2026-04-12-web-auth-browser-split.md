# Web Auth Browser Split

- 日期：2026-04-12
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：27507ee
- 相关 commit：27507ee
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

把 `web_auth.rs` 中的浏览器环境访问从认证状态机与凭据逻辑中拆出，收敛到 wasm-only helper 文件。

## 影响范围

- 模块：
  - `crates/rssr-app/src/web_auth.rs`
  - `crates/rssr-app/src/web_auth_browser.rs`
- 平台：
  - Web
  - native 路径仅受编译条件影响，行为未变
- 额外影响：
  - local web auth shell
  - browser storage / cookie / origin access

## 关键变更

### Web Auth Browser Helper

- 新增 `web_auth_browser.rs`，集中承载：
  - `localStorage` 读写
  - `sessionStorage` 读写
  - gate cookie 检测
  - 浏览器 `origin` 获取
  - 浏览器当前时间毫秒值
  - loopback host 启用判定入口

### Web Auth State Module

- `web_auth.rs` 继续保留：
  - `WebAuthState`
  - `StoredCredentials`
  - setup / login / server probe public API
  - 凭据校验与哈希逻辑
- `verify_server_gate`、`auth_state`、`generate_salt`、`load_credentials` 等调用改为走局部 browser helper，不再直接操作 `web_sys` / `JsCast`。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo test -p rssr-app`：通过，27 个测试通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `git diff --check`：通过
- `bash scripts/run_release_ui_regression.sh --no-serve --with-rssr-web --port 8367 --web-port 18867 --log-dir target/release-ui-regression/20260412-codex-web-auth-browser-split`：通过

### 手工验收

- 未执行独立手工点击验收；本次依赖 release UI 自动门禁与 `rssr-web` browser feed smoke。

## 结果

- 本次变更已验证，可合并。
- `web_auth.rs` 从“状态机 + browser API 杂糅”收敛为“状态机 / 凭据逻辑”，浏览器壳细节转入局部 helper。
- release UI regression summary：`target/release-ui-regression/20260412-codex-web-auth-browser-split/summary.md`

## 风险与后续事项

- release UI summary 记录的是执行命令时的基线 `8d7a0ba`；命令运行时包含本次未提交 worktree，随后提交为 `27507ee`，提交后未重复整轮回归。
- `web_auth.rs` 仍保留哈希与凭据编码逻辑；当前判断这是认证本体的一部分，不建议再为“文件更短”而拆成更多模块。

## 给下一位 Agent 的备注

- 如果继续清理 `web_auth`，优先考虑是否要给 `StoredCredentials` 增补更直接的测试，而不是继续拆文件。
- 继续扫描壳层残留时，`ui/shell_browser.rs`、`entries_page/browser_interactions.rs`、`theme_file_io.rs` 仍是主要浏览器细节入口，但它们当前都符合页面 / 壳层定位。

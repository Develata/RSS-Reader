# Browser Shell Helpers Cleanup

- 日期：2026-04-12
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：e3ace76
- 相关 commit：e3ace76
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

把 `rssr-app` 中两处残留在主文件里的浏览器壳层逻辑下沉到局部 helper：Web clipboard 读取和 settings 页打开仓库链接。

## 影响范围

- 模块：
  - `crates/rssr-app/src/bootstrap/web.rs`
  - `crates/rssr-app/src/bootstrap/web/clipboard.rs`
  - `crates/rssr-app/src/pages/settings_page/mod.rs`
  - `crates/rssr-app/src/pages/settings_page/session.rs`
  - `crates/rssr-app/src/pages/settings_page/browser.rs`
- 平台：
  - Web
  - desktop / native 设置页代码路径
- 额外影响：
  - UI shell organization
  - host capability plumbing

## 关键变更

### Web Clipboard Helper

- 新增 `bootstrap/web/clipboard.rs`，集中封装 `navigator.clipboard.readText()` 的 `document::eval` 调用。
- `bootstrap/web.rs` 中的 `ClipboardCapability` 改为调用局部 helper，不再内联浏览器脚本。

### Settings Browser Helper

- 新增 `pages/settings_page/browser.rs`，集中封装“打开项目 GitHub 仓库”的平台差异。
- `settings_page/session.rs` 不再直接持有 `REPOSITORY_URL` 常量与 `web_sys::window()` / `webbrowser::open(...)` 分支。
- `settings_page/mod.rs` 注册新的局部模块。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo test -p rssr-app`：通过，27 个测试通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `git diff --check`：通过
- `bash scripts/run_release_ui_regression.sh --no-serve --with-rssr-web --port 8366 --web-port 18866 --log-dir target/release-ui-regression/20260412-codex-browser-helper-cleanup`：通过

### 手工验收

- 未执行独立手工点击验收；本次依赖 release UI 自动门禁和 `rssr-web` browser feed smoke。

## 结果

- 本次变更已验证，可合并。
- `bootstrap/web.rs` 与 `settings_page/session.rs` 的职责边界更清晰，浏览器壳细节进一步收口到局部 helper。
- release UI regression summary：`target/release-ui-regression/20260412-codex-browser-helper-cleanup/summary.md`

## 风险与后续事项

- release UI summary 记录的是执行命令时的基线 `784ca92`；命令运行时包含本次未提交 worktree，随后提交为 `e3ace76`，提交后未重复整轮回归。
- 当前剩余的浏览器壳细节主要集中在 `web_auth.rs`、`ui/shell_browser.rs`、`entries_page/browser_interactions.rs` 与 `theme_file_io.rs`；其中后两者属于已经明确接受的页面 / 主题壳逻辑，不建议为“去掉 API 名称”继续上提抽象。

## 给下一位 Agent 的备注

- 若继续做壳核清理，优先审查 `web_auth.rs` 是否需要再拆成本地认证状态 helper 与 DOM helper。
- 不建议把 `settings_page/browser.rs` 和 `bootstrap/web/clipboard.rs` 合并成跨页面通用层；当前两者变化原因不同，保持局部模块更干净。

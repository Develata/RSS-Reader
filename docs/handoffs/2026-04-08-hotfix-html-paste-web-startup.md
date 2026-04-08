# hotfix html rendering paste and web startup

- 日期：2026-04-08
- 作者 / Agent：Codex
- 分支：refactor/wasm-config-exchange-extraction-step2b
- 当前 HEAD：d8a2ca0
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

修补 3 个阻断使用的问题：Web 首屏文章页自触发刷新循环导致的卡死、阅读页将 HTML-like 正文 fallback 当纯文本显示，以及桌面端“新增订阅”输入框缺少粘贴兜底。

## 影响范围

- 模块：
  - `crates/rssr-app/src/pages/entries_page.rs`
  - `crates/rssr-app/src/pages/feeds_page_sections.rs`
  - `crates/rssr-app/src/pages/reader_page.rs`
- 平台：
  - Web
  - Desktop
- 额外影响：
  - workflow
  - docs

## 关键变更

### Web 启动链路

- 将文章页的 `feeds` 加载从文章列表 `use_resource` 中拆出，避免资源既依赖 `feeds()` 又在成功分支里 `feeds.set(...)`。
- 保留现有设置加载与 auto-refresh 结构不变，优先切断首屏 `EntriesPage` 的自触发循环。

### 阅读页正文渲染

- `ReaderPage::select_reader_body` 继续优先使用 `content_html`。
- 当 `content_html` 为空时，新增对 `content_text` / `summary` 的 HTML-like fallback 识别；若内容看起来是 HTML 片段，则仍走 `sanitize_remote_html + dangerous_inner_html`。
- 新增单测覆盖“summary fallback 为 HTML 时按 HTML 渲染”的路径。

### 新增订阅输入框

- 为订阅输入框增加 `Cmd/Ctrl+V` 最小兜底。
- 命中粘贴快捷键时，通过 `document::eval("navigator.clipboard.readText()")` 读取剪贴板文本并写回 signal，避免继续依赖当前默认 paste 行为。
- 未引入新的第三方依赖。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo test -p rssr-app`：通过
- `cargo test -p rssr-app reader_treats_html_like_summary_as_html_fallback`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo clippy -p rssr-app --all-targets`：通过
- `cargo test --workspace`：通过（沙箱内仅 `test_webdav_local_roundtrip` 因本地端口绑定权限失败）
- `cargo test -p rssr-infra --test test_webdav_local_roundtrip`：通过（提权后）

### 手工验收

- Web 首屏黑屏/无响应复测：未执行
- 桌面端阅读页 HTML 显示复测：未执行
- 桌面端新增订阅粘贴复测：未执行

## 结果

- 3 个阻断问题对应的最小代码修补已落地并通过自动化回归。
- 当前可继续进入定向手测确认 UI/运行时行为。

## 风险与后续事项

- 订阅输入框粘贴兜底当前只覆盖“新增订阅”字段；如果桌面端存在更广义的系统粘贴问题，后续仍需决定是否抽成共享输入策略。
- HTML-like fallback 采用启发式识别，当前优先解决正文被原样显示问题；若未来遇到特殊文本误判，可再微调标签识别范围。
- 工作树中还保留本轮之前已有的未提交改动：`crates/rssr-infra/tests/test_config_exchange_contract_harness.rs` 与 `docs/handoffs/2026-04-08-full-test-verification.md`。

## 给下一位 Agent 的备注

- Web 首屏问题优先看 `crates/rssr-app/src/pages/entries_page.rs` 的两个 `use_resource`。
- 阅读页正文修补入口在 `crates/rssr-app/src/pages/reader_page.rs`。
- 订阅输入框粘贴兜底在 `crates/rssr-app/src/pages/feeds_page_sections.rs`。

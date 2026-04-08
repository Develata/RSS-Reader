# testing docs and reader html fallback

- 日期：2026-04-09
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：pending
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

补两类长期缺口：

- 将测试与环境限制沉淀到 `docs/testing/` 的长期文档入口
- 修复阅读页在 `content_html` 为空时，HTML-like `content_text` / `summary` 被当纯文本显示的问题

## 影响范围

- 模块：
  - `crates/rssr-app/src/pages/reader_page_support.rs`
  - `docs/testing/README.md`
  - `docs/testing/mainline-validation-matrix.md`
  - `docs/testing/environment-limitations.md`
- 平台：
  - Web
  - Desktop
- 额外影响：
  - docs
  - testing

## 关键变更

### 阅读页 HTML fallback

- `select_reader_body` 继续优先使用 `content_html`
- 当 `content_html` 为空时，新增对 `content_text` 和 `summary` 的 HTML-like 识别
- 若内容看起来是 HTML 片段，则仍走 `sanitize_remote_html` 并按 HTML 渲染
- 新增单测覆盖“summary fallback 为 HTML 时按 HTML 渲染”的路径

### 测试文档沉淀

- 新增 `docs/testing/mainline-validation-matrix.md`
- 新增 `docs/testing/environment-limitations.md`
- 更新 `docs/testing/README.md`，把两份长期文档纳入目录索引

## 验证与验收

### 自动化验证

- `cargo fmt --all`
- `cargo test -p rssr-app reader_treats_html_like_summary_as_html_fallback`
- `cargo check -p rssr-app`
- `cargo check -p rssr-app --target wasm32-unknown-unknown`
- `git diff --check`

### 手工验收

- 本轮未执行 Chrome MCP 或 Desktop 运行时手测
- 阅读页 fallback 变更当前以单测和构建验证为主

## 风险与后续事项

- HTML-like 识别仍然是启发式规则，后续如发现误判文本，可再收紧标签集合
- `docs/testing/mainline-validation-matrix.md` 当前是长期入口，不是一次性验收报告；后续应持续回填最新验证来源

## 给下一位 Agent 的备注

- 如果后续要继续吸收 `zheye-mainline-stabilization` 中的 hotfix，下一步更值得看的是新增订阅输入框的 paste fallback，而不是旧的 entries 页面修复

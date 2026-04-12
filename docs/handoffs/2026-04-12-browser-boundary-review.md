# Browser Boundary Review

- 日期：2026-04-12
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：b0ea114
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

审查 `rssr-app` / `rssr-infra` 当前剩余浏览器壳层边界，判断哪些模块已经停在合理终点，哪些模块仍是后续最值得处理的结构风险。

## 影响范围

- 模块：
  - `crates/rssr-app/src/ui/shell_browser.rs`
  - `crates/rssr-app/src/pages/entries_page/browser_interactions.rs`
  - `crates/rssr-app/src/pages/settings_page/themes/theme_file_io.rs`
  - `crates/rssr-infra/src/application_adapters/browser/feed.rs`
  - `crates/rssr-infra/src/application_adapters/browser/state/storage.rs`
- 平台：
  - Web
- 额外影响：
  - architecture review

## 关键变更

### 审查结论

- 本次未修改生产代码。
- 本次结论是：
  - `shell_browser.rs` 应视为合理壳层终点，不建议继续为“消除 `web_sys` 名称”而上提抽象。
  - `entries_page/browser_interactions.rs` 与 `theme_file_io.rs` 也已处于可接受的页面 / 主题壳层边界。
  - 下一批真正值得处理的 P1 风险位于 `browser/feed.rs`，不是 UI helper。

### P1 风险

- `crates/rssr-infra/src/application_adapters/browser/feed.rs`
  - 同一文件同时承担：
    - browser origin / `window` 访问
    - feed proxy URL 构造
    - CORS / 回退请求策略
    - HTML 壳 / 登录页识别
    - feed 解析与 entry 规范化
  - 这会把“环境探测”“网络策略”“响应诊断”“解析语义”耦在一起，直接影响 `rssr-web browser feed smoke` 的诊断与演进。
  - 后续若继续收敛，优先考虑按职责拆为：
    - browser request policy / proxy helper
    - response shell detection
    - feed parse normalization

### P2 风险

- `crates/rssr-infra/src/application_adapters/browser/state/storage.rs`
  - 仍是浏览器本地状态持久化单点；当前职责清晰，但如果后续 Web 状态模型继续增长，这里会成为演化热点。
- `crates/rssr-app/src/web_auth.rs` 已完成 browser helper 拆分，当前剩余哈希 / 凭据逻辑属于本体，不建议继续细拆。

### 可接受终点

- `crates/rssr-app/src/ui/shell_browser.rs`
  - 只负责 app shell 级 local storage 与认证切换 reload，职责收口明确。
- `crates/rssr-app/src/pages/entries_page/browser_interactions.rs`
  - 只负责 entries 页面滚动和页面偏好持久化，属于页面壳层。
- `crates/rssr-app/src/pages/settings_page/themes/theme_file_io.rs`
  - 只负责 CSS 文件导入导出与浏览器下载节点操作，属于主题实验室壳层。

## 验证与验收

### 自动化验证

- 未执行；本次为只读架构审查，无代码改动。

### 手工验收

- 未执行；本次为只读架构审查。

## 结果

- 已形成下一批壳层边界工作的优先级结论。
- 后续若继续收敛，优先目标应从 UI helper 转向 `browser/feed.rs`。

## 风险与后续事项

- 本次结论基于静态阅读，不包含新的运行时实测。
- 如果准备处理 `browser/feed.rs`，最好把“代理 URL 构造 / HTML 壳探测 / 解析规范化”拆成三个最小步骤，而不是一次性重构整文件。

## 给下一位 Agent 的备注

- 继续推进时，先看 `crates/rssr-infra/src/application_adapters/browser/feed.rs` 与 `docs/testing/release-ui-coverage-matrix.md` 中关于 `rssr-web browser feed smoke` 的说明。
- 不建议再花时间继续拆 `shell_browser.rs`、`entries_page/browser_interactions.rs`、`theme_file_io.rs`，收益已经很低。

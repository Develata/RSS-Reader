# 退役 `/feed-proxy`

- 日期：2026-10-20
- 作者 / Agent：Claude Code
- 分支：refactor/retire-feed-proxy（基于 feat/rssr-web-remote-ui）
- 当前 HEAD：见 `git log refactor/retire-feed-proxy`
- 相关 commit：本记录同次提交
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

Develata 确认退役 `/feed-proxy`。服务端模式落地后，rssr-web 托管的页面由服务端抓取，界面不再经过代理；
静态包没有这个端点；GitHub Pages 演示关闭了全部在线 feed 操作。代理只剩官方从未提供的
"静态包 + 同源代理"组合，而它是服务端出站策略之外的第二个"替用户访问 URL"的实现。
现在 Web 只有两种形态：静态包（localStorage + 直连，受 CORS 限制），以及 rssr-web（服务端模式）。

## 影响范围

- 删除：
  - `crates/rssr-web/src/proxy.rs` 与 `proxy/isolation_tests.rs`
  - `scripts/run_rssr_web_proxy_feed_smoke.sh`、`scripts/browser/feed_proxy_isolation.mjs`
  - `docs/testing/rssr-web-proxy-feed-smoke.md`
  - CI 的 `feed-proxy-isolation` job
- 浏览器端（`rssr-infra/src/application_adapters/browser/`）：
  - `feed_request.rs` 只剩直连地址的构造：刷新时附加 `_rssr_fetch`，订阅发现时使用原地址。
  - `feed.rs` 只发一次直连请求，订阅探测的最终地址取自响应。
  - `feed_response.rs` 删除代理登录页 / SPA 外壳的判别，只保留 HTML 正文判别；`parse_feed` 的错误文案不再提代理。
- `rssr-web`：去掉 `/feed-proxy` 路由；`reqwest` 只在测试里用，改为 dev-dependency。
- release-UI runner：删除 `proxy-feed` 步骤、`--skip-external-feed` 选项、`Partial` 状态、报告字段
  `external_feed_skipped` 与 `completed-with-skips` 结果；同步更新验收测试。
- 浏览器脚本：`rssr_small_viewport_assertions.mjs` 删除代理拦截模式（直连拦截原本就有）；
  `rssr_refresh_cost_acceptance.cjs` 改为拦截带 `_rssr_fetch` 的直连请求，并返回 CORS 头。
- 文档：`docs/deployment/web.md`（两种形态与退役说明）、设计文档新增第 7 节、`CLAUDE.md`、`README.md`、
  `docs/README*.md`、`docs/user-guide.md`、`docs/agent-handoff.md` 及 `docs/testing/` 下 11 个文件。
  `docs/handoffs/`、`evidence/`、`baselines/` 等历史记录不改。
- 保留：浏览器自动 Feed Smoke（`/__codex/browser-feed-smoke` 与 `/__codex/feed-fixture.xml`），它现在验证的是服务端模式。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`：通过
- `cargo test --workspace --locked`：exit 0，44 个测试二进制
- `cargo check -p rssr-app --target wasm32-unknown-unknown --locked`：通过
- wasm 浏览器契约：`wasm_subscription_contract_harness` 20 passed，`wasm_refresh_contract_harness` 28 passed。
  本机装的 `wasm-bindgen-cli` 是 0.2.128，而本分支栈锁定 0.2.126，所以用临时安装的 0.2.126，
  并通过 `RSSR_WASM_BINDGEN_TEST_RUNNER` 显式指定。
- release-UI runner：fmt、clippy、`cargo test`、`cargo build` 通过；
  `python -X utf8 scripts/release-ui/tests/acceptance.py`：27 个测试通过（2 个跳过）。
- `ci.yml` 用 PyYAML 解析通过；两个改动的浏览器脚本 `node --check` 通过；改动的 shell 脚本 `bash -n` 通过。

### 手工验收

- 未在浏览器里手工复验静态包的直连刷新。`wasm_refresh_contract_harness` 覆盖了浏览器端刷新路径。
- `rssr_refresh_cost_acceptance.cjs` 与小视口脚本本次没有运行（需要 Playwright / CDP 的完整环境）。

## 结果

- 可合并。叠在 #30 之上。

## 风险与后续事项

- 升级前打开的旧标签页里，旧前端仍会请求 `/feed-proxy`，拿到 404 后退回直连，不会出错。
- 若有人自行把静态包放在转发 `/feed-proxy` 的反向代理后面，升级后这个组合失效，需要改用 rssr-web 服务端模式。

## 给下一位 Agent 的备注

- 入口：`crates/rssr-infra/src/application_adapters/browser/feed.rs`、设计文档第 7 节。

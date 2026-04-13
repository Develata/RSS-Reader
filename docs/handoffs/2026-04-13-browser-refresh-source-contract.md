# Browser Refresh Source Contract

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：0969379
- 相关 commit：0969379
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

补一份 browser refresh source-side 的契约说明，把 `/feed-proxy` smoke、browser refresh 请求/回退规则、失败分类和排障顺序统一写进测试文档，避免后续继续把 source-side 问题和 store-side 问题混在一起看。

## 影响范围

- 模块：
  - `docs/testing/rssr-web-proxy-feed-smoke.md`
- 平台：
  - Web
  - Docker / `rssr-web`
- 额外影响：
  - browser refresh source-side failure triage
  - `/feed-proxy` smoke 文档定位

## 关键变更

### 明确 `/feed-proxy` smoke 在链路里的位置

- 写明它只验证 source-side 最前面的部署壳链路：
  - 登录态
  - 同源 `/feed-proxy`
  - 返回真实 XML feed
- 明确它不覆盖：
  - browser state 写回
  - entries 落本地状态
  - UI 页面跳转

### 明确 source-side 契约

- 写入请求顺序：
  - 有 origin 时先 proxy，再 direct
  - direct HTTP/HTTPS 附加 `_rssr_fetch`
- 写入 fallback 条件：
  - `404` / `401` / `403` / `405` / `400`
  - 成功返回但像 login shell / SPA shell
- 写入 outcome 映射：
  - `抓取订阅失败:`
  - `feed 抓取返回非成功状态:`
  - `解析订阅失败:`
  - `NotModified`

### 明确失败分诊顺序

- 先看 `/feed-proxy` smoke
- 再看 `rssr-web browser feed smoke`
- 再按 source-side 失败类型分流
- 最后才转查 store-side

## 验证与验收

### 自动化验证

- `git diff --check`：通过

### 手工验收

- 文档审查：
  - `docs/testing/rssr-web-proxy-feed-smoke.md`
  - `crates/rssr-infra/src/application_adapters/browser/feed_request.rs`
  - `crates/rssr-infra/src/application_adapters/browser/feed_response.rs`
  - `crates/rssr-infra/src/application_adapters/browser/feed.rs`
  - `crates/rssr-infra/src/application_adapters/browser/adapters/refresh.rs`

## 结果

- 本次文档同步可合并。
- browser refresh 的 source-side / store-side 分界、source-side failure 类型和排障顺序现在有了单一说明入口。

## 风险与后续事项

- 这次只统一了 source-side 契约说明，没有新增 source-side 自动化 harness。
- 若下一步继续推进，优先做 source-side harness 设计，而不是再继续扩写描述性文档。

## 给下一位 Agent 的备注

- 若后续遇到 browser refresh 失败，先读：
  - `docs/testing/rssr-web-proxy-feed-smoke.md`
  - `docs/handoffs/2026-04-13-browser-feed-smoke-diagnostics.md`
  - `docs/handoffs/2026-04-13-browser-refresh-store-contracts.md`
- 这三份文档分别对应：
  - source-side
  - smoke 可观测性
  - store-side contract

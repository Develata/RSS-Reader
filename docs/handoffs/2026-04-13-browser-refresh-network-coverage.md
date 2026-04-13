# Browser Refresh Network Coverage

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：5d206b9
- 相关 commit：5d206b9
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

完成 browser refresh source-side 最后一个缺口的归属判断：network / CORS failure 不继续强塞进 contract harness，而归入真实 `rssr-web` smoke 和环境限制索引。原因是它依赖真实 browser fetch、部署壳和网络环境，继续用假 harness 会制造不稳定或不真实的边界。

## 影响范围

- 模块：
  - `docs/testing/contract-harness-rebuild-plan.md`
  - `docs/testing/rssr-web-browser-feed-smoke.md`
- 平台：
  - Web
  - Docker / `rssr-web`
  - wasm32 browser harness
- 额外影响：
  - browser refresh source-side coverage boundary
  - network / CORS failure 验证归属

## 关键变更

### contract harness 计划收口

- `contract-harness-rebuild-plan.md` 现在明确：
  - network / CORS failure 不进入 contract harness 当前范围
  - 该项由 `rssr-web` smoke 和环境限制索引覆盖
  - 不为 network / CORS failure 新建脱离真实浏览器网络模型的假 harness

### browser feed smoke 覆盖边界

- `rssr-web-browser-feed-smoke.md` 新增“覆盖边界”说明：
  - 它使用同源 `__codex/feed-fixture.xml`
  - 主要覆盖 browser 成功路径、store 写回、feed card 诊断属性和 entries page 跳转
  - 它不验证真实外部站点的 network / CORS failure
  - 真实外部 feed / `/feed-proxy` 部署壳链路归 `rssr-web-proxy-feed-smoke.md`
  - 纯静态 Web CORS 限制归 `environment-limitations.md`

## 验证与验收

### 自动化验证

- `git diff --check`：通过

### 手工验收

- 文档审查：
  - `docs/testing/contract-harness-rebuild-plan.md`
  - `docs/testing/rssr-web-browser-feed-smoke.md`
  - `docs/testing/rssr-web-proxy-feed-smoke.md`
  - `docs/testing/environment-limitations.md`

## 结果

- 本次文档收口可合并。
- browser refresh source-side 的 contract harness 范围现在已闭合：
  - body classification
  - status classification
  - request fallback 纯函数
  - store-side contract
  已进入自动化覆盖
- network / CORS failure 不再作为待补 contract harness 项。

## 风险与后续事项

- 如果未来确实需要自动化 network / CORS failure，应单独设计真实 browser fetch fixture，而不是复用当前 contract harness。
- 当前更实际的下一步是跑一轮相关验证入口，确认文档收口后的测试组合仍然健康。

## 给下一位 Agent 的备注

- 继续推进时建议优先跑：
  - `bash scripts/run_wasm_refresh_contract_harness.sh`
  - `bash scripts/run_rssr_web_browser_feed_smoke.sh`
  - `bash scripts/run_rssr_web_proxy_feed_smoke.sh`
- 如果其中 `/feed-proxy` 真实外部源失败，先按 `docs/testing/environment-limitations.md` 和 `docs/testing/rssr-web-proxy-feed-smoke.md` 判断是否为网络/外部源问题。

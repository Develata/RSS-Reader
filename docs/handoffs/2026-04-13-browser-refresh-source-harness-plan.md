# Browser Refresh Source Harness Plan

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：6e958f9
- 相关 commit：6e958f9
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

继续按 browser refresh source-side 的建议推进，但本轮只做测试蓝图回写，不碰生产实现。目标是先把 `BrowserFeedRefreshSource` 后续 harness 的边界、最小覆盖集和明确不做的做法写进主线测试计划，避免后续为了可测性反向污染 `wasm32` 专属 browser adapter 边界。

## 影响范围

- 模块：
  - `docs/testing/contract-harness-rebuild-plan.md`
- 平台：
  - Web
  - wasm32 browser harness
- 额外影响：
  - browser refresh source-side harness 设计
  - refresh contract harness 进度说明

## 关键变更

### 回写 refresh store-side 当前覆盖

- 计划文档现在明确：
  - `BrowserRefreshStore::list_targets`
  - `BrowserRefreshStore::get_target`
  - `commit(NotModified)`
  - `commit(Updated)` 清理旧 `fetch_error`
  - `commit(Failed)` 保留既有 `last_success_at`
  - localStorage 写回
  已属于 browser refresh store-side 的既有覆盖面。

### 新增 source-side harness 设计

- 把 source-side 定义为：
  - 请求顺序
  - fallback 判定
  - HTML shell / login shell 识别
  - 非成功状态映射
  - XML 解析失败映射
- 明确不包含：
  - browser state 写回
  - feed summary 时间戳更新
  - entries upsert

### 固定最小覆盖集和实现顺序

- source-side 最小覆盖集固定为 4 类：
  - network / CORS failure
  - proxy shell / login shell
  - non-success status
  - parse failure
- 建议实现顺序固定为：
  1. 先有设计清单
  2. 再评估是否需要 response classification helper
  3. 最后才补真正的 source-side harness

### 明确不该做的事

- 不为 `BrowserFeedRefreshSource` 引入 host-only mock 结构
- 不把 source-side harness 绑到 `rssr-app` 页面或 `data-action`
- 不为了 source-side harness 回退当前 `wasm32` browser adapter 边界

## 验证与验收

### 自动化验证

- `git diff --check`：通过

### 手工验收

- 文档审查：
  - `docs/testing/contract-harness-rebuild-plan.md`
  - `docs/testing/rssr-web-proxy-feed-smoke.md`
  - `docs/handoffs/2026-04-13-browser-refresh-source-contract.md`
  - `docs/handoffs/2026-04-13-browser-refresh-store-contracts.md`

## 结果

- 本次设计回写可合并。
- browser refresh source-side 后续实现现在不再需要从零决定：
  - 测什么
  - 不测什么
  - 先做哪一步

## 风险与后续事项

- 这次只完成设计固化，没有新增 source-side harness。
- 下一步如果真正开始实现，应先判断：
  - 现有 `feed_request.rs` / `feed_response.rs` 的纯函数入口是否足够
  - 是否确实需要额外 response classification helper

## 给下一位 Agent 的备注

- 若继续推进 source-side harness，先读：
  - `docs/testing/contract-harness-rebuild-plan.md`
  - `docs/testing/rssr-web-proxy-feed-smoke.md`
  - `crates/rssr-infra/src/application_adapters/browser/feed_request.rs`
  - `crates/rssr-infra/src/application_adapters/browser/feed_response.rs`
- 下一步应优先做“设计到测试入口”的最小实现，而不是先改生产路径命名或分层。

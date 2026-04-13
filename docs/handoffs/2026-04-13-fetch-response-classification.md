# Fetch Response Classification

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：7ec485a
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

继续收口 `rssr-infra` 的 fetch client 拆分，把 feed HTTP response 的 cache metadata 提取与 `304 Not Modified` 分类从 `FetchClient::fetch(...)` 主流程中拆到独立 helper，补齐之前剩余的 response classification 切口。

## 影响范围

- 模块：
  - `crates/rssr-infra/src/fetch/client.rs`
  - `crates/rssr-infra/src/fetch/client/feed_http.rs`
  - `crates/rssr-infra/src/fetch/client/feed_response.rs`
- 平台：
  - Linux
  - Desktop / native fetch path
- 额外影响：
  - feed refresh HTTP response handling
  - fetch client unit tests

## 关键变更

### Response Classification

- 新增 `feed_response.rs` 子模块。
- 新增 `classify_feed_response_status(status)`，当前只把 `304 Not Modified` 与需要继续读 body 的响应分开。
- `FetchClient::fetch(...)` 继续保留原有 `error_for_status().context("feed 抓取返回非成功状态")` 调用，避免改变非成功 HTTP 状态的错误语义。

### Metadata Extraction

- 新增 `http_metadata_from_headers(headers)`，集中提取 `ETag` 与 `Last-Modified`。
- `feed_http.rs` 主流程不再内联 header 解析。

### Tests

- 新增 `feed_response` 单测：
  - `classify_feed_response_status_separates_not_modified_from_body_reads`
  - `http_metadata_from_headers_extracts_valid_cache_headers`

## 验证与验收

### 自动化验证

- `cargo test -p rssr-infra --lib`：通过，17 passed
- `cargo test -p rssr-infra --test test_feed_refresh_flow`：通过，2 passed
- `cargo test -p rssr-infra --test test_refresh_contract_harness`：通过，4 passed
- `cargo fmt --check`：通过
- `git diff --check`：通过

### 手工验收

- 静态代码复核：通过
- 确认 public fetch API 未变化：通过，`FetchClient` / `FetchRequest` / `FetchResult` / `HttpMetadata` 仍由 `fetch/client.rs` re-export

## 结果

- `FetchClient::fetch(...)` 中的 response 判断与 metadata 提取已从请求主流程中移出。
- `fetch/client.rs` 的 P1-P2 拆分线现在覆盖了 feed fetch、image localization、HTML/image-tag helper 与 response classification。

## 风险与后续事项

- 非成功 HTTP 状态仍由 `reqwest::Response::error_for_status()` 负责生成错误；本轮有意不改变错误结构。
- 如果后续要继续降低 fetch path 复杂度，可把请求 builder 构造也拆成 helper，但优先级低于 P2 port 语义重复与 P3 summary 文案重复清理。

## 给下一位 Agent 的备注

- 入口文件：
  - `crates/rssr-infra/src/fetch/client/feed_http.rs`
  - `crates/rssr-infra/src/fetch/client/feed_response.rs`
  - `crates/rssr-infra/src/fetch/client/body_asset_localizer.rs`
  - `crates/rssr-infra/src/fetch/client/image_html.rs`
- 当前同一 worktree 里还存在未提交的 auth 拆分增量；本次 fetch 改动与 auth 文件互不重叠。

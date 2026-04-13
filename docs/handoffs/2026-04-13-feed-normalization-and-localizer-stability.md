# Feed Normalization And Localizer Stability

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：a9f6c5b
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

按优先级继续推进 P1 稳定性收口，先完成 `feed_normalization.rs` 的 warning 聚合，再在 `fetch/client.rs` 为正文图片本地化加一层轻量节流并拆出 HTML 图片 source 收集 helper。

## 影响范围

- 模块：
  - `crates/rssr-infra/src/feed_normalization.rs`
  - `crates/rssr-infra/src/fetch/client.rs`
- 平台：
  - Linux
  - desktop
  - Web
  - wasm32
  - CLI
- 额外影响：
  - parser / image localization 诊断日志

## 关键变更

### Feed Normalization

- 新增内部 `MissingContentWarningAggregation`，对缺少 `summary` 和 `content` 的 sparse 条目按 feed 聚合 warning。
- `normalize_entry(...)` 不再逐条打 warn，而是记录聚合计数与样例标题。
- `normalize_feed(...)` 在整份 feed 处理完成后只输出一条聚合 warning，带 `feed_title`、`skipped_entry_count` 和 `sample_entry_titles`。

### Body Asset Localizer

- `BodyAssetLocalizer` 新增全局 `Semaphore`，限制正文图片抓取并发，降低多 feed / 多 entry 后台本地化同时起请求时的突发量。
- `BodyAssetLocalizer::default()` 改为显式走 `new()`，保持外部构造方式不变。
- 从 `localize_html_images(...)` 抽出 `collect_localizable_sources(...)`，把 HTML `<img>` source 扫描、placeholder 跳过和图片数上限控制收成单独 helper，降低主流程复杂度。

## 验证与验收

### 自动化验证

- `cargo test -p rssr-infra --lib`：通过
- `cargo test -p rssr-infra --test test_feed_parse_dedup`：通过
- `cargo fmt --check`：通过
- `git diff --check`：通过

### 手工验收

- 静态代码复核：通过
- 确认 parser warning 聚合没有扩散到 application contract 或 UI 文案层。
- 确认正文图片本地化的节流只影响并发抓取，不改变现有 rewrite / placeholder / `srcset` 行为。

## 结果

- `feed_normalization.rs` 的 sparse-entry warning 噪声已明显收口。
- `fetch/client.rs` 在不改外部接口的前提下加入了基础节流，且主流程复杂度较之前更容易继续拆分。

## 风险与后续事项

- parser warning 目前仍只进 tracing，没有进入 refresh outcome；如果后续需要用户可见提示，需要单独设计 warning 返回面。
- `fetch/client.rs` 仍然偏重；这轮只先做节流和 helper 提取，后续仍建议继续拆分：
  - feed fetch
  - image localization
  - response classification
- native `ImageLocalizationWorker` 仍是逐条 entry warning；若后续日志噪声仍高，可考虑做批量结果汇总。

## 给下一位 Agent 的备注

- 先看：
  - `crates/rssr-infra/src/feed_normalization.rs`
  - `crates/rssr-infra/src/fetch/client.rs`
- 如果继续做 `client.rs`，优先从这几个切口拆：
  - `collect_localizable_sources(...)`
  - `fetch_image_as_data_url(...)`
  - `rewrite_localized_image_tag(...)`

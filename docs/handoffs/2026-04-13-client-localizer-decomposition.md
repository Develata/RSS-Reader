# Client Localizer Decomposition

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：57ef326
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

继续推进 `rssr-infra/src/fetch/client.rs` 的 P1 收口，在不改 public API 的前提下把正文图片本地化流程进一步拆成“计划收集 / 网络抓取 / HTML 回写”三段，并保留前一轮加入的节流能力。

## 影响范围

- 模块：
  - `crates/rssr-infra/src/fetch/client.rs`
- 平台：
  - Linux
  - desktop
  - Web
  - wasm32
  - CLI
- 额外影响：
  - image localization 内部复杂度
  - infra unit tests

## 关键变更

### Localizer Internal Structure

- 在 `BodyAssetLocalizer` 内新增 `LocalizationPlan`，把 HTML 本地化前的准备信息收成显式中间结果。
- `localize_html_images(...)` 现在只负责高层编排：
  - `build_localization_plan(...)`
  - `localize_sources(...)`
  - `rewrite_localized_html(...)`
- `build_localization_plan(...)` 统一承接：
  - HTML 大小与 `<img>` 快速短路
  - `<img>` 正则构造
  - source 收集与图片数上限控制
- `localize_sources(...)` 统一承接：
  - 逐个 source 的抓取
  - 总体积上限判断
  - 单图失败 logging
- `rewrite_localized_html(...)` 把整段 HTML 的回写与单 tag rewrite 分开，便于后续继续拆出独立模块。

### Tests

- 新增 `rewrite_localized_html_only_rewrites_matching_image_tags`，锁定“只替换命中的 `<img>`，不误伤其他标签或未命中的图片”的行为。

## 验证与验收

### 自动化验证

- `cargo test -p rssr-infra --lib`：通过
- `cargo fmt --check`：通过
- `git diff --check`：通过

### 手工验收

- 静态代码复核：通过
- 确认 `BodyAssetLocalizer::new()` / `Default` / `localize_html_images(...)` 的外部调用方式未变化。

## 结果

- `client.rs` 虽然仍偏重，但正文图片本地化主流程已经从单个大函数收成 3 段职责更清楚的内部流程。
- 后续如果继续拆 `fetch/client.rs`，可以从这些 helper 直接下刀，而不需要再先整理一次主函数。

## 风险与后续事项

- `fetch/client.rs` 仍同时承载：
  - feed fetch
  - image localization
  - HTML rewrite
  - content-type / URL 过滤
- 下一步更值得继续的方向：
  - 把 image localization 相关 helper 下沉成独立子模块
  - 把 feed fetch 和 body asset localization 从同一文件彻底分开

## 给下一位 Agent 的备注

- 优先看：
  - `crates/rssr-infra/src/fetch/client.rs`
  - `docs/handoffs/2026-04-13-feed-normalization-and-localizer-stability.md`
- 如果继续拆：
  - 先把 image-localization helpers 独立成模块
  - 再考虑把 `FetchClient` 与 `BodyAssetLocalizer` 拆文件

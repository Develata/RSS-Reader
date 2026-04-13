# Client Image Html Module Split

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：57ef326
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

继续推进 `crates/rssr-infra/src/fetch/client.rs` 的复杂度拆解，把纯 HTML / image-tag 处理逻辑从主文件拆到独立子模块 `crates/rssr-infra/src/fetch/client/image_html.rs`，让 `client.rs` 更聚焦在 HTTP fetch、本地化编排和节流。

## 影响范围

- 模块：
  - `crates/rssr-infra/src/fetch/client.rs`
  - `crates/rssr-infra/src/fetch/client/image_html.rs`
- 平台：
  - Linux
  - desktop
  - Web
  - wasm32
  - CLI
- 额外影响：
  - image localization helper layout
  - infra unit tests

## 关键变更

### Module Split

- 新增 `fetch/client/image_html.rs`，承接这些纯 HTML / URL / attribute 处理逻辑：
  - `image_tag_regex`
  - `collect_localizable_sources`
  - `resolve_asset_url`
  - `normalize_image_content_type`
  - `rewrite_localized_html`
  - 以及相关内部 helper
- `fetch/client.rs` 现在只保留：
  - `FetchClient`
  - `BodyAssetLocalizer`
  - `LocalizationPlan`
  - 图片抓取、本地化编排、节流与总体积控制

### Tests

- 相关 HTML/image-tag unit tests 一并迁移到 `image_html.rs`。
- `client.rs` 保留最小桥接测试，确认：
  - `BodyAssetLocalizer::default()` 与 `new()` 行为一致
  - content-type 判断仍正确委托给 `image_html` 子模块

## 验证与验收

### 自动化验证

- `cargo test -p rssr-infra --lib`：通过
- `cargo fmt --check`：通过
- `git diff --check`：通过

### 手工验收

- 静态代码复核：通过
- 确认 `rssr-app` 和 `rssr-infra` 现有调用点仍然只依赖：
  - `FetchClient`
  - `BodyAssetLocalizer`
  - `localize_html_images(...)`
  外部 API 未变化。

## 结果

- `client.rs` 不再混合承载所有 HTML attribute / regex / rewrite 细节。
- 后续若继续拆分 image localization，可以直接围绕 `image_html.rs` 与 `BodyAssetLocalizer` 两块继续推进。

## 风险与后续事项

- `client.rs` 仍同时包含：
  - feed fetch
  - image fetch
  - 本地化编排
  - 节流控制
- 下一步值得继续做的事：
  - 把 `FetchClient` 与 `BodyAssetLocalizer` 分成独立文件
  - 再评估是否把 image fetch/data-url 逻辑也下沉成单独子模块

## 给下一位 Agent 的备注

- 优先看：
  - `crates/rssr-infra/src/fetch/client.rs`
  - `crates/rssr-infra/src/fetch/client/image_html.rs`
- 如果继续拆：
  - 先拆 `BodyAssetLocalizer` 的网络抓取部分
  - 再考虑把 `FetchClient` 与 body-asset localization 从同一源码文件彻底分离

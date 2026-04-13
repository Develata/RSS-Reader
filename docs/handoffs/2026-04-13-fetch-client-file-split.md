# Fetch Client File Split

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：57ef326
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

继续推进 `rssr-infra` 的 `fetch/client.rs` 拆分，把 `FetchClient`、`BodyAssetLocalizer` 和 HTML/image-tag helpers 分成独立源码文件，同时保持 `crate::fetch::{FetchClient, BodyAssetLocalizer, FetchRequest, FetchResult, HttpMetadata}` 的对外接口不变。

## 影响范围

- 模块：
  - `crates/rssr-infra/src/fetch/client.rs`
  - `crates/rssr-infra/src/fetch/client/feed_http.rs`
  - `crates/rssr-infra/src/fetch/client/body_asset_localizer.rs`
  - `crates/rssr-infra/src/fetch/client/image_html.rs`
- 平台：
  - Linux
  - desktop
  - Web
  - wasm32
  - CLI
- 额外影响：
  - infra fetch module layout
  - localizer unit test layout

## 关键变更

### File Split

- `fetch/client.rs` 收缩成模块入口文件，只保留：
  - `mod body_asset_localizer;`
  - `mod feed_http;`
  - `mod image_html;`
  - `pub use ...`
- 新增 `fetch/client/feed_http.rs`
  - 承接 `FetchClient`
  - 承接 `FetchRequest`
  - 承接 `FetchResult`
  - 承接 `HttpMetadata`
- 新增 `fetch/client/body_asset_localizer.rs`
  - 承接 `BodyAssetLocalizer`
  - 承接本地化编排、节流与图片抓取
  - 保留 `Default = new()`
- `fetch/client/image_html.rs`
  - 继续承接 HTML/image-tag source 收集、URL 解析、rewrite 等纯 helper 逻辑

### Tests

- `image_html.rs` 保留 HTML/image-tag 相关 unit tests。
- `body_asset_localizer.rs` 保留本地化器桥接测试，确认：
  - `default()` 与 `new()` 一致
  - content-type 判断仍委托给 `image_html`

## 验证与验收

### 自动化验证

- `cargo test -p rssr-infra --lib`：通过
- `cargo fmt --check`：通过
- `git diff --check`：通过

### 手工验收

- 静态代码复核：通过
- 确认外部调用点无需改动，`rssr-app` / `rssr-infra` 现有 import 仍然有效。

## 结果

- `fetch/client.rs` 不再承担具体实现，只是清晰的模块入口。
- `FetchClient` 与 `BodyAssetLocalizer` 已完成源码级解耦，后续继续拆时不再需要先做文件级整理。

## 风险与后续事项

- 目前 `feed_http.rs` 与 `body_asset_localizer.rs` 仍在同一 `client/` 子树下，只是文件分离，不是更高层语义模块分离。
- 下一步更值得继续的方向：
  - 评估是否把 `BodyAssetLocalizer` 提升为独立顶层模块，而不是继续挂在 `client/`
  - 继续收 `rssr-web/src/auth.rs`
  - 或回到 composition/bootstrap 共用 builder 收口

## 给下一位 Agent 的备注

- 优先看：
  - `crates/rssr-infra/src/fetch/client.rs`
  - `crates/rssr-infra/src/fetch/client/feed_http.rs`
  - `crates/rssr-infra/src/fetch/client/body_asset_localizer.rs`
  - `crates/rssr-infra/src/fetch/client/image_html.rs`
- 如果继续拆 infra fetch：
  - 先决定 `BodyAssetLocalizer` 是否应升级为独立顶层模块
  - 再考虑 feed fetch / image fetch 是否还需要共享同一 `client` 命名空间

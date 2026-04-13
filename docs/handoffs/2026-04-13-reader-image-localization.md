# Reader Image Localization

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：ba73a07
- 相关 commit：ba73a07
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

针对阅读页图片、表情包和 lazy-loaded 图片经常显示为 broken image 的反馈，增强 native 正文图片本地化 worker 与 reader HTML 清洗的图片来源兼容性。

## 影响范围

- 模块：
  - `crates/rssr-infra/src/fetch/client.rs`
  - `crates/rssr-app/src/pages/reader_page/support.rs`
- 平台：
  - desktop / native
  - reader 页面 HTML 渲染
- 额外影响：
  - tests

## 关键变更

### BodyAssetLocalizer

- 图片候选来源从只读取 `<img src="...">` 扩展到：
  - `data-src`
  - `data-original`
  - `data-lazy-src`
  - `data-orig-file`
  - `srcset`
  - `src`
- lazy 属性和 `srcset` 优先于常见占位 `src`。
- 本地化成功后会把对应 `<img>` 的 `src` 改成本地 `data:` URL，并移除远端 `srcset`，避免浏览器继续优先请求失败的远端候选图。
- 图片抓取请求增加 image-like `Accept`、browser-like `User-Agent`，并在有文章 URL 时发送 `Referer`。
- 单篇文章本地化上限从 4 张提高到 8 张；单图上限从 512 KiB 提高到 1 MiB，总量上限从 1 MiB 提高到 2 MiB。
- 跳过常见占位图资源：`placeholder`、`blank.gif`、`transparent.gif`、`spacer.gif`、`1x1.gif`、`pixel.gif`。

### Reader sanitizer

- `sanitize_remote_html()` 改为显式 ammonia builder。
- 对 `<img>` 保留图片来源相关属性：
  - `class`
  - `data-src`
  - `data-original`
  - `data-lazy-src`
  - `data-orig-file`
  - `srcset`
- 仍会移除事件属性，例如 `onerror`。

## 验证与验收

### 自动化验证

- `cargo fmt --check`：通过
- `cargo test -p rssr-infra fetch::client`：通过，7 passed
- `cargo test -p rssr-infra`：通过
- `cargo test -p rssr-app reader_`：通过，5 passed
- `cargo test -p rssr-app`：通过，31 passed
- `cargo check --workspace`：通过
- `git diff --check`：通过

### 手工验收

- 已复核 `BodyAssetLocalizer` 不进入 application 层，仍保留在 native host worker / infra fetch 边界。
- 已复核 reader sanitizer 保留的仅是 `<img>` 图片来源相关属性，未放开脚本或事件处理。

## 结果

- 本次交付可合并。
- native 刷新后的新文章更可能把 lazy/srcset 图片、表情包候选图本地化为 `data:` URL，从而减少 broken image。

## 风险与后续事项

- Web 端目前没有正文图片本地化 worker；如果 Web 端远端图片仍因 hotlink、referrer 或网络策略失败，需要单独设计 browser-safe 图片代理或缓存方案。
- 已存在的旧文章不会自动重写；需要重新刷新订阅源或后续补迁移/重本地化入口。
- 更大的本地化上限会增加 native SQLite 内容体积，但仍有单图和总量上限保护。

## 给下一位 Agent 的备注

- 用户反馈的 broken image 不全是 CSS 问题；主要涉及 HTML 来源属性、远端图片请求策略和 native/web 本地化能力差异。
- 若继续处理 Web 图片失败，先做 source-side 方案审查，不要把任意远端图片代理直接塞进 shared application use case。

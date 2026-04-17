# 阅读页正文图片本地化修复

- 日期：2026-04-17
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：d6112a1
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

实现了桌面端阅读页正文图片失败的系统性修复，并补上了一轮基于真实坏图源的收敛修补。当前这份交付额外纳入了“正文未本地化时，桌面端仍需稳定实时加载远端图片”的修复。新增修补针对三类仍然高频出现的失败：

- WordPress / NVIDIA feed 内把 emoji 当远端小图输出，阅读页此前会直接尝试加载 `s.w.org` 远端资源，失败后显示坏图标。
- 正文大图同时带 `src` 和 `srcset` 时，本地化此前优先抓取更大的 `srcset` 候选，容易碰到单图预算或站点策略限制，导致本地化没落下来。
- 正文 HTML 超过本地化预算时，此前会整篇跳过抓图；如果原文依赖 `data-src` / `data-srcset` / 相对路径 / `<base href>`，阅读容器里又没有站点原生 JS 和文档级 `<base>`，图片会继续保持坏占位或直接不显示。
- 即使正文已被归一化成远端 `https://...` 图片，桌面 WebView 直连部分站点资源仍然存在坏图。对 NVIDIA / WordPress 这类源，单靠“保留远端链接”还不够，需要 native 主机接管图片请求。
- 新增桌面代理后，又暴露出一条更隐蔽的回归：图片仍然失败时，浏览器会显示 `alt` 文本；而我们的 HTML 属性重写在保留未修改属性时会把已有的 `&quot;` / `&amp;` 再 escape 一次，导致用户看到成片 `&amp;amp;...` 乱码。
- 继续追查后确认，还有一条独立根因：正文图片一旦本地化成 `data:image/...`，阅读页 `ammonia` 清洗默认并不允许 `data:` scheme，结果 `img src` 被移除，只剩坏图图标和正常的 `alt`。

## 影响范围

- 模块：
  - `crates/rssr-infra/src/fetch/client/image_html.rs`
  - `crates/rssr-infra/src/fetch/client/body_asset_localizer.rs`
  - `crates/rssr-app/src/bootstrap*.rs`
  - `crates/rssr-app/src/pages/reader_page/*`
  - `crates/rssr-app/src/ui/runtime/*`
- 平台：
  - Windows
  - macOS
  - Linux
  - Android
  - Web（仅编译兼容；正文图片本地化仍保持 no-op）
- 额外影响：
  - N/A

## 关键变更

### 正文图片抽取与重写

- 将 `image_html.rs` 从纯 regex 的 `<img>` 扫描改为 HTML 片段扫描器，支持：
  - `img[src]`
  - `img[srcset]`
  - `img[data-src]`
  - `img[data-original]`
  - `img[data-lazy-src]`
  - `img[data-orig-file]`
  - `img[data-srcset]`
  - `picture > source[srcset]`
- 支持正文里的 `<base href>` 参与相对地址解析。
- `srcset`/`data-srcset` 现在只选一个最终候选 URL 做本地化，不再把整组候选都抓一遍。
- 对普通 `img` 标签，抓取优先级改为：
  - `data-src` / `data-original` / `data-lazy-src` / `data-orig-file`
  - `src`
  - `data-srcset` / `srcset`
- 这样可以优先抓取正文实际展示的 `src`，避免因为盲选最大 `srcset` 候选而把 WordPress 大图抓成更重的原图版本。
- `s.w.org` / WordPress smilies 这类 emoji 资源现在直接视为“不参与本地化的装饰资源”，不会继续占用正文图片预算。
- 新增“实时加载归一化”步骤：
  - 即使不抓图，也会先把正文里的 `img/source` 补成浏览器可立即加载的形态。
  - 归一化范围包括：提升 `data-src` / `data-srcset`、解析相对路径、吃掉 `<base href>` 影响并回写成绝对 URL。
  - 这样 `html_too_large` 只会跳过“抓图转 data URL”，不会再把图片一起跳没。
  - 对普通 `img`，实时加载路径会进一步收敛到单一展示源，移除 `srcset`、`sizes`、`loading`、`fetchpriority`，避免浏览器继续选到别的响应式候选图，或继续沿用站点原始的 lazy 策略。

### 本地化预算与日志分类

- `BodyAssetLocalizer` 增加两套预算：
  - 后台刷新后的保守预算
  - 当前阅读文章的宽松预算
- 抓图失败日志现在显式区分：
  - `unsupported_pattern`
  - `resolve_failed`
  - `timeout`
  - `http_status`
  - `non_image_content_type`
  - `too_large`
  - `too_large_total`

### 阅读页按需本地化

- 新增 host capability：阅读页可对当前文章触发正文图片按需本地化。
- native 侧在打开文章后，如果当前快照仍含 HTML 正文且尚未尝试本地化，会异步触发一次当前文章本地化；成功后自动 `BumpReload` 重新加载当前文章。
- Web 侧 capability 为 no-op，避免把浏览器限制回流到 application / domain。

### 桌面端实时图片代理

- 桌面端应用入口新增异步自定义协议 `rssr-img://fetch?...`，由 native 主机用 `reqwest` 拉取上游图片并把字节流返回给 WebView。
- 阅读页清洗后的 HTML 在桌面平台会继续做一层运行时重写：
  - `img[src]`
  - `source[src]`
  - `source[srcset]`
  会被改写成 `rssr-img://` 代理地址。
- 代理请求会附带文章 URL 作为 `referer` 参数，并在 native 侧转成真实 `Referer` 请求头，尽量降低站点热链策略带来的失败率。
- 代理侧对目标 URL 与响应做了收敛：
  - 仅允许 `http/https`
  - 上游非 2xx 直接按状态失败
  - `Content-Type` 不是图片时拒绝回传
- 这样 `html_too_large` 现在只表示“未做 data URL 本地化”，不再等价于“阅读页只能裸连远端图片并听天由命”。

### HTML 属性重写稳定性修复

- `rssr-app` 和 `rssr-infra` 的轻量 HTML 标签解析器现在都区分两套属性值：
  - 已解码的逻辑值：用于 URL 解析、站点判断和重写决策
  - 规范化后的序列化值：用于写回 HTML
- 属性值现在会递归解码 HTML 实体直到稳定态，再统一 escape 一次写回。
- 这样不只是“避免继续放大”，也能把已经污染成 `&amp;amp;quot;` / `&amp;amp;amp;` 的旧正文收敛回单层 `&quot;` / `&amp;`。
- 这次修复同时覆盖：
  - 阅读页桌面运行时代理重写
  - `normalize_html_for_live_display`
  - 本地化时的 `rewrite_html`

### 阅读页 data URL 白名单修复

- `reader_page/support.rs` 的 `ammonia::Builder` 现在显式 `.add_url_schemes(&["data"])`。
- 这样正文里已经本地化完成的 `img src="data:image/...;base64,..."` 不会再在最终渲染前被清洗掉。
- 这条修复与桌面图片代理并行生效：
  - 远端未本地化图片继续走 `rssr-img://...`
  - 已本地化图片继续保留 `data:` 直出

### 桌面代理请求头修复

- 继续对真实文章排查后确认，仍有一部分图片虽然被重写为 `rssr-img://...`，但切到 native 代理后开始失败。
- 桌面图片代理现在显式补上浏览器态请求头：
  - `User-Agent`
  - `Accept-Language`
  - 现有 `Accept`
  - 现有 `Referer`
- 已新增本地回归测试，确认代理发出的上游请求确实带有这些 header。

### 桌面运行时代理回退

- 在继续排查真实文章后，确认出现了新的回归模式：
  - 之前原本能直接加载的远端图片，在引入 `rssr-img://...` 强制改写后开始失败。
  - 运行日志中没有出现对应的 `桌面图片代理...` 告警，说明实际 WebView 路径未稳定落到预期的自定义协议抓取链路上。
- 本轮已做保守回退：
  - 阅读页保留 lazy / 相对路径 / `<base href>` 归一化
  - 保留 `data:` 图片白名单
  - 保留实体污染修复
  - 不再把普通远端图片在阅读时强制改写成 `rssr-img://...`
- 当前判断是：
  - 之前“原本就能直接显示”的图片，应随这次回退恢复
  - 之前“因为 HTML 归一化缺失而失败”的图片，仍会受益于前几轮修复

### 阅读页 HTML 白名单

- `reader_page/support.rs` 现在保留 `picture`、`source` 以及 `img/source` 所需的 `src/srcset/sizes/media/type/data-srcset` 等属性。
- 新增对应单元测试，防止清洗阶段把响应式图片结构降级成坏图 HTML。
- 对 WordPress emoji 图片（例如 `class="wp-smiley"` 或 `src` 指向 `s.w.org/images/core/emoji/...`）新增阅读态归一化：
  - 渲染前直接用 `alt` 文本替换整张 emoji 图片。
  - 这样阅读页即使不走本地化，也不会再把 emoji 显示成坏图标。

## 验证与验收

### 自动化验证

- `cargo check -p rssr-app`：通过
- `cargo test -p rssr-app`：通过
- `cargo test -p rssr-infra --lib`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo fmt`：已执行
- 本轮新增或保留覆盖的关键测试包括：
  - `pages::reader_page::support::tests::reader_proxies_remote_images_for_desktop_runtime`
  - `pages::reader_page::support::tests::reader_proxy_rewrite_canonicalizes_polluted_alt_entities`
  - `pages::reader_page::support::tests::reader_preserves_localized_data_url_images_after_sanitizing`
  - `pages::reader_page::support::tests::reader_resolves_relative_lazy_images_before_rendering`
  - `pages::reader_page::support::tests::reader_keeps_remote_images_direct_after_normalizing`
  - `pages::reader_page::support::tests::reader_normalization_canonicalizes_polluted_alt_entities`
  - `tests::desktop_image_proxy_forwards_browser_like_headers`
  - `bootstrap::imp::tests::localize_entry_on_demand_rewrites_current_entry_html`
  - `fetch::client::body_asset_localizer::tests::localize_html_images_falls_back_to_live_remote_sources_when_html_is_too_large`
  - `fetch::client::image_html::tests::normalize_html_for_live_display_promotes_lazy_sources_and_resolves_relative_urls`
  - `fetch::client::image_html::tests::normalize_html_for_live_display_canonicalizes_polluted_alt_entities`
  - `fetch::client::image_html::tests::document_uses_wordpress_display_src_instead_of_full_size_srcset_candidate`
- `cargo test -p rssr-infra normalize_html_for_live_display_preserves_existing_alt_entities -- --nocapture`：
  - 当前机器在跑 `rssr-infra` integration tests 时会间歇出现 Windows 页文件 / `rlib` 映射异常，已改用 `cargo test -p rssr-infra --lib ...` 验证本次修复对应的库内回归测试。

### 手工验收

- 桌面端打开真实远端含 `picture/srcset/lazy` 正文的文章：未执行
- Android 打开真实远端含响应式图片的正文：未执行
- Web 端回归阅读页渲染：部分执行
  - `dx build --platform web --package rssr-app`：通过
  - Chrome MCP 打开 `http://127.0.0.1:8097/`：通过，Web bundle 与本地 auth 门禁可正常启动
  - 已补齐 `reader-demo` 静态 seed 到当前 browser storage schema：
    - `reader_demo_core.json` 现改为 entry index 形状，不再内嵌正文
    - 新增 `reader_demo_entry_content.json`
    - `scripts/run_web_spa_regression_server.sh` 现同步写入 `ENTRY_CONTENT_STORAGE_KEY`
    - 已确认时间字段必须继续使用当前 browser-state 既有的 `OffsetDateTime` JSON 线格式（例如 `2026-04-10 00:00:00.0 +00:00:00`），不能直接换成 RFC3339
  - 由于 Chrome MCP 本身遇到 profile 锁，最终 reader-demo smoke 退回到同源 helper + headless Chrome：
    - 新 profile 下通过同源 helper 注入 reader-demo 状态后，entries 页 DOM 已包含 `Demo Feed`、`Demo Entry Two`、`Demo Entry One`
    - 说明 Web 端静态 smoke 的 seed 数据已恢复可用

## 结果

- 本次交付已把 NVIDIA / WordPress 这类“正文里夹大量 emoji 小图 + 响应式大图”的真实失败模式纳入修复。
- 本次交付已补上“大正文跳过本地化时，仍保留远端实时加载”的退化路径。
- 本次交付进一步把桌面端远端图片加载从“WebView 直接热链”改成“native 主机代理拉取”，以覆盖仍然存在的站点兼容性问题。
- 本次交付可继续进入真实源手工回归。
- Web 平台仍不承诺绕过浏览器 CORS/站点策略；当前修复主要改善 native 端用户可见失败率。
- Web 侧代码已同步编译通过；但静态 `reader-demo` 冒烟基建仍需跟进到当前 browser storage schema，才能稳定覆盖 `/entries/:id` reader 路径。
- Web 侧代码已同步编译通过；静态 `reader-demo` seed 也已修回当前 browser storage schema，至少可稳定把 Web 端拉起到真实 entries 内容。

## 风险与后续事项

- 当前仍未处理 CSS `background-image`、运行时 JS 懒加载、登录态图片、强防盗链图片。
- `picture` 块当前会把选中的单个最终资源回写到整组标签，优先保证“能显示”，不保证响应式多源策略完全保真。
- 还缺一轮基于真实 feed 的手工 smoke，尤其是此前出现坏图的 NVIDIA/新闻站点类正文。
- 如果后续仍想在本机用 Windows + bash 管 `run_web_spa_regression_server.sh`，建议继续观察行尾与 `BASH_SOURCE` 路径推导；本轮已补 `RSSR_REPO_ROOT` override，但当前 WSL `/home/develata/gitclone/RSS-Reader` 与 Windows `E:\gitclone\RSS-Reader` 并非同一工作树，直接拿 WSL 路径起 helper 仍可能读到旧文件。
- 当前静态 smoke 已能稳定覆盖 entries 内容，但还没有在同一条 headless/browser 会话里把 `/entries/2` reader 路由固化成自动回归步骤。
- 如果桌面端代理后仍有坏图，优先查看 `rssr_app` 主进程日志里的代理告警；这时根因应已收敛为上游状态码、返回类型异常或更强的站点策略。
- 如果再看到 `&amp;amp;` 级联文本，优先检查是否有新的 HTML 重写路径绕过了“原始序列化值保留”逻辑。
- 如果后续仍有少量坏图，需要优先检查日志里是否是：
  - `too_large`
  - `http_status`
  - `non_image_content_type`
  - 站点级防盗链 / cookie 依赖

## 给下一位 Agent 的备注

- 优先从 `crates/rssr-infra/src/fetch/client/image_html.rs`、`crates/rssr-app/src/pages/reader_page/support.rs` 和 `crates/rssr-app/src/main.rs` 看本次链路。
- 如果继续收敛真实站点兼容性，先补桌面端手工 smoke，再决定是否需要为 cookie / anti-hotlink 站点单独设计更强的 host 能力。

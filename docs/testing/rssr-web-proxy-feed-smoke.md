# `rssr-web` 代理 Feed Smoke

这份说明服务于 `rssr-web` 部署壳下，真实 feed 代理链路的固定回归。

它不是完整的“浏览器里添加订阅并完成首次刷新”自动化。
它先解决更窄但关键的一层：

- 登录后
- 请求同源 `/feed-proxy`
- 代理到一个真实外部 feed
- 返回的确实是 XML feed，而不是登录页或静态壳

## 它在整个 browser refresh 链路中的位置

browser 端 refresh 可以粗分成两半：

- source-side
  - 发起请求
  - 判断是否要从 proxy fallback 到 direct
  - 识别登录页 / HTML shell
  - 解析 XML feed
  - 产出 `Updated / NotModified / Failed`
- store-side
  - 更新 browser state
  - 写回 `last_fetched_at` / `last_success_at` / `fetch_error`
  - upsert entries

这份 `/feed-proxy` smoke 只验证 source-side 最前面的部署壳链路：

- `rssr-web` 是否成功接管同源 `/feed-proxy`
- 登录态是否正确进入代理路径
- 代理结果是否真的是 XML feed

它不覆盖：

- browser state 写回
- entries 是否成功落入本地状态
- UI 上的 feed card / entries page 跳转

这些由 `rssr-web browser feed smoke` 和 wasm refresh harness 继续覆盖。

## 脚本

- [run_rssr_web_proxy_feed_smoke.sh](../../scripts/run_rssr_web_proxy_feed_smoke.sh)

## 不可信文档隔离回归

确定性验收使用 [feed_proxy_isolation.mjs](../../scripts/browser/feed_proxy_isolation.mjs)，复用现有 CDP 驱动，无 npm 依赖。先准备真实 Web bundle，再运行：

```bash
cargo test --locked -p rssr-web --no-run
RSSR_PROXY_TEST_PUBLIC_DIR=target/dx/rssr-app/release/web/public \
CHROME_BIN=google-chrome \
node scripts/browser/feed_proxy_isolation.mjs target/feed-proxy-isolation
```

Windows PowerShell 使用 `$env:RSSR_PROXY_TEST_PUBLIC_DIR` 和 `$env:CHROME_BIN` 设置同名变量。可沿用 `CARGO_TARGET_DIR`；将 `TEMP`/`TMP` 指向 E 盘测试目录可避免新增临时构建占用 C 盘。

- Rust `cfg(test)` 夹具只监听 `127.0.0.1:0`，注入固定 `reqwest::Response` 到生产的有界正文读取与响应构造函数；使用真实登录、`require_auth` 和 `session-probe`。生产 binary 没有这些路由、夹具或 SSRF 例外。
- 每次使用新 profile、新认证文件和专用 localStorage 键。只请求本地服务；CDP 阻止外部地址，客户端不能靠 direct fallback 掩盖代理失败。不使用个人 RSS 数据。
- 无隔离的 HTML/SVG/XHTML 正向对照必须执行同一无害脚本。修复后的 HTML、SVG、XHTML、XML、缺 MIME 和 404 HTML 顶层文档必须不执行脚本，且 localStorage 读写抛出 `SecurityError`；iframe 也必须隔离。
- 对注入响应的 fetch 保留 MIME/charset、正文、最终 URL、ETag、Last-Modified、404/304；8 MiB 超限仍返回 502，真实私网校验仍返回 400。这里的 304 仅覆盖 production response builder 和测试服务 HTTP 输出，不覆盖 resolve/fetch 全链：现有 `fetch_proxied_feed` 将所有 3xx 当作重定向，真实上游 304 缺少 `Location` 时会报错并返回 502。此既有行为未在本批修改。真实 WASM UI 验证 RSS、Atom、Latin-1 和注入最终 URL 后相对链接 HTML 发现的添加、实际代理刷新及文章列表展示。
- `--expect-vulnerable` 仅用于修复前对照：要求同一生产构造路径执行脚本并读改测试存储。不要在 CI 中使用此开关。
- 结果写入输出目录的 `run-*/result.json`、日志和截图。CI 下载同次构建的 `ci-web-public` 并执行固定后的模式；不上传 profile 或测试认证文件。

策略依据：[CSP sandbox](https://www.w3.org/TR/CSP3/#directive-sandbox) 和 [HTML sandbox origin/scripts 约束](https://html.spec.whatwg.org/multipage/browsers.html#sandboxing)。保留原始 MIME 是因为 `subscription_probe` 的 HTML 分类、`feed_body` 的字符集解码以及 browser refresh 的登录壳识别均使用它；`x-rssr-final-url` 用于相对候选链接解析。`nosniff` 是补充约束，不能独自阻止正确标记为 HTML/SVG 的文档执行。普通 fetch 消费的是数据，不会把响应的文档 sandbox 应用到调用方页面。

## 最短用法

```bash
bash scripts/run_rssr_web_proxy_feed_smoke.sh
```

默认会验证：

- `https://www.ruanyifeng.com/blog/atom.xml`

## 常用参数

```bash
bash scripts/run_rssr_web_proxy_feed_smoke.sh --skip-build
bash scripts/run_rssr_web_proxy_feed_smoke.sh --port 18082
bash scripts/run_rssr_web_proxy_feed_smoke.sh --feed-url https://github.blog/feed/
```

## 验收重点

- 登录请求成功并拿到会话
- `/feed-proxy` 返回 `200`
- `content-type` 不是 HTML
- body 看起来是 XML feed：
  - `<feed`
  - 或 `<rss`
  - 或 `<rdf:RDF`
- body 不是登录页，不是静态壳

## browser refresh source-side 契约

对应实现入口：

- [feed_request.rs](../../crates/rssr-infra/src/application_adapters/browser/feed_request.rs)
- [feed_response.rs](../../crates/rssr-infra/src/application_adapters/browser/feed_response.rs)
- [feed.rs](../../crates/rssr-infra/src/application_adapters/browser/feed.rs)
- [adapters/refresh.rs](../../crates/rssr-infra/src/application_adapters/browser/adapters/refresh.rs)

### 请求顺序

- 有 browser origin 时，优先请求同源 `/feed-proxy`
- 然后再尝试 direct URL
- 对 HTTP / HTTPS direct 请求，会附加 `_rssr_fetch=<timestamp>` 破缓存参数

### fallback 条件

proxy 响应满足下列条件之一时，会继续回退到 direct：

- `404`
- `401`
- `403`
- `405`
- `400`
- 返回成功状态，但看起来像登录页或 SPA shell

### source outcome 映射

- 请求阶段直接失败：
  - 输出 `Failed`
  - message 前缀：`抓取订阅失败:`
  - 常见原因：
    - 目标站点未开放 CORS
    - 当前部署未启用 feed 代理
    - 网络不可达
- HTTP `304`：
  - 输出 `NotModified`
  - 保留 `etag / last_modified`
- 非成功 HTTP 状态：
  - 输出 `Failed`
  - message 前缀：`feed 抓取返回非成功状态:`
  - 保留 `etag / last_modified`
- 返回 HTML body：
  - 输出 `Failed`
  - message 前缀：`解析订阅失败:`
  - 内层错误会指出“当前响应不是 XML feed，而是 HTML 页面”
- XML 解析失败：
  - 输出 `Failed`
  - message 前缀：`解析订阅失败:`
  - 内层错误会指出 feed 解析失败

## 失败分诊顺序

若 browser refresh 失败，建议按下面顺序排：

1. 先看 `/feed-proxy` smoke
   - 如果这里都失败，优先怀疑 `rssr-web` 部署壳、登录态或 proxy 本身
2. 再看 `rssr-web browser feed smoke`
   - 如果 feed card 已进入 `data-refresh-state="failed"`，直接看 `data-fetch-error`
3. 再区分 source-side 失败类型
   - `抓取订阅失败:`：
     先看 CORS / 网络 / proxy 是否启用
   - `feed 抓取返回非成功状态:`：
     先看 upstream 状态码和代理返回链路
   - `解析订阅失败:`：
     先判断返回的是 HTML shell 还是坏 XML
4. 最后才怀疑 store-side
   - 若 source-side 已 `Updated / NotModified`，但 UI 仍不对，再转查 browser state / entries 写回

## 结果记录

脚本会生成：

- `target/rssr-web-proxy-feed-smoke/<timestamp>/summary.md`

建议在模板里补：

- `/login`
- 登录
- `/feed-proxy`
- content-type
- XML body
- 是否通过
- 若失败，明确属于：
  - network / CORS
  - proxy shell / login shell
  - non-success status
  - parse failure

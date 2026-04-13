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

- [run_rssr_web_proxy_feed_smoke.sh](/home/develata/gitclone/RSS-Reader/scripts/run_rssr_web_proxy_feed_smoke.sh)

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

- [feed_request.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-infra/src/application_adapters/browser/feed_request.rs)
- [feed_response.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-infra/src/application_adapters/browser/feed_response.rs)
- [feed.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-infra/src/application_adapters/browser/feed.rs)
- [adapters/refresh.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-infra/src/application_adapters/browser/adapters/refresh.rs)

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

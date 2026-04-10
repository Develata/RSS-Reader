# `rssr-web` 代理 Feed Smoke

这份说明服务于 `rssr-web` 部署壳下，真实 feed 代理链路的固定回归。

它不是完整的“浏览器里添加订阅并完成首次刷新”自动化。
它先解决更窄但关键的一层：

- 登录后
- 请求同源 `/feed-proxy`
- 代理到一个真实外部 feed
- 返回的确实是 XML feed，而不是登录页或静态壳

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

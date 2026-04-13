# `rssr-web` 浏览器自动 Feed Smoke

这份说明服务于 `rssr-web` 部署壳下，真实浏览器态的“登录后添加订阅并完成首次刷新”自动化回归。

它和 [rssr-web 浏览器手工 Smoke](./rssr-web-browser-smoke.md) 的区别是：

- 手工 smoke 依赖人工登录、输入 feed、点击刷新
- 这条 smoke 用同源 helper 自动完成：
  - 建立登录态
  - 打开 `/feeds`
  - 添加固定 feed fixture
  - 刷新订阅
  - 进入订阅文章页

## 脚本

- [run_rssr_web_browser_feed_smoke.sh](/home/develata/gitclone/RSS-Reader/scripts/run_rssr_web_browser_feed_smoke.sh)

## 最短用法

```bash
bash scripts/run_rssr_web_browser_feed_smoke.sh
```

默认行为：

- 构建 `rssr-app` web bundle
- 启动启用 smoke helper 的 `rssr-web`
- 打开 `http://127.0.0.1:<port>/__codex/browser-feed-smoke`
- 自动完成：
  - 登录
  - 在 `/feeds` 添加 `http://127.0.0.1:<port>/__codex/feed-fixture.xml`
  - 点击 `refresh-feed`
  - 点击 `feed-entries`
- 产出：
  - `browser-feed-smoke.html`
  - `browser-feed-smoke.png`
  - `chrome.log`
  - `summary.md`

## 常用参数

```bash
bash scripts/run_rssr_web_browser_feed_smoke.sh --skip-build
bash scripts/run_rssr_web_browser_feed_smoke.sh --port 18089
bash scripts/run_rssr_web_browser_feed_smoke.sh --release
```

## 固定通过条件

- DOM 中有：
  - `data-smoke="rssr-web-browser-feed-smoke"`
  - `data-result="pass"`
- 自动添加的 feed 标题为：
  - `Codex Smoke Feed`
- 自动进入的订阅文章页中包含：
  - `Codex Smoke Entry`

## 覆盖边界

这条 smoke 使用同源 `__codex/feed-fixture.xml`，主要验证：

- 登录后的 browser UI 自动流程
- browser refresh source -> store -> UI 的成功路径
- feed card 诊断属性与 entries page 跳转

它不验证真实外部站点的 network / CORS failure。

真实外部 feed 与 `/feed-proxy` 部署壳链路由 [`rssr-web` 代理 Feed Smoke](./rssr-web-proxy-feed-smoke.md) 覆盖。纯静态 Web 的 CORS 限制归入 [环境限制索引](./environment-limitations.md)。

## 结果记录

脚本会生成：

- `target/rssr-web-browser-feed-smoke/<timestamp>/summary.md`

建议补：

- 最终路径
- 是否通过
- 是否需要再补手工真实远端 feed 验证

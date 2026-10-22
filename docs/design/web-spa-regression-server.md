# Web 回归服务：服务器模式

2026-10-22 更新；保留旧文件名以兼容引用。正式 Web 已改用同源 `/api/v1`，由 `rssr-web` 提供认证、资料库与静态页面的 SPA 回退。旧 Python 静态服务、本地口令与浏览器文章库注入 helper 已退役。

## 当前入口

```bash
# 自动回归：新资料库、独立 feed 服务、真实登录、结束后清理自己的进程
bash scripts/run_server_web_regression.sh --suite regression
bash scripts/run_server_web_regression.sh --suite acceptance --long

# 发布前聚合门禁与三个固定套件；结束后退出
bash scripts/run_release_ui_regression.sh --with-fixed-smokes

# 手工检查：打印临时凭据与地址，服务保持运行供浏览器访问
bash scripts/run_rssr_web_browser_smoke.sh
```

构建标识、fixture、参数与日志契约见 [服务器浏览器回归](../testing/server-web-regression.md)。每次正式 Web 构建都需要匹配的服务器；`dx serve` 自身不提供 `/api/v1`。仅 Pages 演示可用 `dx serve --platform web --package rssr-app --features pages-demo --locked` 独立运行，见 [Pages 边界](../deployment/github-pages.md)。

## 手工路径

1. 用隔离资料库启动服务，打开 `/login` 并输入本轮测试凭据。
2. 检查 `/entries`、`/feeds`、`/settings`，添加订阅后打开真实文章。
3. 检查登录、退出、重新加载、主题与页面错误；结果写入本轮 handoff。

不要在日常资料库或已有浏览器 profile 中播种测试数据。自动测试的 SQLite 夹具通过 `browser_fixture_library` 写入全新目录，不能用旧浏览器 storage key 替代服务器数据。

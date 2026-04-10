# `rssr-web` 浏览器手工 Smoke

这份说明服务于发布前或部署前，对 `rssr-web` 部署壳做真实浏览器态回归。

它和 [发布前 UI 回归清单](./release-ui-regression-checklist.md) 的关系是：

- 发布前 UI 回归清单定义“应该检查什么”
- 这份说明提供一条固定脚本，解决“怎么快速起一个可登录的 `rssr-web` 回归环境”

## 脚本

- [run_rssr_web_browser_smoke.sh](/home/develata/gitclone/RSS-Reader/scripts/run_rssr_web_browser_smoke.sh)

## 最短用法

```bash
bash scripts/run_rssr_web_browser_smoke.sh
```

默认行为：

- 构建 `rssr-app` web bundle
- 用临时凭据启动 `rssr-web`
- 等待 `/healthz` ready 后再打印可访问地址
- 打印：
  - 本地 URL
  - 用户名 / 密码
  - 日志文件路径
  - `summary.md` 结果模板路径

默认地址：

- `http://127.0.0.1:18081`
- 默认推荐 feed：
  - `https://www.ruanyifeng.com/blog/atom.xml`

## 常用参数

```bash
bash scripts/run_rssr_web_browser_smoke.sh --skip-build
bash scripts/run_rssr_web_browser_smoke.sh --port 18082
bash scripts/run_rssr_web_browser_smoke.sh --release
bash scripts/run_rssr_web_browser_smoke.sh --feed-url https://example.com/feed.xml
```

## 固定手工步骤

脚本启动后，按下面顺序检查：

1. 打开 `/login`。
2. 用脚本打印的临时用户名和密码登录。
3. 进入 `/feeds`。
4. 在 `data-field="feed-url-input"` 输入推荐 feed。
5. 点击 `data-action="add-feed"`。
6. 确认页面出现新的 feed 卡片，且卡片标题链接带有 `data-nav="feed-entries"`。
7. 点击该卡片上的 `data-action="refresh-feed"`。
8. 如果页面出现文章，点击 `data-nav="feed-entries"` 进入文章页；如能进入阅读页，再补看 `/reader`。
9. 打开 `/settings`，确认设置页在登录态下正常可达。
10. 打开 `/logout`，确认会回到 `/login`。

## 固定 selector / 期望

- `data-field="feed-url-input"`：订阅输入框
- `data-action="add-feed"`：添加订阅
- `data-action="refresh-feed"`：刷新单个订阅
- `data-nav="feed-entries"`：进入该订阅文章页

建议期望：

- 登录后 `/feeds` 可达
- 添加 feed 后，新的 feed 卡片可见
- 刷新后不应回到登录页，也不应出现明显错误壳
- `/settings` 可达
- `/logout` 后回到 `/login`

## 为什么当前仍是手工 smoke

当前这条链路没有收成固定浏览器自动化，不是因为页面接口不稳定，而是因为：

- 公开 selector 已稳定
- 但当前仓库环境里的 Chrome MCP / DevTools 连接不稳定
- 因此“真实浏览器里添加订阅并完成首次刷新”这条路径，暂时仍保留为固定手工 smoke

也就是说：

- 这条回归的入口、步骤、selector、推荐 feed 都已经固定
- 还没固定下来的只是浏览器自动操作本身

## 结果记录

脚本会自动生成：

- `target/rssr-web-browser-smoke/<timestamp>/summary.md`

建议直接在这份模板上补：

- 登录页结果
- `feed-url-input / add-feed / refresh-feed / feed-entries` 结果
- `/settings` 结果
- `/logout` 结果
- 代理 feed 结果
- console 结果
- 是否通过

## Ready 约定

脚本现在会先等 `http://127.0.0.1:<port>/healthz` 变成 `200`，然后才打印：

- URL
- 用户名 / 密码
- summary 路径

如果启动失败，会直接报错并提示查看对应 `rssr-web.log`，避免刚起进程时浏览器立刻撞上 `502`。

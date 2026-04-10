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

## 常用参数

```bash
bash scripts/run_rssr_web_browser_smoke.sh --skip-build
bash scripts/run_rssr_web_browser_smoke.sh --port 18082
bash scripts/run_rssr_web_browser_smoke.sh --release
```

## 推荐检查项

至少检查：

- `/login`
- 登录后 `/feeds`
- 登录后 `/settings`
- `/logout`

如果环境允许，再补：

- 至少 1 个需要代理的 feed 导入
- feed 首次刷新
- 导入后进入 `/entries` 或 `/reader`

## 结果记录

脚本会自动生成：

- `target/rssr-web-browser-smoke/<timestamp>/summary.md`

建议直接在这份模板上补：

- 登录页结果
- `/feeds` 结果
- `/settings` 结果
- 登出结果
- 代理 feed 结果
- console 结果
- 是否通过

## Ready 约定

脚本现在会先等 `http://127.0.0.1:<port>/healthz` 变成 `200`，然后才打印：

- URL
- 用户名 / 密码
- summary 路径

如果启动失败，会直接报错并提示查看对应 `rssr-web.log`，避免刚起进程时浏览器立刻撞上 `502`。

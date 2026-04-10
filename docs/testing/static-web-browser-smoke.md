# Static Web 浏览器手工 Smoke

这份说明服务于纯静态 `rssr-app` Web 入口的真实浏览器态回归。

它解决的是：本地 Web 门禁默认要求先初始化用户名/密码，手工每次点一遍比较慢，也不利于固定回归路径。

## 脚本

- [run_static_web_browser_smoke.sh](/home/develata/gitclone/RSS-Reader/scripts/run_static_web_browser_smoke.sh)

## 最短用法

```bash
bash scripts/run_static_web_browser_smoke.sh
```

默认行为：

- 启动带 SPA fallback 的静态 Web 服务
- 提供同源 helper URL
- helper 会自动写入本地 `localStorage/sessionStorage`
- 然后跳转到默认内部页 `/entries`

如果要稳定进入真实阅读页，也可以直接播种一份最小 demo 数据：

```bash
bash scripts/run_static_web_browser_smoke.sh --seed reader-demo --next /entries/2
```

这会额外写入一份最小浏览器状态，确保：

- `/entries`
- `/feeds`
- `/settings`
- `/entries/2`

都能在 fresh profile 下直接进入真实应用内部页。

## helper URL

脚本会打印类似：

```text
http://127.0.0.1:8091/__codex/setup-local-auth?username=smoke&password=smoke-pass-123&next=/entries
```

打开这个 URL 后：

- 本地 Web 门禁会被初始化
- 会话会直接写入 `sessionStorage`
- 浏览器会自动跳到目标页

## 推荐检查项

- helper 自动跳转是否成功
- `/entries`
- 内部导航到 `/feeds`
- 内部导航到 `/settings`
- 如使用 `--seed reader-demo`，再补 `/entries/2`
- 刷新当前页后是否仍保持已登录

## 结果记录

脚本会生成：

- `target/static-web-browser-smoke/<timestamp>/summary.md`

建议直接在模板里补：

- helper 结果
- `/entries` 结果
- `/feeds` 结果
- `/settings` 结果
- 刷新保持登录结果
- console 结果
- 是否通过

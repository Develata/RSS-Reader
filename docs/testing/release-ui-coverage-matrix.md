# 发布前 UI 覆盖矩阵

这份矩阵用于回答两个问题：

- 当前发布前 UI 回归，**已经固定覆盖了什么**
- 现在还剩哪些项，**仍然需要手工浏览器回归**

它不是执行清单本身。
执行入口仍以 [发布前 UI 回归清单](./release-ui-regression-checklist.md) 为准。

## 口径

状态分三类：

- `自动化`：可在本地或 CI 中直接执行，结果可重复
- `固定 smoke`：已有固定脚本/固定入口，但仍偏 smoke 或需人工看产物
- `手工`：当前仍主要依赖人工浏览器回归

优先级分三类：

- `P1`：发布前必须确认
- `P2`：建议发布前确认
- `P3`：按风险补充

## 覆盖矩阵

| 能力项 | 当前状态 | 入口 | 优先级 | 备注 |
| --- | --- | --- | --- | --- |
| `rssr-app` Web 构建与单测 | 自动化 | `bash scripts/run_release_ui_regression.sh --no-serve` | P1 | 已串行覆盖 `cargo check/test` |
| builtin theme 契约 | 自动化 | `cargo test -p rssr-app --test test_builtin_theme_contracts` | P1 | 防止内置主题回退到旧 selector |
| `rssr-infra` host + browser contract harness | 自动化 | `bash scripts/run_release_ui_regression.sh --no-serve --with-browser-contracts` | P1 | 串行覆盖 refresh / subscription / config exchange 的 sqlite 与 wasm/browser harness |
| `rssr-web` 单测 | 自动化 | `bash scripts/run_release_ui_regression.sh --no-serve --with-rssr-web` | P1 | 已纳入统一预检 |
| 静态 Web `/entries` / `/feeds` / `/settings` 路由可达 | 固定 smoke | `bash scripts/run_static_web_browser_smoke.sh` | P1 | 依赖同源 local auth helper |
| 静态 Web 真实阅读页 `/entries/2` | 固定 smoke | `bash scripts/run_static_web_browser_smoke.sh --seed reader-demo --next /entries/2` | P1 | 已固定 demo seed |
| 静态 Web `/reader` 多主题矩阵 | 固定 smoke | `bash scripts/run_release_ui_regression.sh --with-fixed-smokes --no-serve` 或 `bash scripts/run_static_web_reader_theme_matrix.sh` | P1 | 默认主题 + 4 个内置主题 |
| 静态 Web 小视口关键路径 | 固定 smoke | `bash scripts/run_release_ui_regression.sh --with-fixed-smokes --no-serve` 或 `bash scripts/run_static_web_small_viewport_smoke.sh` | P1 | 默认 `390x844`，覆盖 `/entries` `/feeds` `/settings` `/entries/2` |
| `rssr-web` 登录 / 会话 / `/feeds` `/settings` 基础壳 | 自动化 | `bash scripts/run_release_ui_regression.sh --with-rssr-web` | P1 | 已覆盖登录、`/session-probe`、登出 |
| `rssr-web` 代理链路 `/feed-proxy` 返回真实 XML | 固定 smoke | `bash scripts/run_release_ui_regression.sh --with-fixed-smokes --no-serve` 或 `bash scripts/run_rssr_web_proxy_feed_smoke.sh` | P1 | 当前默认验证阮一峰 Atom |
| 静态 Web `/reader` 多主题下的视觉细节 | 固定 smoke + 手工结论 | `bash scripts/run_release_ui_regression.sh --with-fixed-smokes --no-serve` 或 `bash scripts/run_static_web_reader_theme_matrix.sh` + 查看 `target/static-web-reader-theme-matrix/<ts>/*.png` | P2 | 2026-04-10 基线已人工通过；后续发布仍需复看新产物 |
| 静态 Web 小视口下的视觉细节 | 固定 smoke + 手工结论 | `bash scripts/run_release_ui_regression.sh --with-fixed-smokes --no-serve` 或 `bash scripts/run_static_web_small_viewport_smoke.sh` + 查看 `target/static-web-small-viewport-smoke/<ts>/*.png` | P2 | 2026-04-10 基线已人工通过；后续发布仍需复看新产物 |
| `rssr-web` 浏览器态下真实添加订阅并完成首次刷新 | 固定 smoke | `bash scripts/run_release_ui_regression.sh --with-fixed-smokes --no-serve` 或 `bash scripts/run_rssr_web_browser_feed_smoke.sh` | P2 | 现已用同源 helper + 本地 feed fixture 自动化 |
| `rssr-web` 浏览器态下真实代理 feed 导入后的页面更新 | 手工 | `bash scripts/run_rssr_web_browser_smoke.sh` | P2 | 公开 selector 已稳定，当前仍主要受限于本地 Chrome MCP / DevTools 连接不稳定 |
| WebDAV 上传/下载 UI 实页回归 | 手工 | 发布前清单 + 浏览器手工 | P2 | 自动化更多停留在 lower-level gates |
| 多主题下 `/entries` `/feeds` `/settings` 的视觉细节 | 手工 | 发布前清单 + 浏览器手工 | P2 | 内置主题契约已自动化，但视觉仍建议 spot check |
| 真实远端 feed 首次刷新后的 `/entries` / `/reader` 浏览器态 | 手工 | 发布前清单 + 浏览器手工 | P2 | 受远端源波动和 CORS/代理形态影响 |
| 小视口下 `rssr-web` 部署壳登录后路径 | 手工 | `bash scripts/run_rssr_web_browser_smoke.sh` + 手工调视口 | P3 | 当前小视口 smoke 只固定了静态 Web |

## 当前结论

当前发布前回归已经把这几类 **P1** 能力固定下来：

- `rssr-app` / `rssr-web` / `rssr-infra` 的核心自动化门禁
- 静态 Web 的真实内部页入口
- 静态 Web 的 `/reader` 多主题矩阵
- 静态 Web 的小视口关键路径
- `rssr-web` 的基础登录壳
- `rssr-web` 的真实 `/feed-proxy` 代理链路

所以现在的主要缺口，不再是“没有固定入口”，而是：

- 仍有少量 **浏览器视觉判断** 需要人工看截图或实机页面
- `rssr-web` 浏览器态下对真实远端 feed 的首刷行为仍主要靠手工补充
- 当前 deploy-shell 自动化已经覆盖了同源 fixture feed 的登录后添加与刷新链路

## 推荐发布前顺序

1. 先跑：

```bash
bash scripts/run_release_ui_regression.sh --debug --port 8091 --with-rssr-web --with-browser-contracts --with-fixed-smokes
```

2. 如果只想分步执行固定 smoke，再跑：

```bash
bash scripts/run_static_web_reader_theme_matrix.sh
bash scripts/run_static_web_small_viewport_smoke.sh
bash scripts/run_rssr_web_proxy_feed_smoke.sh
bash scripts/run_rssr_web_browser_feed_smoke.sh
```

3. 最后补最少量人工浏览器确认：

- 看多主题 `/reader` 截图是否可接受
- 看小视口截图是否可接受
- 如本次发布涉及真实远端 feed 行为，再按固定 selector 手工补一次 `rssr-web` 浏览器态真实添加订阅

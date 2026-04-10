# 2026-04-10 交接汇总

- 日期：2026-04-10
- 作者 / Agent：Codex
- 分支：`main`
- 状态：`active`
- commit：`pending`

## 工作摘要与背景

- 继续推进 `headless active interface + CSS 完全分离 + infra`。
- 本轮重点不是继续大面积改页面逻辑，而是：
  - 继续识别残留的 class 驱动视觉规则
  - 固化一条本地可复用的 web 静态构建 + SPA fallback 回归路径
  - 把 `app-nav`、`entry-directory-rail`、`reader-page`、`web-auth` 这几块壳层从 class 驱动进一步收成稳定语义接口

## 受影响模块与平台

- 前端样式与设计文档
  - `assets/styles/*`
  - `docs/design/*`
- 回归脚本
  - `scripts/run_web_spa_regression_server.sh`
- 本地浏览器回归
  - Web / Chrome MCP

## 关键变更

### 1. 新增本地 SPA fallback 回归脚本

- 新增：
  - [run_web_spa_regression_server.sh](/home/develata/gitclone/RSS-Reader/scripts/run_web_spa_regression_server.sh)
  - [web-spa-regression-server.md](/home/develata/gitclone/RSS-Reader/docs/design/web-spa-regression-server.md)
- 作用：
  - 读取 `target/dx/rssr-app/<profile>/web/public`
  - 为未知路径自动回退到 `index.html`
  - 让 `/entries`、`/feeds`、`/settings` 的本地静态构建回归不再落到 404

### 2. CSS 完全分离基线检查继续收口

- 更新：
  - [css-separation-baseline-checklist.md](/home/develata/gitclone/RSS-Reader/docs/design/css-separation-baseline-checklist.md)
  - [README.md](/home/develata/gitclone/RSS-Reader/docs/design/README.md)
- `app-nav` 已继续迁移：
  - 导航壳、显隐按钮、品牌区、链接区、搜索区都已改成 `data-layout` / `data-slot` / `data-nav`
  - 页面结构中不再保留只为样式服务的 `app-nav*` class
- `entry-directory-rail` / `entry-top-directory` 已继续迁移：
  - 目录栏容器、导航树、section/children/grandchildren、toggle、顶部目录 chip 都已改成 `data-layout`
  - 目录标题、条目标题、条目元信息已统一到 `data-slot`
  - 月份/日期目录跳转已补 `data-nav="entry-directory-*"`
  - 来源目录展开态已补 `data-state="expanded|collapsed"`
- `reader-page` 外壳已继续迁移：
  - 阅读页根、header、toolbar、meta block、body、pagination、bottom bar 都已改成 `data-layout`
  - 标题、meta、底部栏图标/标签已统一到 `data-slot`
  - 同订阅上下文分页已补 `data-context="feed"`
- `web-auth` 本地门禁壳已继续迁移：
  - shell、card、brand、form 都已改成 `data-layout`
  - 品牌 mark / name、title、intro、note 都已统一到 `data-slot`
  - 页面结构中不再保留只为样式服务的 `web-auth-*` class
- `entry-filters` 筛选布局已继续迁移：
  - 根容器、toggle、sources、source-grid 都已补 `data-layout`
  - sources label 已补 `data-slot="entry-filters-sources-label"`
  - `.page` / `.page-header` 当前保留为通用壳类，不继续作为高优先级槽化目标
- 当前剩余最值得继续专项审计的区域已收敛为：
  - 通用布局 class 与允许保留的内容岛边界

### 3. 主题作者与前端接口文档正式对齐到语义接口

- 更新：
  - [theme-author-selector-reference.md](/home/develata/gitclone/RSS-Reader/docs/design/theme-author-selector-reference.md)
  - [frontend-command-reference.md](/home/develata/gitclone/RSS-Reader/docs/design/frontend-command-reference.md)
  - [ui-shell-bus-page-facade.md](/home/develata/gitclone/RSS-Reader/docs/design/ui-shell-bus-page-facade.md)
- 重点：
  - 删掉过时的 `app-nav*`、`reader-page*`、`entry-filters*`、`web-auth*` 契约示例
  - 把主题作者文档改成以 `data-page / data-layout / data-slot / data-nav / data-action / data-field / data-state` 为主
  - 把 `UiCommand / UiRuntime` 的实现路径说明对齐到当前目录结构：
    - `ui/commands/mod.rs`
    - `ui/runtime/mod.rs`
  - 把前端接口说明从“旧 class 列表”收口成“语义接口优先 + 少量通用 class 保留”

### 4. 主题作者 smoke review 暴露了两处真实缺口

- 先前固定脚本的默认 profile 假设有误：
  - `dx build --platform web --package rssr-app` 当前默认产出 `target/dx/rssr-app/debug/web/public`
  - 脚本原先默认读 `release/web/public`，导致清理构建目录后会误报缺失
  - 现已改为默认 `debug`，并在 `--release` 时显式走 release 构建
- fresh web 构建的浏览器回归已确认：
  - `data-layout`
  - `data-slot`
  - `data-page`
  - `data-nav`
  - `data-action`
  - `data-field`
  - `data-state`
  - `data-density`
  这些语义接口都真实落到了 DOM
- 同时静态审计也确认：
  - `assets/themes/*.css` 仍大量依赖旧 class 和旧结构
  - 下一轮更值得收的是“内置主题资产迁移”，而不是继续盲目扩展页面壳 CSS 迁移面

### 5. 内置主题资产已开始对齐新的公开语义契约

- 更新：
  - [atlas-sidebar.css](/home/develata/gitclone/RSS-Reader/assets/themes/atlas-sidebar.css)
  - [newsprint.css](/home/develata/gitclone/RSS-Reader/assets/themes/newsprint.css)
  - [forest-desk.css](/home/develata/gitclone/RSS-Reader/assets/themes/forest-desk.css)
  - [midnight-ledger.css](/home/develata/gitclone/RSS-Reader/assets/themes/midnight-ledger.css)
- 已完成：
  - `app-nav*` 改到 `data-layout="app-nav-*"` / `data-nav`
  - `reader-page*` 改到 `data-layout="reader-*"` / `data-slot="reader-*"`
  - `button.secondary/.danger/.danger-outline` 改到 `data-variant`
  - `theme-card.is-active` 改到 `data-state="active"`
  - `stats-grid`、`settings-grid`、`reader-body` 等关键布局接口切到 `data-layout`
- 继续完成：
  - `feed-card__title / feed-card__meta` -> `data-slot="feed-card-title|feed-card-meta"`
  - `entry-card__title / entry-card__meta` -> `data-slot="entry-card-title|entry-card-meta"`
  - `page-intro` -> `data-slot="page-intro"`
  - `theme-card__title / theme-card__swatches / theme-card__swatch` -> `data-slot="theme-card-*"`
  - 原先未实际使用的 `theme-card__description / theme-card__notes` 已确认为死接口并删除
- 当前判断：
  - 主题示例已经不再依赖高密度旧 selector
  - 下一轮如果继续收，就该转向是否还要保留部分内部组件 class 作为主题公开契约

### 6. 新增内置主题契约测试，防止 selector 回退

- 新增：
  - [test_builtin_theme_contracts.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/tests/test_builtin_theme_contracts.rs)
- 作用：
  - 拦截内置主题重新引入旧 selector 契约
  - 明确要求主题继续引用当前语义接口：
    - `data-layout="app-nav-shell"`
    - `data-nav`
    - `data-variant="secondary"`
    - `data-layout="reader-body"`
- 目标：
  - 让主题资产回归不再依赖手工审查
  - 把 `assets/themes/*.css` 也纳入仓库内的自动化演进边界

### 7. 新增发布前 UI 回归清单，接入 testing 文档索引

- 新增：
  - [release-ui-regression-checklist.md](/home/develata/gitclone/RSS-Reader/docs/testing/release-ui-regression-checklist.md)
- 更新：
  - [README.md](/home/develata/gitclone/RSS-Reader/docs/testing/README.md)
  - [mainline-validation-matrix.md](/home/develata/gitclone/RSS-Reader/docs/testing/mainline-validation-matrix.md)
- 作用：
  - 把发布前 UI 验收与普通手工回归、主线验证矩阵区分开
  - 固定两条 Web 入口：
    - 静态 `rssr-app` + SPA fallback
    - `rssr-web` 部署壳
  - 固定发布门禁关注点：
    - 路由与交互
    - 主题与 CSS 契约
    - 关键持久化与配置交换
    - 小视口与 Console 门禁

### 8. 新增发布前 UI 预检脚本，串起自动化门禁与静态 Web 回归

- 新增：
  - [run_release_ui_regression.sh](/home/develata/gitclone/RSS-Reader/scripts/run_release_ui_regression.sh)
- 更新：
  - [release-ui-regression-checklist.md](/home/develata/gitclone/RSS-Reader/docs/testing/release-ui-regression-checklist.md)
  - [web-spa-regression-server.md](/home/develata/gitclone/RSS-Reader/docs/design/web-spa-regression-server.md)
- 作用：
  - 固定发布前 UI 预检入口
  - 先串行跑：
    - `rssr-app` 编译与测试
    - builtin theme 契约测试
    - `rssr-infra` 关键 contract harness
    - `rssr-web` 测试
  - 自动化门禁通过后，再进入静态 `rssr-app` + SPA fallback 回归服务
  - 当前又继续补了两项：
    - 可选 `rssr-web` 最小部署壳 smoke
    - 自动生成 `summary.md` 结果模板与日志目录
  - `rssr-web` smoke 当前已进一步扩到：
    - 未登录访问 `/entries` 重定向到 `/login`
    - 用临时凭据真实登录
    - 登录后 `/session-probe` 为 `204`
    - 登录后 `/feeds`、`/settings` 为 `200`
    - `/logout` 后回到 `/login`

### 9. 新增 `rssr-web` 浏览器手工 smoke 启动脚本

- 新增：
  - [run_rssr_web_browser_smoke.sh](/home/develata/gitclone/RSS-Reader/scripts/run_rssr_web_browser_smoke.sh)
  - [rssr-web-browser-smoke.md](/home/develata/gitclone/RSS-Reader/docs/testing/rssr-web-browser-smoke.md)
- 更新：
  - [README.md](/home/develata/gitclone/RSS-Reader/docs/testing/README.md)
  - [release-ui-regression-checklist.md](/home/develata/gitclone/RSS-Reader/docs/testing/release-ui-regression-checklist.md)
- 作用：
  - 固定一条真实浏览器态的 `rssr-web` 手工 smoke 启动路径
  - 自动给出临时用户名、密码、日志文件和结果模板
  - 避免每次手工回归时重复拼：
    - `RSS_READER_WEB_*` 环境变量
    - 静态目录路径
    - 临时认证状态文件
  - 已实测：
    - helper 可启动 `rssr-web`
    - `/healthz` 返回 `200`
  - 当前又继续补了一层：
    - helper 会先等待 `/healthz` ready，再打印 URL 与临时凭据
    - 启动失败时直接报错并指向 `rssr-web.log`

### 10. 正式跑了一轮发布前 UI 回归预检

- 自动化 + `rssr-web` 预检：
  - `bash scripts/run_release_ui_regression.sh --no-serve --skip-build --with-rssr-web`
  - 结果：
    - `rssr-app` 自动化门禁通过
    - builtin theme 契约测试通过
    - `rssr-infra` 关键 contract harness 通过
    - `rssr-web` 测试通过
    - `rssr-web` smoke 通过
  - 结果模板：
    - `target/release-ui-regression/20260410-161507/summary.md`
- 静态 Web + SPA fallback：
  - 用前台最小方式起：
    - `bash scripts/run_web_spa_regression_server.sh --skip-build --port 8100`
  - 已确认：
    - `curl -I http://127.0.0.1:8100/entries` 返回 `200`

### 11. 静态 `/reader` 多主题回归与小视口回归已固定成独立 smoke

- 新增：
  - [run_static_web_reader_theme_matrix.sh](/home/develata/gitclone/RSS-Reader/scripts/run_static_web_reader_theme_matrix.sh)
  - [run_static_web_small_viewport_smoke.sh](/home/develata/gitclone/RSS-Reader/scripts/run_static_web_small_viewport_smoke.sh)
  - [static-web-reader-theme-matrix.md](/home/develata/gitclone/RSS-Reader/docs/testing/static-web-reader-theme-matrix.md)
  - [static-web-small-viewport-smoke.md](/home/develata/gitclone/RSS-Reader/docs/testing/static-web-small-viewport-smoke.md)
- 更新：
  - [run_web_spa_regression_server.sh](/home/develata/gitclone/RSS-Reader/scripts/run_web_spa_regression_server.sh)
  - [release-ui-regression-checklist.md](/home/develata/gitclone/RSS-Reader/docs/testing/release-ui-regression-checklist.md)
  - [README.md](/home/develata/gitclone/RSS-Reader/docs/testing/README.md)
  - [web-spa-regression-server.md](/home/develata/gitclone/RSS-Reader/docs/design/web-spa-regression-server.md)
- 作用：
  - 同源 local auth helper 新增 `preset=atlas-sidebar|newsprint|forest-desk|midnight-ledger`
  - 可以把内置主题 CSS 和 `reader-demo` seed 一起播种进浏览器状态
  - `/reader` 多主题回归不再需要手工进设置页切主题
  - 小视口回归不再需要手工拖浏览器尺寸
- 已实跑：
  - `bash scripts/run_static_web_reader_theme_matrix.sh --skip-build --port 8114`
  - `bash scripts/run_static_web_small_viewport_smoke.sh --skip-build --port 8115`
- 结果：
  - 主题矩阵产物落盘到：
    - `target/static-web-reader-theme-matrix/20260410-210009/`
  - 小视口产物落盘到：
    - `target/static-web-small-viewport-smoke/20260410-210009/`
  - 两条 smoke 都通过：
    - 多主题 `/entries/2` 均进入真实阅读页
    - 小视口 `/entries`、`/feeds`、`/settings`、`/entries/2` 均进入真实页面

### 12. `rssr-web` 真实代理 feed 回归已固定成 deploy-shell smoke

- 新增：
  - [run_rssr_web_proxy_feed_smoke.sh](/home/develata/gitclone/RSS-Reader/scripts/run_rssr_web_proxy_feed_smoke.sh)
  - [rssr-web-proxy-feed-smoke.md](/home/develata/gitclone/RSS-Reader/docs/testing/rssr-web-proxy-feed-smoke.md)
- 更新：
  - [README.md](/home/develata/gitclone/RSS-Reader/docs/testing/README.md)
  - [release-ui-regression-checklist.md](/home/develata/gitclone/RSS-Reader/docs/testing/release-ui-regression-checklist.md)
- 目标：
  - 先把 `rssr-web` 部署壳下最关键的代理链路固定成可重复 smoke
  - 验证“登录后请求 `/feed-proxy` 返回真实 XML feed，而不是登录页或静态壳”
  - 当前选择的默认远端 feed：
    - `https://www.ruanyifeng.com/blog/atom.xml`
- 已实跑：
  - `bash scripts/run_rssr_web_proxy_feed_smoke.sh --skip-build --port 18086`
- 结果：
  - 登录成功
  - `/feed-proxy` 返回 `200`
  - body 为真实 XML feed
  - 产物落盘到：
    - `target/rssr-web-proxy-feed-smoke/20260410-210340/`
- 边界说明：
  - 这条 smoke 先覆盖 deploy-shell 代理链路
  - 还不是“浏览器里真实添加订阅并完成首次刷新”的全 UI 自动化

### 13. 发布前 UI 缺口已收成正式覆盖矩阵

- 新增：
  - [release-ui-coverage-matrix.md](/home/develata/gitclone/RSS-Reader/docs/testing/release-ui-coverage-matrix.md)
- 更新：
  - [README.md](/home/develata/gitclone/RSS-Reader/docs/testing/README.md)
  - [release-ui-regression-checklist.md](/home/develata/gitclone/RSS-Reader/docs/testing/release-ui-regression-checklist.md)
- 作用：
  - 把当前发布前回归分成：
    - `自动化`
    - `固定 smoke`
    - `手工`
  - 明确写出：
    - 已固定的 P1 能力
    - 仍然要手工补的 P2 / P3 缺口
- 当前矩阵结论：
  - P1 已基本固定：
    - 自动化门禁
    - 静态 Web 真实内部页
    - 静态 `/reader` 多主题矩阵
    - 静态 Web 小视口关键路径
    - `rssr-web` 登录壳
    - `rssr-web` `/feed-proxy` 代理链路
  - 当前真正剩下的主要缺口是：
    - 多主题与小视口的视觉可接受性，仍需人工看截图
    - `rssr-web` 浏览器态下“真实添加订阅并完成首次刷新”仍未自动化
    - headless Chrome dump DOM 能拿到真实应用入口，而不是浏览器错误页

### 14. `rssr-web` 浏览器态真实添加订阅已收成固定手工契约

- 更新：
  - [run_rssr_web_browser_smoke.sh](/home/develata/gitclone/RSS-Reader/scripts/run_rssr_web_browser_smoke.sh)
  - [rssr-web-browser-smoke.md](/home/develata/gitclone/RSS-Reader/docs/testing/rssr-web-browser-smoke.md)
  - [release-ui-coverage-matrix.md](/home/develata/gitclone/RSS-Reader/docs/testing/release-ui-coverage-matrix.md)
  - [release-ui-regression-checklist.md](/home/develata/gitclone/RSS-Reader/docs/testing/release-ui-regression-checklist.md)
  - [README.md](/home/develata/gitclone/RSS-Reader/docs/testing/README.md)
- 这轮没有继续硬做浏览器自动化，而是先把当前 P2 缺口收成稳定手工 smoke：
  - 固定推荐代理 feed：
    - `https://www.ruanyifeng.com/blog/atom.xml`
  - 固定 selector：
    - `data-field="feed-url-input"`
    - `data-action="add-feed"`
    - `data-action="refresh-feed"`
    - `data-nav="feed-entries"`
  - 固定结果模板：
    - 登录
    - 添加订阅
    - 首次刷新
    - 进入文章页 / 阅读页
    - `/settings`
    - `/logout`
- 结论：
  - 这条缺口现在已经不是“没有入口 / 没有契约”
  - 而是“浏览器自动操作尚未固定”
  - 当前仓库环境里的 Chrome MCP / DevTools 连接不稳定，因此这条链路暂时继续保持固定手工 smoke
- 静态 Web 路由级 DOM smoke：
  - `entries / feeds / settings` 三条路径都用 headless Chrome dump DOM 验证过
  - 当前统一落到本地 Web 门禁壳：
    - `data-layout="web-auth-shell"`
    - `data-slot="web-auth-title"`
    - `初始化 Web 登录`
    - `保存并进入`
  - 这说明：
    - SPA fallback 正常
    - 三条核心路由都能稳定回到同一套本地门禁壳
    - 当前还缺的是“已初始化本地凭据后的真实应用内部页面”浏览器态回归

### 11. 新增静态 Web 浏览器手工 smoke helper

- 新增：
  - [run_static_web_browser_smoke.sh](/home/develata/gitclone/RSS-Reader/scripts/run_static_web_browser_smoke.sh)
  - [static-web-browser-smoke.md](/home/develata/gitclone/RSS-Reader/docs/testing/static-web-browser-smoke.md)
- 更新：
  - [run_web_spa_regression_server.sh](/home/develata/gitclone/RSS-Reader/scripts/run_web_spa_regression_server.sh)
  - [README.md](/home/develata/gitclone/RSS-Reader/docs/testing/README.md)
  - [release-ui-regression-checklist.md](/home/develata/gitclone/RSS-Reader/docs/testing/release-ui-regression-checklist.md)
  - [web-spa-regression-server.md](/home/develata/gitclone/RSS-Reader/docs/design/web-spa-regression-server.md)
- 作用：
  - 固定一条“静态 Web 本地门禁已初始化后的真实浏览器态回归”入口
  - 用同源 helper URL 自动写入：
    - `rssr-web-auth-config-v1`
    - `rssr-web-auth-session-v1`
  - 自动跳转到目标页，避免每次手工先填一遍初始化表单
  - 已实测：
    - helper 脚本可起服务并输出 helper URL
    - `GET /__codex/setup-local-auth?...` 返回 `200`
    - headless Chrome 访问 helper URL 后，最终 DOM 已进入真实 `/entries` 页面
    - 可见：
      - `data-page="entries"`
      - `data-layout="entries-layout"`
      - `data-nav="feeds|entries|settings"`
      - 空状态 banner

### 12. 正式补齐了静态 Web 应用内部页与 `rssr-web` 登录后 smoke

- 静态 Web：
  - 继续使用同源 helper：
    - `/__codex/setup-local-auth?username=...&password=...&next=/entries`
    - `/__codex/setup-local-auth?username=...&password=...&next=/feeds`
    - `/__codex/setup-local-auth?username=...&password=...&next=/settings`
  - headless Chrome 已确认：
    - `/entries` 最终进入真实应用页
    - `/feeds` 最终进入真实订阅页
    - `/settings` 最终进入真实设置页
  - DOM 命中包括：
    - `data-page="entries"`
    - `data-page="feeds"`
    - `data-page="settings"`
    - `data-layout="entries-layout"`
    - `data-layout="feed-workbench-single"`
    - `data-layout="settings-grid"`
- `rssr-web`：
  - 在 helper 拉起的部署壳上补走了一轮真实 cookie 登录流
  - 已确认：
    - `POST /login` 返回 `303`
    - `/session-probe` 为 `204`
    - 登录后 `/feeds` 为 `200`
    - 登录后 `/settings` 为 `200`
    - `/logout` 返回 `303` 到 `/login`
- 当前判断：
  - 发布前回归工具链已经覆盖：
    - 静态 Web 门禁壳
    - 静态 Web 应用内部页
    - `rssr-web` 部署壳登录前后路径
  - 这条线当前剩下的就不是“缺入口”，而是后续是否还要加更细的业务 smoke

### 13. 静态 Web 现已补齐可重复进入真实 `/reader` 的 demo seed

- 新增浏览器状态 fixture：
  - `tests/fixtures/browser_state/reader_demo_core.json`
  - `tests/fixtures/browser_state/reader_demo_app_state.json`
  - `tests/fixtures/browser_state/reader_demo_entry_flags.json`
- 新增 fixture 契约测试：
  - [test_browser_state_seed_contracts.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-infra/tests/test_browser_state_seed_contracts.rs)
- `run_web_spa_regression_server.sh` 的同源 helper 现在支持：
  - `seed=reader-demo`
  - 额外暴露：
    - `/__codex/dump-browser-state`
- `run_static_web_browser_smoke.sh` 现在支持：
  - `--seed reader-demo`
  - 可直接配合：
    - `--next /entries/2`
- 这轮先踩到一个真实问题：
  - 初版 seed 用了 RFC3339 风格时间字符串
  - 但当前 `time::OffsetDateTime` 的 JSON 反序列化实际要求的是：
    - `2026-04-10 00:00:00.0 +00:00:00`
  - fixture 契约测试把这个问题直接卡了出来
- 修正后已确认：
  - helper 会写入真实可用的 `BrowserState`
  - fresh profile 下 headless Chrome 访问：
    - `/__codex/setup-local-auth?...&seed=reader-demo&next=/entries/2`
  - 最终 DOM 可见：
    - `data-page="reader"`
    - `data-layout="reader-page"`
    - `Demo Entry Two`
    - `来源：https://example.com/posts/demo-entry-2`
    - `收藏（F）`
- 额外旁证：
  - seed 后直接访问 `/feeds`
  - 订阅页统计已从 `0 / 0` 恢复到真实 seeded 数据路径
- 进一步收口：
  - 发布前预检脚本 `summary.md` 模板已新增：
    - `静态 reader seed smoke`
  - `run_release_ui_regression.sh` 的静态阶段提示也会直接打印 reader-demo helper URL
  - 发布前回归清单与 testing 索引都已把 `/entries/2` seed 路径提升为正式入口，而不再只是附带技巧

## 已执行的验证 / 验收

- 脚本可执行权限：
  - `chmod +x scripts/run_web_spa_regression_server.sh`
- SPA fallback 回归脚本：
  - `bash scripts/run_web_spa_regression_server.sh --skip-build --port 8092`
- 路径回退检查：
  - `curl -I http://127.0.0.1:8092/feeds`
  - `curl -I http://127.0.0.1:8092/settings`
  - `curl -I http://127.0.0.1:8092/assets/rssr-app-*.js`
- Chrome MCP 回归：
  - 直接访问 `http://127.0.0.1:8092/feeds`
  - 直接访问 `http://127.0.0.1:8092/settings`
  - 已确认不再是静态服务器 404，而是正确回退到 SPA 入口页
- 编译检查：
  - `cargo fmt --all`
  - `cargo check -p rssr-app`
  - `git diff --check`
- 文档一致性复查：
  - `rg -n "\\.app-nav|\\.reader-page|\\.reader-header|\\.reader-toolbar|\\.entry-filters|\\.web-auth-|\\.is-active|\\.is-disabled|\\.button\\.secondary|\\.button\\.danger|\\.button\\.danger-outline" docs/design`
  - `rg -n "commands\\.rs|runtime\\.rs" docs/design`
- 主题作者 smoke review：
  - `rg -n "\\.app-nav|\\.reader-page|\\.reader-header|\\.reader-toolbar|\\.entry-filters|\\.web-auth-|\\.button\\.secondary|\\.button\\.danger|\\.button\\.danger-outline|\\.theme-card\\.is-active|\\.entry-filters__source-chip\\.is-selected|\\.reader-bottom-bar__button\\.is-" assets/themes`
  - `dx build --platform web --package rssr-app`
  - `bash scripts/run_web_spa_regression_server.sh --debug --skip-build --port 8094`
  - Chrome MCP 直接访问 `http://127.0.0.1:8094/entries`、`/settings`
  - 在浏览器中注入只依赖公开接口的最小 CSS，并确认 `app-nav` 与 `settings-grid` 可被稳定覆写
- 主题资产迁移验证：
  - `rg -n "\\.app-nav|\\.reader-page|\\.reader-header|\\.reader-toolbar|\\.entry-filters|\\.web-auth-|\\.button\\.secondary|\\.button\\.danger|\\.button\\.danger-outline|\\.theme-card\\.is-active|\\.entry-filters__source-chip\\.is-selected|\\.reader-bottom-bar__button\\.is-" assets/themes`
  - `rg -n "theme-card__description|theme-card__notes|feed-card__title|feed-card__meta|entry-card__title|entry-card__meta" assets/themes docs/design/css-separation-baseline-checklist.md`
  - `cargo check -p rssr-app`
  - `git diff --check`
- 主题契约测试：
  - `cargo test -p rssr-app --test test_builtin_theme_contracts`
- 发布前 UI 回归清单接入后复查：
  - `git diff --check`
- 发布前 UI 预检脚本：
  - `bash -n scripts/run_release_ui_regression.sh`
  - `bash scripts/run_release_ui_regression.sh --no-serve --skip-build`
  - `bash scripts/run_release_ui_regression.sh --no-serve --skip-build --with-rssr-web`
- `rssr-web` 浏览器手工 smoke 脚本更新后复查：
  - `bash -n scripts/run_rssr_web_browser_smoke.sh`
- `rssr-web` 浏览器手工 smoke helper：
  - `bash -n scripts/run_rssr_web_browser_smoke.sh`
  - `timeout 20 bash scripts/run_rssr_web_browser_smoke.sh --skip-build --port 18083`
  - `curl -i http://127.0.0.1:18083/healthz`
  - `timeout 20 bash scripts/run_rssr_web_browser_smoke.sh --skip-build --port 18085`
- 正式发布前预检执行：
  - `bash scripts/run_release_ui_regression.sh --no-serve --skip-build --with-rssr-web`
  - `timeout 15 bash -lc 'bash scripts/run_web_spa_regression_server.sh --skip-build --port 8100 ...'`
  - `timeout 18 bash -lc 'bash scripts/run_web_spa_regression_server.sh --skip-build --port 8101 ...'`
- 静态 Web 浏览器 smoke helper：
  - `bash -n scripts/run_static_web_browser_smoke.sh`
  - `timeout 20 bash scripts/run_static_web_browser_smoke.sh --skip-build --port 8102`
  - `timeout 15 bash -lc 'bash scripts/run_web_spa_regression_server.sh --skip-build --port 8103 ...'`
  - `timeout 18 bash -lc 'bash scripts/run_web_spa_regression_server.sh --skip-build --port 8104 ...'`
- 语义接口 grep：
  - `rg -n "app-nav__|entry-directory-rail__|entry-top-directory__" assets/styles crates/rssr-app/src -g'*.css' -g'*.rs'`
- 阅读页接口 grep：
  - `rg -n "\\.reader-page\\b|\\.reader-header\\b|\\.reader-toolbar\\b|\\.reader-meta-block\\b|\\.reader-title\\b|\\.reader-meta\\b|\\.reader-pagination\\b|\\.reader-bottom-bar\\b" assets/styles crates/rssr-app/src -g'*.css' -g'*.rs'`
- 门禁壳接口 grep：
  - `rg -n "class:\\s*\\\"web-auth|\\.web-auth-" assets/styles crates/rssr-app/src -g'*.css' -g'*.rs'`
- 筛选布局接口 grep：
  - `rg -n "\\.entry-filters\\b|\\.entry-filters__toggle\\b|\\.entry-filters__sources\\b|\\.entry-filters__source-grid\\b" assets/styles crates/rssr-app/src -g'*.css' -g'*.rs'`
- 导航壳浏览器回归：
  - `bash scripts/run_web_spa_regression_server.sh --skip-build --port 8093`
  - 完成本地登录后检查：
    - `http://127.0.0.1:8093/entries`
    - `http://127.0.0.1:8093/feeds`
    - `http://127.0.0.1:8093/settings`
  - 已确认导航壳、搜索框、导航链接正常显示，console 无新错误
- 最终静态 Web 应用内部页 headless smoke：
  - `bash scripts/run_web_spa_regression_server.sh --skip-build --port 8107`
  - `google-chrome --headless=new --dump-dom 'http://127.0.0.1:8107/__codex/setup-local-auth?...&next=/entries'`
  - `google-chrome --headless=new --dump-dom 'http://127.0.0.1:8107/__codex/setup-local-auth?...&next=/feeds'`
  - `google-chrome --headless=new --dump-dom 'http://127.0.0.1:8107/__codex/setup-local-auth?...&next=/settings'`
  - 已确认：
    - `data-page="entries"`
    - `data-page="feeds"`
    - `data-page="settings"`
    - `data-layout="entries-layout"`
    - `data-layout="feed-workbench-single"`
    - `data-layout="settings-grid"`
- 最终 `rssr-web` 登录流 smoke：
  - `bash scripts/run_rssr_web_browser_smoke.sh --skip-build --port 18087`
  - `curl -X POST /login`
  - `curl /session-probe`
  - `curl /feeds`
  - `curl /settings`
  - `curl /logout`
  - 已确认：
    - 登录 `303`
    - `/session-probe` `204`
    - `/feeds` `200`
    - `/settings` `200`
    - `/logout` `303 -> /login`
- 静态 Web reader-demo seed 契约：
  - `cargo test -p rssr-infra --test test_browser_state_seed_contracts`
- 静态 Web real reader smoke：
  - `bash scripts/run_web_spa_regression_server.sh --skip-build --port 8109`
  - `google-chrome --headless=new --dump-dom 'http://127.0.0.1:8109/__codex/setup-local-auth?...&seed=reader-demo&next=/entries/2'`
  - 已确认：
    - `data-page="reader"`
    - `data-layout="reader-page"`
    - `Demo Entry Two`
    - `来源：https://example.com/posts/demo-entry-2`
    - `收藏（F）`

## 当前状态、风险、待跟进项

- 当前工作区干净，仅剩未跟踪的 `.codex`。
- 今天这条线已经完成：
  - CSS 公开语义接口迁移
  - 内置主题迁移
  - 主题契约测试
  - 静态 Web / `rssr-web` smoke helper
  - 发布前 UI 预检入口
  - 静态 Web 应用内部页与 `rssr-web` 登录后路径的正式回归
- 当前没有新的阻塞性风险。
- `rssr-web` 浏览器态真实添加订阅仍然不是自动化项，但步骤、selector、推荐 feed 与结果模板都已固定。
- 如果下一步继续，不再建议扩 selector 迁移面；更值的是：
  - 继续补更细的业务 smoke
  - 或在环境允许时把 `rssr-web` 浏览器态添加订阅收成真正自动化
  - 或转去做真正的发布前缺口清单
  - 或回到其它功能/架构主线

## 相关 commit / worktree 状态

- 今日新增关键提交：
  - `70318a9` `test: add release ui preflight runner`
  - `25c34f2` `test: add rssr-web browser smoke helper`
  - `66f8cec` `test: wait for rssr-web browser smoke readiness`
  - `537cafc` `test: add static web browser smoke helper`
- 之前已完成的 CSS/主题基线提交：
  - `be2b7dd` `refactor: add semantic layout interfaces for css`
  - `7fe328a` `refactor: finalize semantic css interfaces`
  - `5937c49` `refactor: align builtin themes with semantic slots`
- 当前 worktree：
  - clean

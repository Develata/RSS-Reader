# GitHub Pages：真实 Dioxus Web 静态演示

基线：`dbe68e11fdbf93769bfa876831113a17928108ab`（main / v0.1.20 发布记录）。

## 一次性仓库配置

1. Settings → Pages → Build and deployment → Source 选择 **GitHub Actions**。
2. 确认 Actions 可用，组织策略允许本 workflow 使用的 actions；
   `github-pages` environment 的 deployment branch 限定 `main`。
3. 合并补丁后 main push 自动构建并部署；也可在 main 手动运行 `Pages demo`。
   PR 仅构建和验收，没有 Pages/OIDC 写权限，也不会上传发布 artifact 或部署。
4. 默认地址为 `https://develata.github.io/RSS-Reader/`；以 deployment 输出为准。
   若配置自定义域名，在 Pages 设置中配置并启用 HTTPS。workflow 从
   `configure-pages` 的 `base_path` 获取路径，支持项目子路径和域名根路径。

无需 `gh-pages` 分支、Jekyll、`_config.yml` 或将产物提交到 docs。
无需为了发布而修改仓库默认 `Dioxus.toml`：本地、Docker 和 rssr-web 保持根路径构建。
本补丁不修改仓库在线设置，也不触发部署。

## 构建与路由

```bash
dx build --platform web --package rssr-app --release --locked \
  --features pages-demo --base-path /RSS-Reader --debug-symbols false
python3 scripts/prepare_pages.py target/dx/rssr-app/release/web/public
PAGES_BASE_PATH=/RSS-Reader bash scripts/test_pages.sh
```

CLI 与项目依赖一致，固定为 Dioxus 0.7.9。上传整个
`target/dx/rssr-app/release/web/public`，包含 index、JS、WASM 和全部资源。
不上传源码、SQLite、认证信息或回归 server 的 `__codex` 辅助接口。

Dioxus 0.7.9 的 `--base-path` 同时控制资源前缀和 WebHistory。
继续使用原始 `AppRoute`、`Router` 与 `Link`，无需给各页面手工添加仓库名前缀。
release 的路由前缀来自编译配置；改 base path 必须重新构建，不能只改 HTML。
不要使用 hash router：当前阅读器自身使用 hash 定位正文。

准备脚本只把生成的 `index.html` 原样复制为 `404.html`，另加 `.nojekyll`。
直接访问 `/RSS-Reader/entries/2` 时，Pages 返回这个相同的 WASM 启动页，
浏览器仍保留原 URL，Dioxus 按前缀后的 `/entries/2` 渲染阅读器。
**首次直达深链接仍返回 HTTP 404**，这不是服务端 rewrite；站内跳转不需请求页面。
若将来要求所有路由直接访问都返回 200，应换支持 rewrite 的托管服务。
`Dioxus.toml` 中 dev-server 的 SPA fallback 配置不能改变 GitHub Pages 的行为。
未知路由仍由现有 Router 处理。

## 演示数据与能力边界

`pages-demo` 为默认关闭的构建 feature，仅 wasm32 生效。构建直接运行同一个 App，
在原有 app shell 内显示一条公开静态演示说明。演示没有服务端账户和访问控制，
不生成测试密码；该 feature 下跳过本地浏览器门禁，方便 localhost 验收。
常规 Web / rssr-web / desktop / Android 构建不启用它。

首次初始化复用 `tests/fixtures/browser_state/reader_demo_{core,app_state,entry_flags,entry_content}.json`，
四份样例由 Rust `include_str!` 编入 WASM，并用真实持久化类型反序列化。
在 `BrowserStore::open_with_initial_state` 中，判断存储是否全新和 seed 发布使用
同一个 Web Lock，发布走既有 copy-on-write slices 与 commit key。
已有 commit、legacy slice 或 staging slice 都不会被 seed 覆盖。
用户删除全部订阅后不会在刷新页面时被重新灌入样例，升级部署也不会重置数据。
损坏数据报错并保留，禁止悄悄恢复样例。存储写入失败时初始化报错；若已建立空数据库，
下一次进入保留这个空库，不会自动重试覆盖。需要重新体验样例时可使用全新浏览器配置。

注意：当前 BrowserStore 的 localStorage key 按 **origin** 共享，URL 子路径并不提供隔离。
本补丁保留这个既有存储约定，不清空已有数据；同一 `develata.github.io` origin 下
其他 RSS-Reader 构建也会读到这些 key。若需要相互隔离，应采用独立 origin。
公开部署只提供静态样例资源，不上传访问者 localStorage。

| 能力 | Pages 演示行为 |
| --- | --- |
| 文章列表、阅读、搜索、已读、收藏、主题 | 使用原有 UI、用例和 BrowserStore |
| 添加订阅、单 feed 刷新、全部刷新 | 在 Web RefreshPort 入口返回清晰错误，不发网络请求 |
| 自动刷新 | 不启动调度器，即使设置中存在刷新间隔 |
| 远端配置 push/pull | 在 Web RemoteConfigPort 入口阻断 |
| 本地配置导入导出 | 保留原实现，不承诺导入后能拉取在线文章 |
| 原文链接、用户导入正文的外部图片 | 仍可能访问第三方；不是完整离线沙盒 |

CORS 是浏览器对跨源请求的限制，Pages 无法通过静态文件为第三方 feed 补响应头。
某些 feed 本来就允许跨源访问，但演示统一关闭在线 feed 操作以保持可预期行为。
现有按钮点击后显示禁用原因，不显示虚假的刷新成功，也不引入公共 CORS proxy。

## 验收

workflow 在发布前运行 `scripts/test_pages.sh`：使用临时 Chrome profile 和一个模拟
Pages HTTP 404 行为的静态 server，检查真实阅读器挂载、JS/WASM 加载、子路径路由、
收藏切换后重载不被 seed 覆盖、手动刷新被明确阻断以及未产生跨源 feed 请求。
这个 server 不包含任何认证或 seed 注入 helper，因此能发现生产入口漏初始化的问题。
验收使用 OS 分配的端口和独立 Chrome profile，支持同机并行；失败保留日志、网络错误与截图，
CI 上传到 `pages-smoke-<run_id>-<attempt>`（保留三天）。本地可设置 `PAGES_PUBLIC_DIR`
使用非默认构建目录。浏览器/服务启动失败立即报错，CDP 验收有 120 秒总超时。
已有 wasm subscription contract harness 增加 seed 并发、已有空库、损坏数据保留测试，
继续由仓库原 CI 执行。可单独运行：

```bash
bash scripts/run_wasm_contract_harness.sh wasm_subscription_contract_harness
```

本地 smoke 需要 Node 22+、google-chrome、curl、Python 3.9+，以及已经构建的 release bundle。

## 为什么不复制 HTML 展示页

页面、CSS、路由、交互状态与持久化仍来自 rssr-app 的同一份源代码。
后续修改阅读器、主题或移动布局时，Pages 自动编译同一个提交并运行实际 bundle 验收。
仅部署配置、合成初始数据和宿主网络能力有所区别；没有第二套 DOM/CSS/模拟交互需要同步。

## 实现依据

- [Dioxus v0.7.9 WebHistory 前缀处理](https://github.com/DioxusLabs/dioxus/blob/v0.7.9/packages/web/src/history.rs)
- [Dioxus v0.7.9 CLI base-path 参数](https://github.com/DioxusLabs/dioxus/blob/v0.7.9/packages/cli/src/cli/target.rs)
- [GitHub Pages 自定义 workflow](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages)

## CI 并发与构建成本

- build 按 ref 分组，后来的同分支提交取消旧 build；不同 PR 仍可并行。
- deploy 单独使用 `github-pages` 并发组且不取消运行中的发布。取得执行机会后检查
  main 当前 SHA，跳过已过时的构建。检查后到真正部署之间仍可能出现新提交；当前发布
  正常完成，新的成功构建随后发布，不强制中断在途部署。
- 纯 docs / README / LICENSE 改动不触发 Pages；手工 workflow_dispatch 不受此限制。
  因为这个 workflow 有路径过滤，不应把它作为所有 PR 都必须出现的 required check；
  原 CI 的 `lint-and-test` 仍承担主线必过检查。
- 保留普通 Web CI 和 Pages 的两种构建，因为 feature 和 base path 不同，不能复用同一个
  WASM artifact。Pages 使用独立 Cargo cache key，只允许 main 保存 Cargo cache；
  不重复执行原 CI 已负责的 cargo fmt。普通 CI 的聚合结果不属于此 workflow 的依赖，
  Pages 发布以自身 build/smoke 成功为门槛，合并策略仍由仓库规则控制。
- 共享 `setup-dioxus-cli` 与 `setup-wasm-bindgen` 改用 `cargo binstall`，指定精确版本、
  `--strategies crate-meta-data` 和 `--no-confirm`；只从 crate 声明的上游二进制路径安装，
  不用 quick-install，也不静默源码编译。确需源码回退时，显式设置 action 输入
  `allow-source-fallback: 'true'`。缓存命中依旧执行版本校验。
- `setup-cargo-binstall` 固定安装器 1.23.0 和 action commit，已有匹配版本时不重复下载。
  并行冷缓存 runner 仍可能各自下载一次工具；不为省这次下载而串行化整个 CI。
- Pages 从 Cargo.lock 取得 wasm-bindgen 版本，复用共享安装器和缓存。dx 0.7.9 的 managed
  mode 不查 PATH，因此在明确的 `DX_HOME` 中登记已验证的 wasm-bindgen，并缓存 esbuild /
  Binaryen 等 asset 工具。这是局限于 dx 0.7.9 的适配，升级 dx 必须重新核对其工具目录。
  `--debug-symbols false` 避免发布调试符号；没有改全局 release profile 或牺牲其他平台调试。

## 模块边界

`demo.rs` 拥有演示策略与样例选择；App 只渲染说明，Web host ports 负责拒绝不支持的操作。
BrowserStore 仅暴露通用的“未初始化才设置初值”能力，不依赖 Pages、fixture 或 feature。
初始化独立于常规 transaction，后者保留原实现；二者共享 Web Lock、取消处理和 cache
访问执行器。日常读写不携带 seed 对象，也不增加 localStorage 存在性扫描。

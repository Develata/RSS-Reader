# GitHub Pages：真实 Dioxus Web 静态演示

实现核对：2026-10-22，v0.2 阶段 D。此页仅说明 `pages-demo` 静态演示；正式 Web 的同源 API 与服务器资料库见 [Web 部署](./web.md)。演示使用单连接内存 SQLite，重新加载页面即重置资料库。

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
cargo run --locked -p rssr-infra --example pages_snapshot
dx build --platform web --package rssr-app --release --locked \
  --features pages-demo --base-path /RSS-Reader --debug-symbols false
python3 scripts/prepare_pages.py target/dx/rssr-app/release/web/public
PAGES_BASE_PATH=/RSS-Reader bash scripts/test_pages.sh
```

CLI 与项目依赖一致，固定为 Dioxus 0.7.9。上传整个
`target/dx/rssr-app/release/web/public`，包含 index、JS、WASM 和全部资源。
不上传源码、用户数据库文件、认证信息或回归 server 的 `__codex` 辅助接口。

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
不生成测试密码；本地浏览器口令门禁已删除，演示直接进入。
常规 Web / rssr-web / desktop / Android 构建不启用它。

每次重新加载使用 `assets/pages-demo/snapshot.json` 的真实 RSS 文章快照，
由 Rust `include_str!` 编入 WASM，并经共享的订阅与刷新仓储端口写入内存资料库。
`assets/pages-demo/sources.json` 指定两个公开源：Develata 每日数学最近 5 篇、
Terence Tao 博客最近 1 篇。保留 RSS 提供的标题、作者、发布时间、正文与原文链接。
测试用的合成 `reader_demo_*` 夹具继续供固定回归使用，不再作为 Pages 的展示内容。

Pages workflow 在构建前运行 Rust `pages_snapshot`，复用产品的 RSS/Atom 解析器。
它在 CI 主机抓取源，不要求源允许浏览器 CORS；单个请求总超时 40 秒，最多 3 次跳转，
每份 RSS 及最终快照分别不超过 2 MiB。按发布时间选取少量正文，按源条目标识去重；
任一源失败、没有可读条目或超过上限则构建失败，不发布半份数据，也不悄悄使用旧快照。
快照记录抓取时间、原始 RSS 的 SHA-256 与最终地址，并作为 7 天 workflow artifact 保留。
没有定时刷新任务；运行一次 Pages 构建才更新一次内容。仓库保留已抓取快照，
本地离线构建可跳过上述 `cargo run`。手动刷新快照后需重新编译 WASM。

页面用 `LocalBackend` 和 `compose_offline_library_use_cases` 复用原生/服务器的 SQL
仓储、迁移器与用例。连接归页面线程的 `thread_local`，句柄只有索引及任务状态，
不使用 Worker、OPFS、IndexedDB、Web Locks 或 localStorage 文章库。每个任务一个事务，
排队取消立即释放闭包，准入等待和 SQL 运行各有期限。同步 SQL 执行期间事件循环不能
同时处理取消；progress handler 按浏览器时钟中断超期语句，任务之间让出事件循环。

阅读、收藏、导入配置、删除订阅等只影响当前页面的内存库，重新加载全部重置。
设备外观偏好由设备壳单独保存，不属于演示资料库。旧版浏览器文章库不读取、不迁移、
不清除；不同标签页不共享演示资料库。初始化失败时报告错误，不暴露初始化了一半的库。

| 能力 | Pages 演示行为 |
| --- | --- |
| 文章列表、阅读、搜索、已读、收藏、主题 | 使用原有 UI、用例和共享 SQL 仓储；资料库重载即重置 |
| 添加订阅、单 feed 刷新、全部刷新 | LocalHost 与 composition 网络端口均拒绝操作 |
| 自动刷新 | 不启动调度器，即使设置中存在刷新间隔 |
| 远端配置 push/pull | LocalHost 拒绝，不装配浏览器 WebDAV 客户端 |
| 本地配置导入导出 | 保留原实现，不承诺导入后能拉取在线文章 |
| 原文链接、用户导入正文的外部图片 | 仍可能访问第三方；不是完整离线沙盒 |

CORS 是浏览器对跨源请求的限制，Pages 无法通过静态文件为第三方 feed 补响应头。
某些 feed 本来就允许跨源访问，但演示统一关闭在线 feed 操作以保持可预期行为。
现有按钮点击后显示禁用原因，不显示虚假的刷新成功，也不引入公共 CORS proxy。

## 验收

workflow 在发布前运行 `scripts/test_pages.sh`：使用临时 Chrome profile 和一个模拟
Pages HTTP 404 行为的静态 server，检查真实阅读器挂载、JS/WASM 加载、子路径路由、
收藏切换后重载重置、手动刷新被明确阻断以及未产生 feed/API 请求。
逐篇核对真实标题、正文和原文链接，检查来源名称和旧存储哨兵不被覆盖。
result.json 记录传输字节、页面就绪时间、WASM 线性内存、JS heap 与主线程 long tasks；
本地 server 不压缩响应，不能把其传输量当成 GitHub CDN 压缩后的下载量。
正文外部图片按浏览器 `Image` 类型单独记录，不计作 feed 请求；同源资源或其他跨源请求失败仍会失败。
这个 server 不包含任何认证或 seed 注入 helper，因此能发现生产入口漏初始化的问题。
验收使用 OS 分配的端口和独立 Chrome profile，支持同机并行；失败保留日志、网络错误与截图，
CI 上传到 `pages-smoke-<run_id>-<attempt>`（保留三天）。本地可设置 `PAGES_PUBLIC_DIR`
使用非默认构建目录。浏览器/服务启动失败立即报错，CDP 验收有 120 秒总超时。
SQL 案例在 native 和 Chrome 两端运行同一份源码，执行器专有行为另用 `wasm_memory_store`。
旧三个 harness 及存储层已退役，69 项行为去向见 [迁移对照](../testing/phase-d-browser-contract-migration.md)。例如：

```bash
bash scripts/run_wasm_contract_harness.sh test_library_contracts test_library_refresh_tickets wasm_memory_store
```

本地 smoke 需要 Node 22+、google-chrome、curl、Python 3.9+，以及已经构建的 release bundle。

部署后另用 `scripts/browser/pages_smoke.mjs` 检查实际公网入口：`PAGES_TEST_BASE` 指向
deployment URL，`PAGES_SNAPSHOT_PATH` 使用该次部署上传的快照，`CDP_BASE` 指向独立
Chrome profile 的调试端口，`PAGES_SMOKE_LOG_DIR` 使用本次新建的证据目录。
公网传输较慢时可显式设置 `PAGES_LOAD_TIMEOUT_MS=180000`（最大 300000）；它只影响
初次进入、重载与逐篇导航的加载等待，交互仍为 30 秒。图片等待另设
`PAGES_IMAGE_TIMEOUT_MS=60000`（默认 5000，最大 300000）；使用短 CDP 查询轮询，
不延长单条 CDP 命令期限，公式图片仍须实际加载并渲染，记录各篇实际等待时间。
默认加载期限仍为 30 秒，CI 的本地 smoke 总时限不变。结果记录所用期限和真实资源计时，
不能把延长等待后的功能通过描述成启动性能达标。驱动禁用缓存，结束时关闭本次标签页，
避免超时后残留下载干扰下次运行。

## 为什么不复制 HTML 展示页

页面、CSS、路由与交互来自 rssr-app 的同一份源代码，资料库操作复用共享 SQL 仓储。
后续修改阅读器、主题或移动布局时，Pages 自动编译同一个提交并运行实际 bundle 验收。
仅部署配置、预抓取初始数据和宿主网络能力有所区别；没有第二套 DOM/CSS/模拟交互需要同步。

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

`demo.rs` 读取快照并经仓储端口建库；App 渲染演示说明；Web host 禁用网络和自动刷新。
`store/wasm_executor.rs` 只管页面线程上的连接和任务。composition 统一装配仓储，
原生使用网络端口，演示使用拒绝网络的端口，SQL 核心不依赖 Pages。

WASM SQLite 的 C 部分需要支持 wasm 的 clang 和 llvm-ar；必要时设置
`CC_wasm32_unknown_unknown` 与 `AR_wasm32_unknown_unknown` 指向工具路径。

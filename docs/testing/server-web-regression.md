# 服务器模式的浏览器回归

2026-10-22 核对 v0.2 C4c 源码。正式 Web 经同源 `/api/v1` 使用服务器资料库；自动回归使用真实 `rssr-web`、独立 SQLite 夹具资料库和本地 feed 服务，经真实登录表单进入页面。浏览器不注入文章库或本地口令。

入口是 [`run_server_web_regression.sh`](../../scripts/run_server_web_regression.sh)，驱动与夹具在 [`scripts/browser/`](../../scripts/browser)。需要 Rust、wasm target、匹配项目的 Dioxus CLI、Node.js 22+、Chrome、Bash 和 curl；Windows 用 Git Bash，避免调用 system32 的 WSL `bash.exe`。

## 套件

| 套件 | 命令 | 范围 |
| --- | --- | --- |
| `regression`（默认） | `bash scripts/run_server_web_regression.sh` | 小视口、导航、列表/阅读/订阅/设置、阅读位置与元信息；默认主题另含目录契约 |
| `theme-matrix` | `bash scripts/run_server_web_regression.sh --suite theme-matrix` | 默认及四个内置主题的阅读页、DOM 和截图 |
| `feed-smoke` | `bash scripts/run_server_web_regression.sh --suite feed-smoke` | 新资料库登录、表单添加订阅、服务器抓取、刷新、进入文章列表 |
| `acceptance` | `bash scripts/run_server_web_regression.sh --suite acceptance --long` | 同源认证与反向代理、CSRF、结果未知、后台任务、只读刷新、草稿、构建不匹配、恶意正文及启动失败 |

`acceptance` 默认跳过等待 30 秒以上的场景；`--long` 纳入全部场景。C4c-2 R2 对应 24 个常规场景、3 个长场景；实际数量和成败以本次 `assertions.json` 为准。`--only a,b` 只由 `rssr_server_acceptance.mjs` 驱动接受，Shell 入口不转发它。

## 构建与隔离

```bash
# 默认 debug；--release 同时选择 release 页面与服务器
bash scripts/run_server_web_regression.sh --preset atlas-sidebar --port 8131
bash scripts/run_server_web_regression.sh --directory-only

# 聚合三个固定套件：theme-matrix、regression、feed-smoke
bash scripts/run_release_ui_regression.sh --with-fixed-smokes
# 已另跑 Rust 门禁时，可跳过该部分
bash scripts/run_release_ui_regression.sh --skip-automated --with-fixed-smokes
# 长验收单独执行，聚合入口的 --full 不包含它
bash scripts/run_server_web_regression.sh --suite acceptance --long --debug --skip-build
```

默认从当前源码构建页面、服务器与夹具工具。`--skip-build` 只跳过页面；Cargo 仍构建服务器和夹具工具。提交、修改源码或切换 feature 后应重建页面。用已有产物时同时传 `--server-bin`、`--fixture-bin`、`--static-dir`，并记录各产物来源。每次载入夹具后，驱动要求页面请求的完整构建标识与服务器启动标识一致；显式 `RSSR_BUILD_ID` 必须同时交给两端。

每个种子使用新资料库，服务器启动刷新结束后才开始交互；固定 feed 由独立服务提供，无需登录。测试出站规则只放行 `127.0.0.1`，拒绝其他 IPv4 与全部 IPv6，不代表部署时的出站策略。不要用日常资料库作为夹具目录。

HTTP 默认端口 8091，CDP 为该端口加 10000；预先拒绝端口占用，不借用已有服务。用 `--port`、`--chrome-bin` / `CHROME_BIN`、`NODE_BIN`、`--log-dir` 指定本机环境。默认 regression 视口 360×800 @ DPR 3，部分检查切到桌面；acceptance 固定 1280×800 @ DPR 1。`--preset`、`--directory-only` 仅用于 regression，`--long` 仅用于 acceptance。

## 结果与覆盖边界

每次保留 `assertions.json`、`summary.md`、构建/服务器/feed/Chrome 日志及 HTML/PNG。验收失败还保留场景请求记录。报告中写明被测 commit/tree、工作树是否干净、构建标识、命令退出码、检查/场景数量，以及意外控制台错误和被忽略条目的原因；任何运行初始化或收尾错误都不是通过。

debug 包会尝试连接服务器未提供的热重载 WebSocket；驱动单独记录这类错误。故障注入只忽略对应请求号，场景声明的预期网络错误还核对来源、origin、路径与状态码。未解释的控制台错误应为零，不能把所有 401/403 或包含类似文字的异常一概忽略。

结构断言和截图不等于人工视觉、辅助技术或 Android/macOS 实机验收。本地 feed 不证明公网可达；本地 Windows 结果不证明 Ubuntu CI 已运行。发布标识注入与旧页面对新服务器的发布验收属于 C6。

Pages 演示是独立构建，继续使用浏览器存储并禁用在线订阅与 WebDAV；用 [`test_pages.sh`](../../scripts/test_pages.sh) 验收，见 [Pages 部署](../deployment/github-pages.md)。三个 wasm BrowserStore 契约 harness 也不替代正式 Web 的 HttpBackend 验收。

## 手工页面与原生夹具

需要保留浏览器页面用于手工检查时，用 `bash scripts/run_rssr_web_browser_smoke.sh` 并按 [手工步骤](./rssr-web-browser-smoke.md) 登录。自动套件结束会关闭自己的进程，聚合入口不再常驻静态 SPA 服务。

原生目录检查可用同一夹具工具创建全新隔离资料库（目录须不存在或为空，不要先用 CLI 建库）：

```bash
cargo run --locked -p rssr-infra --example browser_fixture_library -- \
  --data-dir target/native-directory-app/RSS-Reader --seeds tests/fixtures/browser_state \
  --seed directory-contract --feed-base http://127.0.0.1:8114
```

这个命令只准备库与测试出站策略，不启动 feed 服务。Windows 下把本轮应用及资源复制到 `target/native-directory-app/`，其数据目录即 exe 旁的 `RSS-Reader/`；不要只改 working directory 或 WebView2 profile 来隔离资料库。启动前按需要提供对应本地 feed 服务并记录它。WebView2 的显式 CDP target 方式见 [手工回归](./manual-regression.md)。旧版基线中的双库播种命令只用于复现当时版本，不适用于 v0.2。

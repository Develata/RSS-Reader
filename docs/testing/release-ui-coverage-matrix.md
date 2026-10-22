# 发布前 UI 覆盖矩阵

2026-10-22 按 v0.2 C4c 的当前入口核对。此表描述覆盖能力，不是本次执行结果；实际命令、commit/tree、计数和失败保存在对应 handoff。执行步骤见 [发布清单](./release-ui-regression-checklist.md)。

“自动化”表示已有可重复断言；“截图复核”需要人工查看本次产物；“实机待验”不能用浏览器模拟代替。

| 能力 | 入口 | 状态 / 边界 |
| --- | --- | --- |
| app wasm 构建、单元测试、内置主题契约 | `bash scripts/run_release_ui_regression.sh` | 自动化；不等于页面已验收 |
| 原生共享用例契约 | 三个 `test_library_*_contract_harness`；聚合门禁 | 自动化，真实 SQLite |
| 演示的 BrowserStore 契约 | `bash scripts/run_wasm_contract_harness.sh wasm_refresh_contract_harness wasm_subscription_contract_harness wasm_config_exchange_contract_harness` | 自动化，独立浏览器；不能替代 HttpBackend |
| 服务器 API、认证、HttpBackend | `cargo test --locked -p rssr-web` | 自动化，含真实路由与故障注入 |
| 正式 Web 内部路由、列表、订阅、设置、阅读页 | `bash scripts/run_server_web_regression.sh --suite regression` | 自动化，真实登录与 SQLite 夹具 |
| 手机/桌面几何、导航、搜索、触控、分页、图片、键盘与文字选择 | `regression`，用 `--preset` 选择主题 | 自动化 + 截图复核；WebView2 / Android 另验 |
| 阅读位置、元信息与本地时间、来源计数 | `regression` 与相关 app 测试 | 自动化，沿用同一驱动 |
| Reader 五主题 | `--suite theme-matrix` | 自动化 + 截图复核；不穷举配色/视口 |
| 新库登录、添加订阅、服务器刷新、文章列表 | `--suite feed-smoke` | 自动化，本地独立 feed 服务；不证明公网可达 |
| 反向代理同源 cookie、Origin、CSRF | `--suite acceptance` | 自动化，本地代理夹具 |
| 结果未知、零自动重发、只读刷新合并、跨浏览器可见性 | `--suite acceptance` | 自动化，检查实际 API 请求与可见状态 |
| 草稿/CSS、恢复后编辑、存储失败、停止离开、构建不匹配 | `--suite acceptance --long` | 自动化；本标签页 sessionStorage，不自动保存 |
| 长任务、后台标签时限、页面切换与重载 | `--suite acceptance --long` | 自动化；CI 不含三个长场景 |
| 启动失败、状态提示可见、恶意正文不执行脚本 | `--suite acceptance` | 自动化；不代替辅助技术实机检查 |
| 乱序列表、Reader 旧结果、Settings 保存中的编辑 | app session/runtime 测试 | 自动化，受控任务完成顺序 |
| SQLite 刷新原子性、失败重试 | `test_library_application_refresh_store_adapter`、`test_library_persistence_atomicity` | 自动化，真实资料库 |
| CLI stdout / stderr 与配置交换 | `cargo test --locked -p rssr-cli --test test_stdout_contract` | 自动化，真实进程；不等于 Web 页面 |
| 服务器认证 smoke | 聚合入口 `--with-rssr-web` | 自动化，含真实登录/退出与 `/api/v1/session` |
| Pages 演示启动、路由、状态保留与网络禁用 | `bash scripts/test_pages.sh`（先构建演示包） | 独立构建与验收 |
| WebDAV 上传/下载的实际部署 UI、真实远端 feed | 手工页面 + 当前部署 | 需单独留证，不用 fake 或本地 feed 替代 |
| Android 长按选择、系统返回、图片手势；macOS 交互 | 对应实机与 handoff | 实机待验，浏览器模拟不构成证据 |
| 发布构建标识、旧页面对新服务器 | C6 | 本轮范围外，尚不作为完成项 |

推荐先跑 `bash scripts/run_release_ui_regression.sh --full`，再单独跑 `bash scripts/run_server_web_regression.sh --suite acceptance --long`，最后复看本次截图并补实际部署/实机项。`--full` 的固定组只有 theme-matrix、regression、feed-smoke；CI 的四个非默认主题还需分别用 `--preset` 跑 regression。

控制台错误不能笼统忽略：报告意外条目（必须为零）及每个被忽略条目的原因。运行成功不证明独立评审、CI 或人工验收完成。统一构建和证据规则见 [服务器浏览器回归](./server-web-regression.md)。

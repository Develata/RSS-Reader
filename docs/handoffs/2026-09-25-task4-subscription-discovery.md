# 任务 4：网站首页订阅发现

- 日期：2026-09-25
- 作者 / Agent：Codex
- 分支：detached worktree `.handoff/worktrees/task4`
- 当前 HEAD：2ba826a
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`draft`

## 工作摘要

添加订阅支持网站首页；共享 application 发现流程，infra 有界 HTTP 与 HTML 解析，UI 多候选选择，CLI 非交互列出候选。原生环境检查仍阻塞，保留未提交。

## 影响范围

- 模块：subscription workflow / composition / feed service，NewFeedSubscription，SQLite / browser feed adapter，新增 infra subscription_probe，feeds page / host capability，rssr-web proxy，测试与文档。
- 平台：Web、桌面、Android、CLI。
- 额外影响：无数据库迁移。直接使用依赖树已存在的 html5ever 0.39、encoding_rs 0.8、futures-util 0.3；reqwest 启用 stream，有界读取。

## 关键变更

- 先解析 feed；失败且为 HTML 才扫描 head 声明。支持 RSS / Atom、相对 URL / base、最终响应 URL、大小写与去重、仅 HTTP(S)，忽略正文和脚本伪 link。
- 无候选才依次验证 `/feed`、`/rss.xml`、`/atom.xml`、`/index.xml`，最多四个，不递归；多个候选交给 UI 选择。
- 单响应最多 8 MiB，流式检查大小，请求超时30秒。Web 复用代理/直连与 CORS 提示；发现请求不保存防缓存参数。
- 原生 / 浏览器保存已解析 feed，首次入库复用结果。`--skip-refresh` 仍解析发现，但不导入文章。重复 URL 返回“已订阅”，不修改原订阅、不刷新。
- 保存最终 feed URL；site_url 优先 feed 自带地址，再回退发现页面。导入配置仍使用无网络的 FeedService，构造点新字段默认 None。
- proxy 新响应头 `x-rssr-final-url` 为受验证的最终上游 URL；维持原 SSRF、重定向及大小上限。新版发现页面需配套服务端；旧代理缺该头时会明确报错或按既有规则回退直连，不伪造最终地址。
- 候选标题/地址文本渲染，长文本上下排列并换行。输入变化清除候选，取消保留草稿，沿用添加忙态与原生 form submit。

### 公开契约

- `FeedDiscoveryCandidate { url: Url, title: Option<String> }`。
- `SubscriptionProbeOutcome::Feed { url, update } | Html { page_url, candidates }`。
- `SubscriptionProbePort::probe(&self, &Url) -> anyhow::Result<SubscriptionProbeOutcome>`，平台 async Send 差异仅适配约束。
- `PreparedSubscription { url, update }`，`with_fallback_site_url(self, Option<Url>) -> Self`。
- `PrepareSubscriptionOutcome::Ready(PreparedSubscription) | NeedsSelection { page_url, candidates }`。
- `SubscriptionWorkflow::new(feed_service, refresh_service, app_state, Arc<dyn SubscriptionProbePort>)`；`prepare_subscription(&self, &str) -> Result<PrepareSubscriptionOutcome>`；`add_prepared_subscription(&self, AddSubscriptionLifecycleInput, PreparedSubscription) -> Result<AddSubscriptionLifecycleOutcome>`。
- `AppCompositionInput.subscription_probe`；`RefreshService::apply_prepared_update(&self, i64, FeedRefreshUpdate) -> Result<RefreshFeedOutcome>`。
- `NewFeedSubscription.site_url: Option<Url>`；FeedRepository 方法签名未变，schema 未变。
- UI 内部 RefreshPort / FeedsPort add_subscription 新增 `Option<Url>` 回退地址，AddSubscriptionOutcome 新增 NeedsSelection；FeedsCommand::AddFeed 新增同字段。
- 稳定选择器：`data-layout=feed-discovery-candidates`、`data-action=select-feed-candidate|cancel-feed-discovery`、`data-slot=feed-candidate-url`；旧 feed-form/add-feed 保留。样式在 workspaces.css。

## 验证与验收

### 自动化验证

实际日志：共享 `target/task4-validation/`。

| 命令 | 退出码 | 结果 |
|---|---:|---|
| cargo fmt --all --check | 0 | 通过 |
| cargo clippy --workspace --all-targets -- -D warnings | 101 | GTK/WebKit 等开发 pkg-config 库缺失 |
| cargo test --workspace | 101 | 同上，未执行完 |
| cargo check -p rssr-app --target wasm32-unknown-unknown | 0 | 通过 |
| cargo clippy -p rssr-app --target wasm32-unknown-unknown -- -D warnings | 0 | 通过 |
| cargo check -p rssr-app --target aarch64-linux-android | 101 | 缺 aarch64-linux-android-clang / NDK |
| git diff --check | 0 | 通过 |
| cargo test -p rssr-application -p rssr-infra -p rssr-cli | 0 | 通过 |
| cargo clippy -p rssr-application -p rssr-infra -p rssr-cli -p rssr-web --all-targets -- -D warnings | 0 | 通过 |
| cargo check -p rssr-app --tests --target wasm32-unknown-unknown | 0 | 测试代码编译通过，非执行 |
| cargo test -p rssr-infra --test test_subscription_discovery -- --nocapture | 0 | 2项，通过本地 TCP HTTP fixture 验证重定向、base、首次请求复用、去重、4路径、skip-refresh、超大响应；共享解析案例含畸形HTML、大小写、多候选、无效协议 |
| cargo test -p rssr-web | 0 | 19项通过，含现有代理安全测试 |
| bash scripts/run_wasm_contract_harness.sh wasm_subscription_contract_harness wasm_refresh_contract_harness wasm_config_exchange_contract_harness | 0 | 5 / 19 / 3项通过；最后解析修正后 subscription 单独复跑5项通过 |
| dx build --package rssr-app --platform web | 0 | 成功；工具版本0.7.10与Dioxus0.7.9不匹配警告保留 |
| bun target/task4-validation/browser.cjs | 0 | 最终两视口场景通过 |
| bun target/task4-validation/direct-browser.cjs | 0 | 真实本地 HTTP + CORS直连，从首页发现、保存与首次导入通过 |

Harness 使用既有 ChromeDriver 151 remote9518 / Chrome151 / wasm-bindgen0.2.126；取消 HTTP 代理。初次漏导入 Url 编译失败已修复；direct-browser 初次与重建资源重叠导致页面未就绪超时，最终构建后复跑通过。
CLI 实际 HTTP fixture：`add-feed /single --skip-refresh` 退出0，DB文章0且site_url正确；`add-feed /multi` 退出1并列出两个URL（预期）；`add-feed /atom.xml` 退出0且文章1。请求日志证明每次 feed 只抓一次。

### 手工验收

- Chromium151 / Playwright（bun执行），360×800、1280×800：单候选自动添加；多候选选择；长中文标题与地址无横向溢出，按钮≥44px；重复订阅保留输入且不发新请求；无候选四次探测后保留输入报错；site_url 回退/优先规则通过。
- 受控 proxy 响应 fixture 验证新头及相对路径，真实HTTP另测现有代理拒绝本地地址后的CORS直连。
- 移动截图初版标题/URL并排挤压已改为上下排列，最终重跑通过。截图 candidates-360.png / candidates-1280.png。
- 未运行：原生桌面界面（缺开发库），Android实机/模拟器（本轮只做编译）；不把编译失败算通过。

## 结果

已验证 native 存储用例、Web UI、CLI与相关wasm契约。全平台验收未达标，不提交，不推送。

## 风险与后续事项

- 原生依赖和NDK安装未经授权，未安装；命令和缺包详见任务3交接。
- 多个常见路径候选在用户选定后会重新验证选中URL，首次入库仍复用这次验证结果；不持久化候选缓存。
- 新代理头需要服务端配套；此次没有部署。
- 未与任务2/3合并；环境修复后按顺序集成并回归。`target` 未跟踪symlink仅本机共享构建缓存，不提交。

## 给下一位 Agent 的备注

入口 subscription_discovery.rs / subscription_probe.rs。不要清理任何未提交 worktree；任务5同样从2ba826a独立推进，集成时保留 apply_source_output 复用路径和新增计数。

## 改动文件清单

- `Cargo.lock`：显式复用既有轻量解析/流依赖，不升级版本。
- `README.md`：同步本任务的用户流程、行为口径和限制。
- `assets/styles/workspaces.css`：增加可换行操作区、长文本布局和44px触控目标样式。
- `crates/rssr-app/src/bootstrap.rs`：同步平台装配和host capability结果透传。
- `crates/rssr-app/src/bootstrap/native.rs`：同步平台装配和host capability结果透传。
- `crates/rssr-app/src/bootstrap/web.rs`：同步平台装配和host capability结果透传。
- `crates/rssr-app/src/pages/feeds_page/facade.rs`：连接页面状态、交互事件、异步结果与渲染。
- `crates/rssr-app/src/pages/feeds_page/intent.rs`：连接页面状态、交互事件、异步结果与渲染。
- `crates/rssr-app/src/pages/feeds_page/reducer.rs`：连接页面状态、交互事件、异步结果与渲染。
- `crates/rssr-app/src/pages/feeds_page/sections/compose.rs`：连接页面状态、交互事件、异步结果与渲染。
- `crates/rssr-app/src/pages/feeds_page/session.rs`：连接页面状态、交互事件、异步结果与渲染。
- `crates/rssr-app/src/pages/feeds_page/state.rs`：连接页面状态、交互事件、异步结果与渲染。
- `crates/rssr-app/src/ui/commands/feeds.rs`：连接命令执行、结果映射或反馈生命周期。
- `crates/rssr-app/src/ui/runtime/feeds.rs`：连接命令执行、结果映射或反馈生命周期。
- `crates/rssr-app/src/ui/runtime/services.rs`：连接命令执行、结果映射或反馈生命周期。
- `crates/rssr-application/src/composition.rs`：扩展统一用例/组合端口并透传本任务结果。
- `crates/rssr-application/src/feed_service.rs`：扩展统一用例/组合端口并透传本任务结果。
- `crates/rssr-application/src/import_export_service.rs`：扩展统一用例/组合端口并透传本任务结果。
- `crates/rssr-application/src/lib.rs`：扩展统一用例/组合端口并透传本任务结果。
- `crates/rssr-application/src/refresh_service.rs`：扩展统一用例/组合端口并透传本任务结果。
- `crates/rssr-application/src/subscription_discovery.rs`：统一单候选、多候选及四个常见路径的发现流程。
- `crates/rssr-application/src/subscription_workflow.rs`：扩展统一用例/组合端口并透传本任务结果。
- `crates/rssr-domain/src/feed.rs`：扩展订阅site_url输入，沿用现有实体和schema。
- `crates/rssr-infra/Cargo.toml`：显式复用既有轻量解析/流依赖，不升级版本。
- `crates/rssr-infra/src/application_adapters/browser/adapters/feed.rs`：实现或复用浏览器适配逻辑，保持平台差异在边界内。
- `crates/rssr-infra/src/application_adapters/browser/feed.rs`：实现或复用浏览器适配逻辑，保持平台差异在边界内。
- `crates/rssr-infra/src/composition.rs`：同步平台装配和host capability结果透传。
- `crates/rssr-infra/src/db/feed_repository.rs`：保存可选站点URL，不修改数据库schema。
- `crates/rssr-infra/src/lib.rs`：适配本任务的跨层实现与契约。
- `crates/rssr-infra/src/subscription_probe.rs`：实现有界抓取、响应分类、feed解析及最终URL处理。
- `crates/rssr-infra/src/subscription_probe/html.rs`：以html5ever提取head内订阅候选并归一化去重。
- `crates/rssr-infra/tests/support/discovery_cases.rs`：补充或适配用例、存储契约和失败路径验证。
- `crates/rssr-infra/tests/test_application_refresh_store_adapter.rs`：适配新增site_url构造字段，保留原测试语义。
- `crates/rssr-infra/tests/test_archive_filter_parity.rs`：适配新增site_url构造字段，保留原测试语义。
- `crates/rssr-infra/tests/test_concurrent_refresh_writes.rs`：适配新增site_url构造字段，保留原测试语义。
- `crates/rssr-infra/tests/test_config_exchange_contract_harness.rs`：适配新增site_url构造字段，保留原测试语义。
- `crates/rssr-infra/tests/test_config_package_io.rs`：适配新增site_url构造字段，保留原测试语义。
- `crates/rssr-infra/tests/test_entry_large_dataset_performance.rs`：适配新增site_url构造字段，保留原测试语义。
- `crates/rssr-infra/tests/test_entry_state_and_search.rs`：适配新增site_url构造字段，保留原测试语义。
- `crates/rssr-infra/tests/test_feed_refresh_flow.rs`：适配新增site_url构造字段，保留原测试语义。
- `crates/rssr-infra/tests/test_refresh_contract_harness.rs`：适配新增site_url构造字段，保留原测试语义。
- `crates/rssr-infra/tests/test_subscription_contract_harness.rs`：补充或适配用例、存储契约和失败路径验证。
- `crates/rssr-infra/tests/test_subscription_discovery.rs`：补充或适配用例、存储契约和失败路径验证。
- `crates/rssr-infra/tests/test_webdav_local_roundtrip.rs`：适配新增site_url构造字段，保留原测试语义。
- `crates/rssr-infra/tests/wasm_subscription_contract_harness.rs`：补充或适配用例、存储契约和失败路径验证。
- `crates/rssr-web/src/proxy.rs`：透传经验证的最终上游URL，保留代理安全边界。
- `docs/design/frontend-command-reference.md`：同步命令、状态流转与反馈规则。
- `docs/design/theme-author-selector-reference.md`：登记新增稳定选择器与样式接口。
- `docs/handoffs/2026-09-25-task4-subscription-discovery.md`：记录本任务接口、验收证据和剩余阻塞。
- `docs/user-guide.md`：同步本任务的用户流程、行为口径和限制。

## 最终环境备注

本轮创建的临时HTTP fixture、SPA验收服务和ChromeDriver已停止，截图与日志保留在target中。最终PATH未找到sdkmanager、adb或xvfb-run，ANDROID_HOME/ANDROID_SDK_ROOT未配置；NDK安装命令以已有Linux Android command-line tools为前提，本轮没有安装。各worktree仍以2ba826a为HEAD，全部改动未暂存，未commit/push。

## 2026-09-26 主线集成复验

集成基线为任务 3 提交 `a1aae97`（已含任务 2），原隔离工作树保留。文档合并同时保留三项能力；subscription harness 同时保留批量筛选与 HTML 发现案例。任务 3 的 SQLite 测试补齐 NewFeedSubscription.site_url=None，没有改变筛选语义。合并后原生 workspace lint / tests 和 wasm check 已退出 0；其余结果在本次补验末尾记录，日志位于 target/integration-validation/task4-*。
- 合并后完整八项检查全部退出 0：fmt、workspace clippy/tests、wasm check/clippy、Android ARM64 check、工作树/暂存区 diff check。
- 三个 wasm harness 分别退出 0：subscription 6 passed（同时含批量已读与发现）、refresh 19 passed、config exchange 3 passed。
- 合并后 Web build、browser.cjs、direct-browser.cjs 均退出 0：360×800 / 1280×800 单候选、多候选、重复订阅不抓取、site_url 回退与 feed 信息优先、四次路径探测、无横向溢出通过；真实 HTTP 8101 / CORS 直连发现和首次刷新通过。已查看最新 360px 候选截图。
- 当前 CLI build 退出 0；本地 HTTP `/single --skip-refresh` 退出 0，`/multi` 预期退出 1 并列出候选，直接 `/atom.xml` 添加退出 0。独立 SQLite 断言退出 0：两个订阅中只有直接添加的订阅包含 1 篇文章，skip-refresh 订阅没有文章。
- 本轮确认范围已通过，任务 4 达到本地提交条件；Android 仅编译，未作运行验收。

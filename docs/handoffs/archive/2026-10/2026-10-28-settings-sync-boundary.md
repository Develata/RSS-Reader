# Settings 配置交换规则与页面边界收口

- 日期：2026-10-28
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 开始时 HEAD：`7f1014a014f29f9f5449baa60fe015406c7aba23`
- 相关 commit：与本记录同批本地提交；记录生成时 `commit: pending`。提交后用 `git log -1 -- docs/handoffs/2026-10-28-settings-sync-boundary.md` 定位。
- 相关 tag / release：N/A；未 push、打 tag 或发布。
- 状态：`validated`

## 工作摘要

承接来源编辑批次的建议，先盘点 Settings 加载、同步与主题文件的职责。加载与主题文件已具备明确边界，因此只把 WebDAV 确认与请求快照收回现有 Rust 状态模块。保持操作与结果行为，补充真实页面同路径验收，不新增模块框架或第二份状态。

## 影响范围

- 生产代码：`crates/rssr-app/src/pages/settings_page/sync/state.rs`、`sync/session.rs`。
- 平台：共享 Rust 页面行为影响 Web、Windows、Linux、macOS、Android；实际交互验收为 Web 与 Windows 隔离 WebView2，Android 仅编译检查。
- 工具与文档：新增 Settings 同步验收脚本，现有启动失败回归支持选择该脚本，更新原生前置边界及前端命令契约。
- 没有修改 application/domain/infra/host、数据库迁移、配置包格式、样式或组件树。

## 关键变更

### 确认和请求准备

- `prepare_pull` 首次要求显示确认，再次返回 `PullConfig` 并消耗确认；每次实际下载都需再次确认，包括失败后的重试。
- 修改端点或路径（同值输入事件也包括在内）、准备上传，均清除已有下载确认。上传仍可直接执行，没有新增 busy gate。
- 命令准备在一次状态借用中完成；捕获两项原始输入，不裁剪、不预校验。session 在释放借用后经原有 `spawn_projected_ui_command` 执行；提示文案、任务作用域、Settings 结果投影和发布顺序保持原样。
- 纯 Rust 测试由 3 项增至 6 项，覆盖反复确认、任一字段改动、同值事件、上传清除确认、没有新增去重、空值/空格/非 ASCII 目标与后续编辑不改变已准备的命令。

### 页面边界盘点

- 初次加载由页面启动任务，runtime 调用 settings capability，session 发布结果；没有继续拆简单的加载规则。
- 主题文件已经分为平台文件 I/O、成功/取消/失败反馈、CSS 校验与保存三层；保留现有边界。
- 现状：初次加载和下载成功会替换整份草稿；外观保存则保护提交后的新编辑。本次不改变这些操作交错时的语义，也不将其宣称为已解决的并发保护。
- 详细约束见 `docs/design/native-ui-preparation.md` 的 Settings 边界段落。

## 验证与验收

### 自动化检查

证据根目录：`target/settings-sync-20261028/`，均为本地忽略产物。

| 命令 | 结果 | 日志 |
| --- | --- | --- |
| `cargo fmt --all --check` | 通过 | `fmt.log` |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 通过 | `clippy.log` |
| `cargo test -p rssr-app pages::settings_page --locked` | 27 通过 | `settings-tests.log` |
| `cargo test --workspace --locked` | 540 通过、0 失败、2 忽略（既有手工性能探针） | `workspace-tests.log` |
| `cargo check -p rssr-app --target wasm32-unknown-unknown --locked` | 通过 | `check-wasm.log` |
| `cargo check -p rssr-app --target aarch64-linux-android --locked` | 通过，使用本机 NDK 28.2 的 clang/llvm-ar | `check-android.log` |
| `target/release-ui-tools/dioxus-0.7.9/dx.exe build --platform web --package rssr-app --locked --release --debug-symbols false` | 通过，完整日志确认 client build 成功 | `build-web.log` |
| `cargo build -p rssr-app --release --locked --target-dir target/reading-baseline-20261025/build-b` | 通过 | `build-native.log` |
| `node scripts/browser/rssr_feeds_lifecycle_startup_test.cjs target/settings-sync-20261028/startup-settings settings-sync` | 3 项通过 | `startup-settings/results.json` |
| `node scripts/browser/rssr_feeds_lifecycle_startup_test.cjs target/settings-sync-20261028/startup-feeds` | 既有默认入口 3 项通过 | `startup-feeds/results.json` |

启动失败测试以真正的子进程验证缺失目标页、缺失数据库、CDP 503：及时非零退出、旧成功回执被替换为失败、调用方浏览器仍存活、只读数据库哈希不变。

### 同路径真实页面验收

- Web：分别指定基线 `PUBLIC_DIR=target/feed-edit-20261028/candidate/public` 与候选 `PUBLIC_DIR=target/settings-sync-20261028/candidate/public`，运行 `node scripts/browser/rssr_settings_sync_acceptance.cjs --artifact-dir <path>`。
- Windows：本地 `run-native.ps1` 核对 EXE 哈希，复制 checkpoint 后的 5000 篇固定 fixture，来源地址仅在复制库内改为回环地址；使用独立程序目录、独立 WebView2 profile、隐藏进程和 CDP。脚本在任何导航/修改前只读核对数据库及当前 Entries 页的 ID/标题，结束只关闭自己创建且路径匹配的进程。
- 两端基线与候选均验证 1536×844、390×844 视口，经过相同路径：页面加载 → 下载只显示确认且无请求 → 改端点取消确认 → 改路径取消确认 → 上传取消确认并提交当前目标 → 再次确认下载 → 延迟回复期间改目标 → 原请求仍使用旧参数且正确回填配置 → 404 → 无效配置失败 → 重新确认后重试成功。
- Web 使用真实 WASM/BrowserStore；Playwright 将请求转发给自管 `127.0.0.1:9277` HTTP fixture。Windows 使用实际原生 HTTP/SQLite。每个视口核对恰好 1 次 PUT、4 次 GET 的方法与路径，导入成功的草稿、全局主题和持久化设置一致。
- 服务端回复仅修改上传包的主题和合法归档阈值 120，完整保留原订阅集合。每个视口核对 Web 全部 72 篇、Windows 全部 5000 篇文章标记，以及全部 6 个订阅 ID/URL 不变。
- 最终回执：`baseline-web-full/acceptance.json`、`candidate-web-full/acceptance.json`、`native-baseline-final/write/acceptance.json`、`native-candidate-final/write/acceptance.json`。
- Console：两端没有未预期的 Console error 或 pageerror。Web 每轮缺失远端场景产生明确归属于 fixture URL 的 404 资源错误，共 2 条，单独保留在 `expectedConsoleErrors`，没有忽略其他错误。
- 整页截图：初始页、确认态各两视口。3 张逐字节一致；390 确认态仅 2 个像素的蓝通道相差 1/255，尺寸均 390×3857。已查看桌面和窄屏截图，未见布局漂移。原始哈希及差异位置/RGBA 见 `screenshots-full-comparison.json`、`screenshots-pixel-comparison.json`。
- 手工前台验收、Android 实机、Linux/macOS 实机：未执行。

### 验收工具的调整记录

- 最初 Web 基线被 Chromium 对被拦截页面 origin 的 loopback 网络权限阻止，尚无 HTTP 请求到达。改为仅在隔离 Web fixture 中经 Playwright 转发，生产代码未变；不据此声称真实站点 CORS/本地网络权限通过。
- 最初 Windows 基线脚本使用了不存在的 `data-nav="entries"`，在身份检查后超时。改用既有 Feeds/Settings 导航，失败运行已关闭自己的进程并保留失败回执；修正后重新建立基线。
- 卡片局部截图会把标题置于 sticky 导航下，最终改为从页面顶部截整页；保留旧截图供审计，以 `*-web-full` 为最终视觉证据。

### 候选身份

| 产物 | SHA-256 |
| --- | --- |
| 基线 WASM | `d53771b051df8097e8abbd6ad169dca532f029fd46d2ed02900658f210236b34` |
| 候选 WASM | `62bcec002c5a55bffe5f994d9a47e901a252ff260d0ad9bcb24589667d21dfb3` |
| 基线 Windows EXE | `5BB314912F694B0ADDFDD822681437C4E9B41F2CBA8BFDF7F40BE35588AB1CDE` |
| 候选 Windows EXE | `429558B24C14CFF4A821A0E4F4BDF1ED846E8359F2ADF4ACE7F642F95E131907` |

完整 Web 引用文件身份见 `baseline-web-identity.json`、`candidate/web-identity.json`；生产源码及验收脚本身份见 `validation-identity.json`，原生每次运行另有 `identity.json`。本地候选可追溯，不作为发布包。

## 结果

- 共享 Rust 交互规则已收口，session 的重复判断被移除；本批具备测试、同路径验收和交接，可作为本地提交交付。
- 未改变配置交换语义、页面视觉或平台职责。没有测量或宣称用户可感知的性能提升。

## 风险与后续事项

- 未测试真实远端 WebDAV 的 HTTPS、认证、权限与网络环境；本机 HTTP fixture 不能代替远端服务验收。
- 未新增下载/上传 busy gate，也没有改变初次加载、同步与保存结果交错的规则；本次延迟测试覆盖目标快照，不等价于完整竞态验收。
- 未复测 CSS 文件选择器与 Android 文件交互。下一批建议补主题文件交互契约验收（取消、编码错误、保存失败、离页），先获取行为证据，再决定需要哪些原生适配改动。

## 给下一位 Agent 的备注

- 从 `sync/state.rs` 的 `prepare_push` / `prepare_pull` 和对应 `session.rs` 入手；外观保存仍看 `save_policy.rs`，不要混用两者的草稿回填语义。
- 运行同步验收需要空闲回环端口 9277；原生辅助运行器另用 CDP 9276。只使用独立复制 fixture，不得连接日常使用的程序或数据库。
- 本次沿用当前 checkout。开始时的 `.agents` / `.specify` 删除、旧目录交接修改、未跟踪附件和其他交接均属任务外内容，保留且不纳入提交。

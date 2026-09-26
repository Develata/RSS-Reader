# Web 多标签与跨片存储一致性修复

- 日期：2026-09-26
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：0d781bc（本轮起点，工作树干净）
- 相关 commit：本记录所在提交（`fix(web): serialize browser writes and preserve complete snapshots`）
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

承接用户对 P2 修复的授权，处理上轮已复现的多标签已读／收藏覆盖，以及四片 localStorage 顺序写入的部分失败。将一致性、缓存同步和失败恢复集中到 browser adapter，移除刷新共享未提交状态的延迟批次。没有迁移到 IndexedDB，也没有改变 application/domain 或 SQLite 的语义。扩展 UI 验收同时发现收起按钮挤窄搜索框，补齐 CSS 换行预算。

## 影响范围

- 模块：rssr-infra 的 browser repositories / state、Web 装配、wasm 契约 harness 与浏览器验收脚本。
- 平台：存储行为改动仅 Web；窄导航搜索换行 CSS 跨端共用，桌面与 Android 完成编译／测试矩阵，未改原生存储。
- 额外影响：浏览器持久化提交协议及 Web Locks 能力要求；升级后必须重新加载旧标签页。
- CodeGraph：工具存在但仓库没有 `.codegraph/`，使用源码、rg 与 Git 历史；未安装或生成索引。

## 关键变更

### 一个浏览器事务边界

`BrowserStore` 在同源 Web Lock 中读取提交版本、同步发生变化的片段、应用本次操作，再发布提交记录。锁不跨 HTTP 请求；排队超时设为 5 秒，取消的调用不会在稍后获得锁时补写。JS Promise 留在 host executor，通过 Rust oneshot 保持既有 repository 的 Send future，不引入 unsafe 或 application/domain 平台分支。

四片内容结构不变，新增 `rssr-web-commit-v1`：`{version:1, epoch:<UUID>, revisions:[core, app_state, flags, content]}`。每片固定使用原键和 `-next` 两个槽位，以版本奇偶决定活跃槽。暂存全部变化片段后只发布一次 head；中途写失败时仍读取旧的完整提交。成功后清理非活跃槽，清理失败不把已成功提交误报为失败，下一次写入重试清理；协议最多九个键，不生成持续增长的 journal。已有历史 corrupt 备份、认证键和任务外存储不删除。

每次查询也检查 head，只有版本变化的片段才解析；正常标记操作不读写正文。内存不能被 adapter 外直接修改；公开 snapshot 是独立副本。失败时才重新读取已提交状态，避免每次操作提前 clone 全文；若恢复也失败，缓存保持无效，下次查询继续报错或重新加载，不能暴露未提交值。epoch 防止清空／重新导入后从零开始的版本误命中旧缓存。

首次打开先验证四片旧数据，接入现有键，缺失片段初始化为空。损坏数据现在原位保留并报错，不再删键后以空库启动。head 缺失但仍存在新版槽位时也拒绝重新初始化，避免把已有数据误认成空旧库。不可访问存储、配额不足及缺少 Web Locks 都不会伪装为保存成功。

依据：[Web Locks 标准](https://w3c.github.io/web-locks/)的同源互斥和取消能力，以及 [HTML Storage API](https://html.spec.whatwg.org/multipage/webstorage.html#the-storage-interface) 的单次 setItem 发布语义。锁内序列化是保持读取到提交互斥所需，不能照搬 Web bootstrap 中“尽量锁外持久化”的旧建议。

### 刷新提交与性能取舍

- 每个订阅成功提交后才返回 inserted_count；批次 hooks 使用与 SQLite 相同的空实现。页面退出／批次中断不再依赖 Drop 尽力写入共享内存。
- 304 和失败诊断只写 core；带正文的更新写 core + content；不写 flags 或 app_state。网络请求期间订阅被删除时，迟到响应不会使其复活。
- 标记只写 flags，设置只写 core，偏好只写 app_state；删除文章涉及的索引／正文／标记一起发布。批量已读在锁内重新校验预览范围。
- Web Locks 等待及版本缓存属于必要的一致性成本。没有增加后台轮询、全量回滚副本、主标签选举或 UI 同步事件总线。
- 仍然整片保存正文；取消整轮延迟写回增加大量正文更新时的重复序列化。本轮不声称所有刷新更快，也不改变既有正文更新策略。

### 小屏搜索换行

收起按钮预留 48px，但原搜索容器断点仍为 280px，360px 视口下 Reader 输入宽度仅 102px。改为 280 + 48 = 328px 的容器断点；窄导航给搜索独立一行，并归还仅首行需要的箭头预留；宽导航维持原高度，收起后仍仅 44×44px。最终实测手机输入宽 300px、当前 Atlas 262px、旧 Atlas 174px。保留至少 140px 的输入宽度断言；高度断言按已有窄容器换行语义限制为至多增加一行，而不是要求窄屏也保持一行。

### 文件清单

| 文件 | 一句话说明 |
| --- | --- |
| `Cargo.lock` | 显式列入已存在的 wasm-bindgen 两个传递依赖，未升级版本。 |
| `crates/rssr-infra/Cargo.toml` | wasm 侧声明 host bridge 依赖并启用已有 UUID 的 v4。 |
| `crates/rssr-app/src/bootstrap/web.rs` | 异步打开 BrowserStore，初始化失败返回可读错误。 |
| `crates/rssr-infra/src/composition.rs` | 浏览器装配传入 BrowserStore，原生装配保持不变。 |
| `crates/rssr-infra/src/application_adapters/browser/state.rs` | 导出存储入口和提交键，移除绕过事务的旧函数。 |
| `crates/rssr-infra/src/application_adapters/browser/state/models.rs` | 移除旧 LoadedState，四片业务数据结构不变。 |
| `crates/rssr-infra/src/application_adapters/browser/state/storage.rs` | 实现固定双槽、原子发布、增量缓存与失败恢复。 |
| `crates/rssr-infra/src/application_adapters/browser/state/store.rs` | 封装异步事务入口、宿主回调及排队取消。 |
| `crates/rssr-infra/src/application_adapters/browser/state/lock.js` | 用 Web Locks 与 AbortController 提供有等待上限的宿主能力。 |
| `crates/rssr-infra/src/application_adapters/browser/adapters/entry.rs` | 查询、标记、批量已读及删除统一经过事务。 |
| `crates/rssr-infra/src/application_adapters/browser/adapters/feed.rs` | 在最新 core 上执行订阅读写，避免旧快照覆盖与 ID 冲突。 |
| `crates/rssr-infra/src/application_adapters/browser/adapters/settings.rs` | 设置保存仅修改最新 core 中的设置字段。 |
| `crates/rssr-infra/src/application_adapters/browser/adapters/app_state.rs` | 偏好与条件清理使用统一事务。 |
| `crates/rssr-infra/src/application_adapters/browser/adapters/refresh.rs` | 逐订阅完整提交，删除不可靠的共享延迟批次。 |
| `crates/rssr-infra/src/application_adapters/browser/adapters/shared.rs` | 保留 DomainError 类型并映射存储错误。 |
| `crates/rssr-infra/src/application_adapters/browser/query.rs` | 将原有函数移到测试模块前，修复扩展 wasm clippy 发现的旧 lint，查询逻辑不变。 |
| `crates/rssr-infra/tests/support/browser_storage.rs` | 共用旧格式 seed、独立存储句柄与提交片段读取助手。 |
| `crates/rssr-infra/tests/support/browser_storage_cases.rs` | 增加并发、取消、超时、损坏、回滚失败、缓存失效与正文访问验证。 |
| `crates/rssr-infra/tests/wasm_subscription_contract_harness.rs` | 保留原订阅／标记契约，接入真实持久化并增加一致性用例。 |
| `crates/rssr-infra/tests/wasm_refresh_contract_harness.rs` | 更新逐订阅提交契约，新增各阶段故障注入、删除竞争及写入量测量。 |
| `crates/rssr-infra/tests/wasm_config_exchange_contract_harness.rs` | 配置交换通过 BrowserStore 与实际存储验证。 |
| `scripts/run_web_spa_regression_server.sh` | seed 清理新协议键，dump 在锁内读取当前发布版本。 |
| `scripts/browser/storage_helpers.cjs` | 浏览器测试在锁内修改 fixture 并发布版本，读取当前活跃槽位。 |
| `scripts/browser/rssr_storage_consistency_acceptance.cjs` | 两标签真实 UI 验证覆盖、并发、失败提交、重试和未读数。 |
| `scripts/browser/rssr_task1_acceptance.cjs` | 元信息／时区／计数验收适配新槽位。 |
| `scripts/browser/rssr_reading_position_acceptance.cjs` | 阅读位置 fixture 适配新槽位。 |
| `scripts/browser/rssr_nav_collapse_acceptance.cjs` | 主题 fixture 适配新槽位。 |
| `scripts/browser/rssr_small_viewport_assertions.mjs` | 当前提交读取及旧版无计数／刷新文案断言适配。 |
| `assets/styles/shell.css` | 为窄导航搜索断点补齐收起按钮的 48px 预留。 |
| `docs/design/theme-author-selector-reference.md` | 说明搜索换行预算，稳定选择器不变。 |
| `README.md` | 说明多标签能力、浏览器要求及升级操作。 |
| `docs/user-guide.md` | 说明失败处理、最后提交语义与剩余正文写入成本。 |
| `docs/deployment/web.md` | 明确 HTTPS／localhost 要求与局域网 HTTP 限制。 |
| `docs/design/frontend-command-reference.md` | 记录提交边界与不变的公开命令语义。 |
| `docs/testing/README.md` | 登记可重复执行的多标签验收脚本。 |
| 本文件 | 保存实现、取舍、真实验证和未完成项。 |

### 公开签名与不变契约

仅 browser infra 的 Rust API 改变：

```rust
pub struct BrowserStore { /* private cache */ } // Clone + Default
impl BrowserStore {
    pub async fn open() -> anyhow::Result<Self>;
    pub async fn snapshot(&self) -> anyhow::Result<BrowserState>;
}
pub const COMMIT_STORAGE_KEY: &str = "rssr-web-commit-v1";
// 下列五种 adapter 的构造参数由 Arc<Mutex<BrowserState>> 改为 BrowserStore：
// BrowserEntryRepository, BrowserFeedRepository, BrowserSettingsRepository,
// BrowserAppStateAdapter, BrowserRefreshStore
pub fn new(store: BrowserStore) -> Self;
impl BrowserAppStateAdapter {
    pub async fn load_snapshot(&self) -> anyhow::Result<AppStateSnapshot>;
    pub async fn save_snapshot(&self, app_state: &AppStateSnapshot) -> anyhow::Result<()>;
}
pub fn compose_browser_use_cases(
    state: BrowserStore,
    client: reqwest::Client,
    clock: Arc<dyn ClockPort>,
) -> AppUseCases;
```

移除 `LoadedState`、`load_state()`、`save_state_snapshot(&BrowserState)`、`save_app_state_slice(&PersistedAppStateSlice)`、`save_entry_flags_slice(&PersistedEntryFlagsSlice)`；不再提供可绕过事务直接写片段的入口。私有 `Changes` 与 JS `withStorageLock` 不属于 application 或主题契约。

没有改变 domain repository trait、application 端口、ReaderEntrySnapshot、ReaderService::new、CLI、SQLite schema、路由或任何 data-* 接口；CSS 只修改换行断点与换行后可用宽度。Web 原有四片 JSON 内容结构保持兼容；存储工具必须通过提交记录定位活跃键。配置导入仍按既有工作流分步执行，本协议没有把整个配置导入变成一笔事务。

## 验证与验收

证据位于忽略目录 `target/p2-storage/`。本轮未安装依赖；使用已有 Rust/GTK/WebKit、Android NDK/SDK、Chromium、Playwright 与 ChromeDriver。Rust 主矩阵使用 `CARGO_BUILD_JOBS=1`，Android 在命令前加载 `target/integration-validation/android-env.sh`。

| 命令 | 真实结果／退出码 |
| --- | --- |
| `cargo fmt --all --check` | 通过，0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 通过，0 |
| `cargo test --workspace` | 304 passed、2 ignored、0 failed，0 |
| `cargo check -p rssr-app --target wasm32-unknown-unknown` | 通过，0 |
| `cargo clippy -p rssr-app --target wasm32-unknown-unknown -- -D warnings` | 初次 manual_is_multiple_of lint，101；修正后通过，0 |
| `cargo check -p rssr-app --target aarch64-linux-android` | 通过，0；不是实机验收 |
| `cargo clippy -p rssr-infra --tests --target wasm32-unknown-unknown -- -D warnings` | 最终通过，0；过程修正新旧测试 lint 及旧 items_after_test_module，各失败尝试 101 |
| `bash scripts/run_wasm_contract_harness.sh wasm_subscription_contract_harness` | 18 passed，0；经已有 run-harness.sh 设置匹配的 wasm-bindgen / ChromeDriver |
| `bash scripts/run_wasm_contract_harness.sh wasm_refresh_contract_harness` | 24 passed，0；同上环境 |
| `bash scripts/run_wasm_contract_harness.sh wasm_config_exchange_contract_harness` | 3 passed，0；同上环境 |
| `dx build --platform web --package rssr-app --locked --debug-symbols false` | 所有构建成功（最后一次 44.22s），0；仍打印既有 dx 0.7.10 / Dioxus 0.7.9 不匹配诊断，未升级 |
| `bun scripts/browser/rssr_task1_acceptance.cjs` | 两视口、元信息回退、新标签原文、计数和四组时区／UTC 回退通过，0 |
| `bun scripts/browser/rssr_reading_position_acceptance.cjs` | 两视口通过，0；连续滚动／搜索编辑各 0 次正文位置读取，切文、返回及顶部／底部恢复保持 |
| `bun scripts/browser/rssr_nav_collapse_acceptance.cjs` | 两视口 × 五主题 10/10，0；正文节点保持、无溢出、收起按钮 44×44 |
| `bun scripts/browser/rssr_storage_consistency_acceptance.cjs` | 用 UI 准备数据后连续三轮、每轮两视口通过，均 0；见下方早期失败记录 |
| `NODE_BIN=bun CHROME_BIN=... bash scripts/run_static_web_small_viewport_smoke.sh --skip-build --port 8101 --log-dir ...` | 最终 128 项通过，0；含 360×800 / 1280×800、当前和旧 Atlas 窄侧栏，早期失败见下文 |
| 本地编译的 `wasm-contract-runner --artifact <refresh wasm> <filter> --nocapture` | 两个测量筛选分别 4 passed / 1 passed，均 0 |
| `git diff --check` | 通过，0 |

### 真实浏览器及故障验证

Chromium Web 使用隔离测试 context、SPA server 与固定 RSS fixture；视口 360×800、1280×800。两个标签共用 context，各自通过 sessionStorage 登录，第二标签不重新 seed。没有使用构建成功替代 UI 验收。

- 跨标签修改不同文章、同一文章的不同标记；仓储中并发添加订阅 ID 不冲突，批量已读重新校验预览。
- 真实 Storage API 注入 quota / SecurityError：原 flags、插入新 flag、批量已读、刷新 core 暂存、content 暂存、head 发布分别验证失败不漂移。
- 清理失败不反转已提交结果；中断暂存不会被读取，后续写入可恢复。
- 排队取消、锁 5 秒超时、缺少 Web Locks 均不意外写入；回滚读取也失败时，原句柄不能暴露未提交值，恢复访问后可继续操作。
- 原有数据以旧格式 seed 并接入；损坏旧片段与缺失已提交片段以及缺失 head 但仍有新版槽位的情况均原样保留并报错；清空后相同版本号通过 epoch 区分。
- 标记操作和热缓存查询的 Storage.getItem/setItem 记录没有 core 或 content 访问；刷新元信息写入没有 flags 或 content 写入。

### 测量及边界

以下为本机 debug wasm 契约 harness、合成数据的单次测量，不是 FPS、生产基准或所有设备保证：

- 20 个订阅仅更新元信息：40 次 setItem（core + head），155,066 字符，41ms；没有 flags / content 写入。
- 12 个订阅，每个新增 10 篇、每篇约 2KiB HTML + 2KiB text：36 次 setItem，3,726,363 字符，365ms；最终 core + content 为 569,611 字节。
- 后一项揭示逐订阅提交的写入放大；没有拿最终体积作为旧实现实测，也未宣称总刷新速度提升。复制写入还需要暂存空间，配额不足会拒绝本次修改并保留旧提交。

### 失败尝试与未完成项

- 早期 adapter 未全部迁移时 wasm check 编译失败，后来通过。测试初次机械适配有 import 语法错误，已修复；不是环境阻塞。
- subscription harness 首次 15 passed / 1 failed，退出 1：旧 quota 注入只拦截原 flags 键，漏掉 `-next`；扩大到两个槽位后原断言通过，没有删失败验证。
- 扩展 wasm clippy 依次发现新代码取余写法、测试默认值赋值、旧测试 needless_update、旧函数放在 test module 之后；逐项改正后通过。
- 扩展小屏 smoke 的多次失败均退出 1，保留独立日志。先发现两项旧断言：把含数字的全部可见文本当作控件名称、仍匹配“刷新完成”；按已确认的名称／独立未读描述与“新增 N 篇”契约修正。随后在搜索输入宽度 102px／至少 140px 的断言失败。根因是先前添加箭头时未同步容器预算，本轮改为 328px 断点。旧 Atlas 的第二行仍被预留挤到 126px，继续归还第二行的 48px，保持原来的 140px 阈值。
- 扩展 smoke 还发现图片准备未等待 decode／布局，以及进入 Reader 后直接 scrollTo 与待完成的位置恢复竞争。测试现在等待 Reader ready、通过真实 CDP wheel 输入让恢复让步，再建立滚动基准；返回列表后等待异步恢复目标，不再用固定 150ms 猜测完成；图片解码后即时定位并等待两帧，保留命中测试、正文节点／内容和滚动位置不变的原断言。专门的阅读位置脚本仍验证恢复功能本身，未关闭产品恢复逻辑。
- 双标签脚本有一次收藏保留断言失败（退出 1）；随后带提交版本日志的原脚本两视口通过。原测试直接改活跃片段而不推进版本，存在不合法 fixture 写入与缓存／后台操作竞争，不能据此证明产品事务失败的根因。最终测试改为用实际 UI 命令准备已读／收藏，明确等待文章 ID 与标记状态，再重复并发／失败验收；连续三轮、两视口均退出 0。保留失败与 trace 日志，不把一次成功重跑描述为已定位根因。其余三个 Playwright 脚本的 fixture 编辑也统一为锁内写暂存片段并发布 head，避免同类前置条件错误。

## 结果、风险与后续事项

- **已验证**：仓储一致性／失败恢复契约、主构建矩阵与上列 UI 路径；双标签 UI 在真实命令准备数据后连续三轮两视口通过；完整小视口 smoke 最终 128 项通过。
- **源码支持／假设**：所有合作写入者遵守新协议且位于同一 origin；旧版本标签必须重新加载。不同字段的 flags 合并，针对同一字段的明确操作以最后成功提交为准。
- **保留的语义**：设置和 AppState 是整份保存，跨标签同时编辑同一份草稿按最后提交生效；没有逐字段草稿合并或自动 UI 重载。业务工作流仍可能包含多笔 repository 操作，不承诺整套配置导入原子性。
- **未验证**：Firefox/Safari、Android 模拟器／真机、Windows/macOS/Linux 原生 GUI 的本轮实际操作、硬件帧率、真实长期大库。Android 本轮只有编译。
- **未运行**：原生 GUI / Android 模拟器专项，本轮以 Web 视口检查共用 CSS；默认 ignored 的两个性能基准未额外运行。完整小视口 smoke 仅跑默认主题及内置的当前／旧 Atlas 专项，另外四主题由 10/10 导航矩阵覆盖，未重复整个四主题 smoke。未执行远端发布或安装新版依赖。
- **未关闭**：大量正文更新的写入放大；此前记录的原生 feed 刷新响应无字节上限也未在本轮 Web 存储范围内修改。

## 给下一位 Agent 的备注

从 `state/store.rs`、`state/storage.rs` 及两个 support 契约文件理解事务边界；不要重新引入直接改共享 BrowserState 后整库写回。常规跨标签测试应通过 UI 或 repository 操作，不要绕过提交版本直接改活跃键后继续使用旧缓存。更改脚本 seed 后应重启 SPA server，server 内嵌 Python 只在启动时载入。

仅授权本地 commit；未 push / tag / release。`.handoff/` 仍按原规则排除，task3/4/5 旧 worktree 及其任务外内容保持原状。

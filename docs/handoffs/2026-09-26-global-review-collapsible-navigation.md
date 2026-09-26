# 全局职责与性能检查、顶栏收起

- 日期：2026-09-26
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：f808e41（本轮开始基线 974c4e6）
- 相关 commit：性能与状态修复 `f808e41`；导航为本记录所在的提交
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

在上轮移除滚动位置扫描后，检查六个 crate 的依赖方向及阅读、列表、刷新、存储、配置交换与 Web 代理主要调用链，修复有源码证据的重复工作与失败路径。增加桌面和手机共用的顶栏收起入口。没有进行大规模职责迁移或引入新依赖。

## 影响范围

- `rssr-app`：导航组件 / shell 状态、阅读页快照。
- `rssr-infra`：Web 文章导航查询及已读/收藏状态写入。
- CSS、主题接口、使用指南、规格、浏览器回归。
- 实际运行验收：Chromium Web、Linux WebKitGTK；Android 为编译检查。

## 检查发现与处理

### 已修复

1. **P2，失败一致性**：Web `set_starred` 先改内存再写存储，失败没有回滚；与 `set_read` 的规则分叉。两者现在共用 `update_flags`，仅备份和回滚本次条目，既有标记和新插入标记均有真实 localStorage 失败注入验证。
2. **P2，阅读页分配**：`ReaderPageState` 保存 String 正文，快照、多个 facade 回调和标量状态读取都会复制正文。改为内部 `Arc<str>` 共享不可变正文，按钮状态读取借用 Signal；加载用例的输入/输出仍保留原 String 契约。
3. **P2，Web 切文复杂度**：为选四个相邻目标而复制引用数组并排序全库。现在按同一 `(published_at.unwrap_or(created_at), id)` 键线性选前驱/后继。最坏时间从排序的 O(E log E) 降为 O(E + flags + feeds)，移除排序临时 Vec；没有引入持久化索引或缓存失效协议。缺日期、同时间戳、删除/缺失来源和稀疏标记都与列表顺序对照。
4. **职责内聚**：`AppNav` 从应用入口移至 `components/app_nav.rs`，路由/搜索/刷新/收起状态继续复用 `ui/shell.rs`，没有第二套导航或 UI 内数据库调用。

### 保留并明确跟进的既有边界

- **P2，浏览器持久化**：`browser/state/storage.rs::save_state_snapshot` 顺序写四片 localStorage；部分写入失败不能保证跨片原子性，设置/订阅类调用仍可能先修改内存。标记操作本轮已修复，但不等于所有 Web 写入都有回滚。需要单独设计快照提交/恢复协议；简单复制整个状态回滚既不能修复跨片落盘，也会重新放大正文复制。
- **P2，下载资源边界**：原生 `fetch/client/feed_http.rs` 有请求超时，但 `response.text()` 无字节上限；`rssr-web/proxy.rs` 已有 8MiB 上限。没有在本轮静默给全部源加大小限制，以免改变大 feed 的兼容性。后续应统一说明大小预算并补流式超限测试。
- **架构债**：`rssr-application/refresh_service.rs` 仍含 wasm 串行 / 原生 JoinSet 调度分支，统一结果语义仍在 application。没有继续扩大这种平台分支。若抽取调度 host port，需要保留并发上限、panic 到订阅失败的归属、取消与结果顺序契约，不宜仅为去掉 cfg 做大规模迁移。
- 本轮没有发现需要立即重写分层的证据。domain 无平台 I/O 依赖；GUI/CLI 仍复用 `AppUseCases`；SQLite 和浏览器差异保留在 adapter；UI facade、command、runtime 的现有职责继续有效。
- 已核对但未改：列表分组 memo/Arc 投影、权威订阅计数、SQL 排序与导航查询、刷新合并/取消、正文图片限额、WebDAV 大小/超时、Web 代理地址校验/超时、配置交换服务及 CLI 装配。

上述结论来自源码及相关测试，不是所有文件的逐行安全审计，也不证明所有规模下的帧率。

## 文件变更

| 文件 | 说明 |
| --- | --- |
| `crates/rssr-app/src/components/app_nav.rs` | 独立导航组件，增加窄箭头、可访问名称、展开状态和受控区域。 |
| `crates/rssr-app/src/components.rs` | 注册导航组件。 |
| `crates/rssr-app/src/app.rs` | 移出导航视图，保留原导出路径兼容调用者。 |
| `crates/rssr-app/src/ui/shell.rs` | App 级会话收起状态；收起关闭搜索模式但保留搜索词。 |
| `crates/rssr-app/src/pages/reader_page/state.rs` | 正文改为不可变共享存储。 |
| `crates/rssr-app/src/pages/reader_page/reducer.rs` | 加载结果转入共享正文。 |
| `crates/rssr-app/src/pages/reader_page/session.rs` | 标量读取避免克隆快照，调整既有断言。 |
| `crates/rssr-infra/src/application_adapters/browser/query.rs` | 线性选择四个相邻文章目标。 |
| `crates/rssr-infra/src/application_adapters/browser/adapters/entry.rs` | 统一单条标记写入、失败回滚和单次查找。 |
| `crates/rssr-infra/tests/wasm_subscription_contract_harness.rs` | 导航顺序对照、收藏失败/重试契约。 |
| `assets/styles/shell.css` | 右侧箭头、44px 点击范围、收起后只剩按钮；不增加尺寸动画。 |
| `assets/themes/atlas-sidebar.css` | 新版 Atlas 桌面侧栏在收起时缩到 44px。 |
| `assets/themes/legacy/atlas-sidebar-v2.css` | 冻结本轮前的 Atlas CSS，保留旧设置的主题身份识别。 |
| `crates/rssr-app/src/pages/settings_page/themes/theme_preset.rs` | 识别上述历史版本。 |
| `scripts/browser/rssr_nav_collapse_acceptance.cjs` | 两个视口、五套样式的真实浏览器回归。 |
| `README.md`、`docs/user-guide.md` | 说明收起和恢复操作。 |
| `docs/design/frontend-command-reference.md`、`docs/design/theme-author-selector-reference.md` | 同步导航接口与主题契约。 |
| `specs/001-minimal-rss-reader/spec.md` | FR-023a 记录已确认的收起行为。 |
| 本文件 | 检查证据、验证结果及保留风险。 |

## 接口与行为

- 新增 `data-action="toggle-nav"`、`data-slot="app-nav-toggle"`、内容区域 `id="app-nav-content"`；按钮有 `aria-expanded` / `aria-controls` 和“向左收起导航 / 向右展开导航”名称。
- `app-nav-shell[data-state]` 从 `normal / search` 扩展为 `normal / search / collapsed`；原来源筛选、导航、刷新选择器不变。
- `AppNav(on_back: Option<EventHandler<()>>) -> Element` 签名及 `app::AppNav` 导出保持，新增 `components::app_nav::AppNav` 实现位置。
- 内部 ReaderPageState 正文为 `Arc<str>` / `Option<Arc<str>>`；ReaderPageLoadedContent、ReaderEntrySnapshot、application/domain trait、repository、CLI、schema 均不变。
- 默认展开；收起只保留可交互箭头，同次运行中切页保持，重新启动或 Web reload 恢复展开。搜索词保留，刷新结果仍通过 live region 报告。按钮外观宽 12px，触控范围至少 44×44px。
- 旧用户自定义 CSS 不自动重写；历史 Atlas 可继续识别，旧侧栏自定义列宽不会自动升级为新模板。

## 验证与验收

完整日志和截图位于忽略目录 `target/global-review/`。

| 命令 | 结果 / 退出码 |
| --- | --- |
| `cargo fmt --all --check` | 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 |
| `cargo test --workspace` | 0；304 passed、2 ignored |
| `cargo check -p rssr-app --target wasm32-unknown-unknown` | 0 |
| `cargo clippy -p rssr-app --target wasm32-unknown-unknown -- -D warnings` | 0 |
| `cargo check -p rssr-app --target aarch64-linux-android` | 0；沿用已安装 SDK/NDK 环境 |
| `cargo build -p rssr-app` | 0 |
| `dx build --platform web --package rssr-app --locked --debug-symbols false` | 0；仍报告 dx 0.7.10 / Dioxus 0.7.9 版本差异，未升级依赖 |
| `bash scripts/run_wasm_contract_harness.sh wasm_subscription_contract_harness` | 最终 0，7 passed；先启动本地 ChromeDriver、确认 /status ready，通过 CHROMEDRIVER_REMOTE 执行，子进程禁用代理 |
| `bun scripts/browser/rssr_nav_collapse_acceptance.cjs` | 0；两视口 × 默认/四内置主题共 10 组全部通过 |
| `bun scripts/browser/rssr_reading_position_acceptance.cjs` | 最终 0；两视口 2000 段正文连续滚动布局读取均为 0，返回/键盘切文/顶部末尾/host 采集通过 |
| `git diff --check` | 0 |

真实浏览器使用既有 Playwright / Chromium 151，通过 NODE_PATH、CHROME_BIN 指定工具，不新增产品依赖。

### 界面验收

- Web，360×800 / 1280×800，默认及 Atlas / Newsprint / Amethyst / Midnight：收起后只剩 44×44px 控件，无横向溢出；焦点保留、键盘展开、搜索词保留、跨页收起和 reload 重置均验收。标记变化不替换已渲染正文节点。
- 新 Atlas 桌面导航 280×112 → 44×44；默认移动导航 318×62 → 44×44。箭头固定最右，收起后回左侧；没有逐帧尺寸动画。
- Linux WebKitGTK / WSLg，1280×900，隔离测试数据：原生 Inspector 实际点击，导航 982×62 → 44×44、仅 1 个交互控件，再次点击恢复 normal / Home；截图 `native-nav-collapsed.png`。软件渲染兼容参数用于功能验收，不作为 FPS 证据。
- 滚动专项在 360×800 和 1280×800 均通过，保留操作时采集策略；没有重新引入 scroll 扫描。

### 中间失败的真实记录

- 首次 native check / Web build 因 SVG 使用了不支持的 `aria_hidden` Rust 属性失败，改为字符串 `aria-hidden` 后通过。
- 初次 app tests 因两处原 String 断言未改为 Arc 的借用形式失败，修正后工作区测试通过。
- 两次 wasm harness 自动启动驱动返回 connection reset，并在失败清理阶段显示 SIGKILL，退出 1。复用既有 ready-driver 方法后 7 项通过，不能将该 SIGKILL 单独解释为浏览器崩溃或 OOM。
- 首轮最终滚动专项手机通过、桌面首次进入阅读页等待超过 15 秒，退出 1，未进入性能断言。结束原生窗口后单独完整重跑两视口通过；没有修改断言或延长等待，未确定首次超时根因，保留日志 `position-browser.log` 与 `position-browser-retry.log`。

## 结果、假设与未验证项

- 已验证：行为回归、全平台编译检查、Web 真实失败回滚、导航对照、布局与可访问控件。
- 源码证明：正文共享减少快照字节复制；导航不再构造排序 Vec，并具备线性最坏时间界限。没有据此编造毫秒收益或固定 FPS。
- 假设：文章 ID 唯一，沿用现有持久化契约；本轮没有新增缓存一致性前提。
- 未运行：Android 实机/模拟器、Windows/macOS 界面、物理滚轮帧率和全库十万篇端到端基准。编译通过不等于这些平台体验通过。
- 未运行 refresh/config-exchange wasm harness：对应适配器未改，新增及变更的文章查询/标记由 subscription harness 覆盖。
- 保留上文列明的存储事务、下载资源预算与刷新调度技术债；未提交任务外 worktree 内容，未 push / tag / release。
- 本轮启动的原生窗口、SPA 服务器及 ChromeDriver 在验收后停止；日志、截图和隔离测试数据保留于忽略目录。

## 给下一位 Agent 的备注

界面入口为 `components/app_nav.rs` 与 `ui/shell.rs`，查询入口为浏览器 `query.rs::reader_navigation`。不要为了简化相邻查询而改变日期/ID 排序或删除来源过滤。处理 Web 多片快照时先设计失败恢复，再考虑吞吐优化；不要重新深克隆全部正文来补回滚。

# Home / Read、共享刷新与 Reader 交互

- 日期：2026-09-22
- 作者 / Agent：Codex
- 分支：`main`
- 实现基线 HEAD：`1b5f8ebf8f7b50d576df2dc1e2ae4eec0ede4d23`
- 相关 commit：`d0d98da`（共享 UI）、`9d03652`（浏览器验收）、`2a1687a`（工具）；原实现阶段为 pending，现已分批本地提交
- 相关 tag / release：基线 v0.1.14；后续已获授权分批本地提交，未打 tag、push、触发远端工作流或发布
- 状态：`validated`（本机 Rust / Web 自动化；Android 实机及原生桌面 UI 未验）

分批提交清单与后续本地提交授权见[提交交接](2026-09-22-batched-local-commits.md)；以下保留各实施阶段的验证证据与平台缺口。

后续 `review and fix` 的两项修复及最新 252 项 Rust / 93 项浏览器验收见[复审交接](2026-09-22-home-reader-review-fixes.md)；以下保留初次实现的过程与验证记录。

## 工作摘要

在现有 command / runtime / application 边界内，把 R 统一为 Home / Read，补齐可复用的手动刷新、展开搜索、下拉刷新、固定分页、原生文本选择验证和图片查看器。初始工作树干净，读取了根 AGENTS / CLAUDE、页面与 browser bootstrap 附近约束及近期 handoff；fetch 后确认 HEAD 与 origin/main 一致。没有 CodeGraph 索引，结构核对使用源码和 rg。

## 影响范围

- `rssr-app`：AppNav、shell 状态与命令、host refresh capability、Entries / Feeds / Reader、移动返回 hook。
- `assets/styles/` 与 Atlas Sidebar 内置主题：导航、来源选择、目录、分页、lightbox。
- `rssr-web/src/smoke.rs`：仅启用 smoke helpers 时使用的浏览器测试入口和 feed fixture。
- 既有固定小视口 assertions、SPA fixture、release regression 参数传递，以及命令 / selector / 覆盖文档。
- 共享 Rust / Dioxus 代码影响 Desktop、Web、Android；`rssr-domain`、`rssr-application`、`rssr-infra`、Kotlin、依赖及 Cargo.lock 未改动。

## 关键变更

### 导航与刷新生命周期

- `ui/shell_state.rs` 的纯 resolver 只将全局 Entries route 的 R activation 分派为手动刷新；Reader、FeedEntries、Feeds、Settings 上仅导航。
- `NavMode::Normal / Search` 代替旧导航折叠状态；R 始终保留，Reader 返回在最左侧。搜索展开取代 S / Settings，Enter 保持标题搜索，Esc 收起并还原焦点；输入法 composing 期间不截走 Esc，未添加键盘字母 R 快捷键。
- `AppShellState::manual_refresh` 在生成任务前同步取得 in-flight 状态，任务在 App scope 中运行。R、下拉和 Feeds“刷新全部”共用 `ShellCommand::ManualRefresh`，原 Feeds 刷新实现移至 shell runtime，仅保留一份。
- `bootstrap/refresh_flight.rs` 由 native / browser 现有 capability 共用，合并与自动刷新的重叠请求。仅保留进行中结果；成功、失败可共享，owner 被取消会唤醒等待者报错，下次请求可重新开始。原应用刷新用例仍负责抓取、解析、并发与存储。
- 刷新完成（包括部分失败）只增加列表失效 revision，Entries / Feeds 重取快照；Reader 不订阅该 revision。成功 / 错误提示脱离文档流，避免完成通知移动正文。
- `data-action="activate-home"` 明确承载复合动作；`data-nav` 不承载刷新。旧 show/hide-top-nav、brand-name、reader-toolbar selector 已从当前契约移除。旧 `nav_hidden` / `rssr-nav-hidden` 被忽略；搜索词和文章控件折叠偏好保留。

### 列表与移动交互

- 分页仅渲染一份，作为 page panel 的同级固定控件，避开 backdrop-filter 对 fixed 的包含块影响。按钮 44px，预留 safe area 与末尾空间，切页回到新页起点，保持筛选和全量目录映射。
- 来源 checkbox 选择行完整换行；选择区限制总高度并独立纵向滚动，不再依赖 title / hover 识别名称。极端无空格文本使用 anywhere 换行。
- 目录侧栏增加 min-width / grid track 约束，名称与计数上下排列，修复截图发现的裁切。手机来源目录同样允许完整名称换行。
- `pull_refresh.rs` 持有方向、顶部、80px 阈值和 release 判定；JS 只发送坐标、滚动、目标与选择事实。监听仅挂全局 Entries，passive，无 pointer capture / 全局 preventDefault；Reader 无该监听。pulling / armed 和 shell 的 refreshing / finished / error 组成反馈。
- Atlas Sidebar 保留侧栏风格，但为共用五图标行预留宽度；移动端不再将导航改成 static。

### Reader

- UI 返回和 Android 系统返回共用原 history / fallback 策略；图片查看器打开时先关闭图片。移除标题下方旧大返回按钮。
- `ImageViewerState::Closed / Open` 由 Rust 持有；受控 DOM delegation 从 sanitizer 之后的 img 读取 currentSrc / src / alt。未放宽 sanitizer，未引入 inline handler 或新剪贴板系统。
- 使用原生 dialog 管理 modal focus / inert；支持点击、Enter / Space、Esc、背景和关闭按钮。DOM adapter 只负责监听、显示、滚动锁定及恢复，卸载时移除监听并恢复滚动 / 焦点。
- 极高图片曾撑开 grid 隐式行；实际 `2000×12000` fixture 复现后，以明确的 minmax grid 行列约束修复，横竖图均落在查看器视口内。
- 保留 modifier 组合键放行策略。未发现默认 CSS 阻止文字选择，因此未新增 user-select 覆盖。跨端基本查看器共用；Android 原生 pinch / 选择手柄仍须实机确认。

### 性能与误截断审计

- Entries 的 entries / feeds 集合改为 Arc 共享，snapshot 不再复制完整集合，投影输入以 Arc 指针快速判等。条目标记变化使用 copy-on-write；已有 memo 与分组键不变量继续生效，卡片读取最新状态，避免陈旧标记。
- 日期按 UTC Date 分桶，每桶只格式化一次日期文字。源码核实 source/month 路径此前已直接生成所需结构，没有重复实现。
- 保留 `entry_query().limit = None`、全结果目录、分组 count / 起点 / 页号语义。没有 SQL LIMIT/OFFSET，也没有虚构存储 aggregation contract。
- 扫描默认样式、内置 / legacy 主题与组件中的 ellipsis / nowrap / overflow / max-width。核心 feed、entry、reader 标题及 metadata 保持可换行；仅 sr-only 辅助内容保留裁切 / nowrap；正文阅读宽度、图片适配上限、表格独立横向滚动属于有意约束。没有修改用户自定义 CSS。

同一 native release fixture、同一主机、800 / 2000 条、40 个来源、每页 50 条；snapshot 与 input+eq 各循环 10,000 次，presenter 各循环 300 次。迁移前先加入相同 ignored benchmark 采基线，再改集合与分桶；以下是单轮每次平均 ns，不是端到端帧时间。

| 操作 | 800 条：前 → 后（ns） | 2000 条：前 → 后（ns） |
| --- | ---: | ---: |
| state snapshot | 11,189 → 48 | 27,671 → 55 |
| presenter input + equality | 15,237 → 23 | 29,694 → 28 |
| Time presenter 完整构建 | 799,975 → 679,636 | 1,677,068 → 1,593,285 |
| Source presenter 完整构建 | 425,634 → 494,200 | 1,340,803 → 1,196,996 |

明确收益是这些 fixture 下集合快照和投影输入的复制成本下降。Source 800 条构建反而变慢，分组测量有波动，不据此承诺整体 presenter、帧率或用户可感知加载速度提升。选中 URL、状态文字等小字段仍按现有 state clone；未测内存峰值或移动端耗时。

### 回归工具与文档

- 扩展现有 `rssr_small_viewport_assertions.mjs`，未建立第二套 QA 框架。CDP 暂停 / 放行真实 RSS 请求验证去重、自动与手动合并、页面卸载后继续完成、错误与重试、Reader DOM / 滚动稳定。
- 新 home-reader fixture 包含两个来源、每页两篇、无空格长来源、正文长文本和本地 data URI 图片；不读取用户 localStorage。
- 修复 release 聚合脚本没有向部分子脚本传 profile、`--skip-automated` 却标记 passed 的记录错误。
- rssr-web 现有 helper 使用父 window 的 Event 会使 iframe 输入变为空串；换为 iframe Event 后添加 / 刷新通过。随后发现固定 4 月 fixture 已超过默认 3 个月归档窗口，改用当前 UTC RSS 日期，GUID 不变；再次运行完整成功。生产错误提示现在保留原始原因链，避免只显示“保存订阅失败：保存订阅失败”。
- 更新 README、frontend-command-reference、theme-author-selector-reference、shell 边界文档、小视口说明、rssr-web smoke 说明与 release 覆盖矩阵。

## 验证与验收

### 自动化验证

日志与新产物统一位于 `target/home-reader-review/`（git ignored），不是旧版本发布收据。

- `cargo fmt --all --check`：通过。
- `cargo clippy --locked --workspace --all-targets -- -D warnings`：通过。
- `cargo test --locked --workspace`：250 passed，0 failed，1 ignored（显式性能测量）；33 组含 doc tests。后续小修改也重跑了受影响的 shell prefs / style checks。
- `cargo check --locked -p rssr-app --target wasm32-unknown-unknown`：通过。
- `cargo check --locked -p rssr-app --target aarch64-linux-android`：环境阻断，E0463；未安装该 Rust target，常用路径也无 NDK。没有把 Web touch emulation 当作 Android 构建或实机通过。
- `dx 0.7.9 build --platform web --package rssr-app --release --locked --debug-symbols=false`：通过，最终日志 `web-build-final.log` 无 WARN / ERROR。
- `cargo test --locked -p rssr-app --release --bin rssr-app measure_entries_projection -- --ignored --nocapture --test-threads=1`：前后 fixture 计时见 `perf-before.log` / `perf-after.log`。
- `bash scripts/run_static_web_small_viewport_smoke.sh --release --skip-build`：默认主题 86 项通过，360×800 / DPR 3 与 1280×800，console errors / ignored console errors 均为 0；最终产物 `default-final/`。
- `bash scripts/run_static_web_small_viewport_smoke.sh --release --skip-build --preset atlas-sidebar`：86 项通过，console errors / ignored console errors 均为 0；`atlas-final/`。与默认主题串行执行。
- `bash scripts/run_static_web_reader_theme_matrix.sh --release --skip-build`：默认 + 四个内置主题通过，最终产物 `themes-final/`。
- `bash scripts/run_rssr_web_browser_feed_smoke.sh --release --skip-build --port 18131`：真实登录、添加订阅、首次刷新、手动单源刷新和 feed entries 通过，`rssr-web-browser-feed-final4/`。
- `bash scripts/run_release_ui_regression.sh --release --skip-build --skip-automated --with-fixed-smokes --no-serve --port 8120 --web-port 18120`：已实际运行；主题 / 小视口通过，公网 proxy feed 项失败，未声称整套通过。目标 www.ruanyifeng.com 被当前 DNS 解析为 198.18.0.119，仓库 SSRF 规则返回 HTTP 400；没有放宽规则。其后被中止的本地 browser feed 项单独修复并通过。
- `node --check scripts/browser/rssr_small_viewport_assertions.mjs`、修改脚本的 `bash -n`、`git diff --check`：通过。

### 验证环境与工具限制

- Linux / rustc 1.97.0；未改依赖。GTK / WebKit dev 包缺失且无 sudo，使用官方 Debian 包解压到 `/tmp/rssr-native-deps`，设置临时 pkg-config / library 路径后执行 native 编译、测试和 Clippy；未安装到系统目录。
- 本机默认 dx 0.7.10 与项目 Dioxus 0.7.9 不匹配，而且首次 release 优化出现 wasm-opt DWARF SIGABRT 后仍退出 0。旧工具生成的基线可用于行为对比，**不能**称作干净的优化构建。后续使用官方 dx 0.7.9，关闭 debug symbols 后干净重建；项目依赖未升级。
- 临时 dx 下载 SHA256：`3b132551b480bc96f938f9f0d37936ee1190f994977539dcc347eaf38540d005`。
- Chromium：`/home/deve/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome`。固定小视口与主题脚本设置 CHROME_BIN；旧 rssr-web 脚本硬编码 google-chrome，使用 `/tmp/rssr-review-bin/google-chrome` 临时软链接。浏览器检查串行，使用隔离 profile / 本地端口。
- Chrome 进程 stderr 有环境 UPower / DBus 提示；应用页面的 CDP console error 是单独检查的，不能把二者混为一谈。

### 截图及交互复核

- 复核导航、Reader 搜索、完整来源名、固定分页、桌面目录、lightbox 及主题矩阵新截图；截图发现的透明背景文字重叠、目录计数裁切、Atlas 旧侧栏布局均已处理。
- 浏览器自动化实际执行鼠标拖选、Ctrl+C 复制并读取原生 clipboard 对照、Ctrl+A 全选；未新增产品 clipboard 服务。图片展开 / 关闭和刷新成功 / 失败均检查正文 DOM 与原滚动位置。
- 未运行：Windows / macOS 原生桌面 UI，Android 实机长按选择、系统返回和 pinch，macOS Cmd+C / Cmd+A；本机没有对应设备 / 宿主环境。Linux native 编译单测不是这些 UI 行为的验收。

## 结果

- 实现结束时工作树保留为可 review 的未提交改动，后续已分批本地提交；原 domain / application / infra 方向未变，无新增 crate、service trait、平台业务分叉、数据库分页语义变更或发布动作。
- 新交互在共享 Rust reducer / resolver / capability 中定义；JS 仅保留必须依赖 DOM 的事件、focus、scroll 适配，平台生命周期仍沿用原宿主机制。
- 本轮不是发布许可或 Android 实机验收完成证明。

## 风险与后续事项

- 在 Android 和原生桌面 WebView 上确认选择手柄、返回优先关闭图片、系统 pinch / safe area 与 DOM cleanup；尤其旧 WebView 的 dialog / CSS 支持不能仅由 Chromium 证明。
- 自定义主题可能仍针对退役 selector 或设置非 sticky / 省略名称样式；本轮只更新默认及内置主题契约，不强制覆盖用户 CSS。
- 公网 proxy feed 集成在正常 DNS 网络下补验；当前环境的拒绝结果不能当作真实公网成功证据。
- SQL pagination、存储 aggregation contract、跨标签页刷新互斥、整页帧率和内存峰值均未扩展。

## 给下一位 Agent 的备注

- 先看 `ui/shell_state.rs`、`ui/shell.rs`、`bootstrap/refresh_flight.rs` 和 command reference。不要把 refresh_all 拷回 Entries 或重绑到页面级 spawn。
- 图片 viewer 与 pull bridge 的 dispose 是成对的；不要加全局手势 preventDefault 或吞掉 modifier 快捷键。
- 基线 / 中途失败日志保留用于解释根因；最终结果以本记录所列最终目录为准，不能从旧目录名中的 final 推断验收状态。

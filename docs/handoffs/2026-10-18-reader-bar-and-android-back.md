# 阅读页底栏固定与快捷键；Android 系统返回键

- 日期：2026-10-18
- 作者 / Agent：Claude Code（Opus），与 Develata
- 分支：v0.2-architecture
- 当前 HEAD：见两次提交（基于 `de0aad8`）
- 相关 commit：阅读页一次、Android 返回键一次（本记录随后者提交）
- 相关 tag / release：N/A
- 状态：`draft`（阅读页已在浏览器验收；Android 返回键待真机验收）

## 工作摘要

Develata 试用时提出：短文章里阅读页底栏（上一未读 / 已读 / 收藏 / 下一未读）随正文长短上下跳动，无法连续点击；希望给上一 / 下一未读加快捷键。另报 Android 上系统返回键直接退出应用，以及进过阅读页、点过首页后返回失灵。顺带修掉 Codex 审计（`v0.1.23`）在 v0.2 上仍残留的一处：阅读页加载中按下的已读 / 收藏切换会被迟到的加载结果盖掉。

## 影响范围

- 模块：
  - `crates/rssr-app/src/pages/reader_page/`、`hooks/use_reader_shortcuts.rs`、`hooks/reader_shortcuts.js`、`ui/reading_position.js`、`assets/styles/reader.css`、`responsive.css`
  - `crates/rssr-app/src/hooks/use_mobile_back_navigation.rs`、`pages/entries_page/mod.rs`、`assets/styles/entries.css`、`android/MainActivity.kt`
  - `scripts/browser/rssr_small_viewport_assertions.mjs`、`scripts/run_web_spa_regression_server.sh`
- 平台：
  - 阅读页：Web、桌面、Android
  - 返回键：仅 Android（其余平台的钩子不做任何事）
- 额外影响：
  - `docs/agent-handoff.md`、`docs/testing/manual-regression.md`

## 关键变更

### 阅读页底栏与快捷键

- 原因：底栏是 `position: sticky`，只在页面能滚动时贴底；短文章里它排在正文后面，位置随正文长度变。
- 底栏改为 `position: fixed` 贴视口底部，并移到阅读页 `article` 之外：amethyst-glass 主题给阅读页加了 `backdrop-filter`，会把里面的 fixed 定位困在阅读页里（做法同文章列表的翻页条）。页面底部用 `.app-shell:has(...)` 留出底栏高度，去掉原先为 sticky 留的 92 / 84px 下边距。
- 快捷键改为监听整个阅读页（`reader_shortcuts.js` 经 `document::eval` 把动作发回 Rust），不再要求焦点在阅读区：此前页面刚打开、点过阅读区以外、或焦点所在按钮变成禁用后按键都没反应。新增 J（下一篇未读）/ K（上一篇未读），保留 ← / →、M、F；底栏按钮标出（K）（J）。输入框与可编辑区域里的按键、带修饰键、输入法组字中、按住不放的重复、图片查看器打开时都不触发。切文前记阅读位置的 `reading_position.js` 用同样的边界。
- 已读 / 收藏只在本篇加载完成且没有出错时可切换（`ReaderPageState::flags_known`）；加载中按钮禁用、M / F 不生效。原因：加载中屏幕上的标记被清成 false，按下的切换又会被迟到的加载结果覆盖，界面与库里不一致（Codex 审计在 v0.2 上的残留，审计各条在 v0.2 上的状态见下方“Codex 审计在 v0.2 上的对照”）。

### Android 系统返回键

- 原因（读源码确认）：tao 0.34.8 在 Android 上不接输入队列（`ndk_glue.rs` 那段整体注释掉），Rust 收不到返回键；返回键一直由 wry 的 `WryActivity.onKeyDown` 处理：WebView 能后退就后退，否则交给系统（退出或退到后台）。应用内跳转用的是 Dioxus 的内存路由，不进 WebView 历史，所以几乎总是直接退出；WebView 历史里只有首页目录写下的页内锚点，此时按返回只是无声地弹掉锚点（“返回失灵”的最可能来源，推断，未在真机上确认）。v0.1.23 与 v0.2 这条链路相同。
- `MainActivity` 关掉 `handleBackNavigation`，用 `OnBackPressedDispatcher` 接住返回键，经 JNI（`requestAppBack`）通知 Rust；Rust 侧 `tokio::sync::Notify::notify_waiters` 只唤醒当前页面的等待者，换页途中按下的那一次丢掉，不会留到下一页。
- 页面处理：先关图片查看器，再回上一页，没有上一页去各页的回退页面（首页或订阅列表）。首页（`fallback_route` 为 `None`）不沿历史后退：第一次按显示“再按一次返回键退出”，5 秒内仍在首页再按，经 JNI 调 `MainActivity.exitToBackground`（`moveTaskToBack(true)`）退到后台；超过 5 秒或从后台回来，提示清除。
- 删去在 Android 上到不了的 `KeyboardInput` / `CloseRequested` 分支及其按键映射测试；`Resumed` / `Focused(true)` 恢复窗口的处理保留。

### 阅读页 ← 按钮在 Android 上卡住（2026-10-19，Develata 录屏）

- 现象：装上本记录的测试版后，系统返回键正常；但“进入阅读页 → 点 R 回首页 → 再进阅读页 → 点左上角 ←”时，连点约 3 秒没有反应，之后才回到首页（录屏逐帧确认：第一次进阅读页时 ← 立即生效）。
- 应用内的 ← 在 Android 上走的是与系统返回键相同的异步路径：先经 `document::eval` 采集阅读位置（最多 500ms）再后退，期间重复点击合并；桌面与 Web 上同一个按钮是同步后退的。排除了：正文过大（这篇 24 KB、没有内嵌图片）、重复 push（列表只有一个 `Link`）、刷新占住界面线程（多线程 tokio 运行时）、有代码替换路由（没有）。卡住的确切机制没有找到；应用的 tracing 写到标准输出，Android 上不进 logcat。
- 修改：← 在所有平台上同步后退（`navigate_back`）。点击是 DOM 事件，`reading_position.js` 在捕获阶段、路由切换之前已记下位置，不需要再等一轮。系统返回键没有 DOM 事件，保留先采集再后退（`navigate_back_after_capture`，仅 Android）。待真机确认；若 ← 仍卡，说明原因不在这条路径，需用 `chrome://inspect` 连上 WebView 看。

### 测试脚本

- 浏览器断言新增：底栏为 fixed 且在阅读页之外、离视口底部 8–24px、页面滚到底时最后一个控件在底栏之上；输入框里按 K 不切文；不先聚焦即可用 K 切到上一篇未读，读完后 ← / → / J 都不跳转。
- `run_web_spa_regression_server.sh` 的静态服务器把连接积压从 5 调到 128（仍单线程）：Windows 上浏览器并行请求 wasm 的 JS 片段时，多出来的连接被直接拒绝，页面空白。
- “初始读取失败”一项原先等阶段 B2 之前的底层报错文字；B2 起界面只显示通用错误，改为检查“初始化应用失败”且不声称文章库为空。这一项在本次改动之前就过不了。

### Codex 审计（`v0.1.23`）在 v0.2 上的对照

Develata 转来的 Codex 审计针对 `e4c4a82`（`v0.1.23`）。逐条对照 v0.2（`de0aad8`）的源码与提交记录（子代理读代码，两处“仍存在”由本会话亲自核对），没有动态复现：

| 审计项 | v0.2 上的结论 |
|---|---|
| P1 两次刷新交错，留下“新标题＋旧正文＋新 ETag”，之后 304 不纠正 | 原生已修复：单库单事务写入、持久化刷新序号、304 只在请求基线等于当前已发布版本时生效（`83e2136`、`22c944f`、`c1864b8`、`8f3fbe0`、`aa9098e`）。浏览器原子性已修复；浏览器不发条件请求，304 那一半不适用 |
| P2 同一代际内迟到的旧响应覆盖新内容 | 原生已修复（同上）；浏览器同一标签页已修复。**浏览器跨标签页仍存在**：浏览器存储没有刷新序号，`begin_refresh` 走默认实现不领号，`commit` 只查代际、恒返回 `superseded: false`（已核对）。Web 在 C4 / D 改走服务器后这套存储删除，暂不修；最小修法是照搬原生的序号 |
| P2 旧查询覆盖刚改的已读 / 收藏显示 | 列表页已修复（`2418c14`、`493a158`、`beb3d17`）。阅读页的同类残余：加载结果无条件覆盖标记（`reducer.rs` 的 `ApplyLoadedContent`，已核对），本次修复（加载完成前不可切换） |
| 低 P2 AppState 整份保存 | 已修复：原子窄字段更新、设置按补丁合并、配置替换先比对（`abc2f8f`、`3f39022`、`cbe4cc4`、`01b54d5`）。残余两处没有可见影响，不处理：迟到的“记住打开的订阅”可能写入已删除的 id（启动时过滤）；文章页偏好整块替换，两个界面实例改不同字段时后写者胜 |

## 验证与验收

### 自动化验证

Windows（rustc 1.99.0，基准 `de0aad8`；日志在会话临时目录 `evidence-reader-ui/`、`evidence-android-back/`；构建 `-j 2`）：

- 阅读页改动：`cargo clippy -p rssr-app --all-targets -- -D warnings`、wasm 目标 clippy、Android `cargo check -p rssr-app`：退出码均为 0；`cargo test -p rssr-app`：133 passed / 0 failed / 1 ignored（含新增 `flags_are_known_only_once_the_entry_has_loaded`）。
- 浏览器（`scripts/run_static_web_small_viewport_smoke.sh --skip-build`，debug web 包由 dioxus-cli 0.7.9 构建，本机 Chrome，360×800 与桌面视口）：通过，282 项断言，控制台无错误。前几次失败与改动无关：冷构建超过脚本等待服务器的时限（改为先构建再 `--skip-build`）；静态服务器拒绝连接（见上）；B2 起过时的一项断言（见上）；新增的一项断言起初找了未展开的搜索框（改为临时插入的输入框）。截图看过手机与桌面阅读页：底栏贴底、桌面显示快捷键提示。
- 两组改动合在一起：`cargo clippy -p rssr-app --all-targets`、wasm 目标 clippy：退出码 0；`cargo test -p rssr-app`：131 passed / 0 failed / 1 ignored（删去 2 个到不了的按键映射测试）；Android `cargo check -p rssr-app -p rssr-cli`：退出码 0。Android 目标的 `cargo clippy -p rssr-app --no-deps` 报 2 处，都是这个文件里原有的代码：`thread_local` 已写成 `const` 仍报（同一误报也出现在 `rssr-infra` 的 `store/executor.rs`），以及原有的 `let _ = window.set_focus();`；CI 对 Android 只跑 `cargo check`。

### 手工验收

- Android 真机：未执行。需要：各页逐页返回、阅读页先关图片、首页两次返回（5 秒内 / 超过 5 秒）、在首页点过目录后返回、退到后台再回来提示不残留。`MainActivity.kt` 只有在打 APK 时才编译，本次没有打 APK。
- 桌面实际使用：未执行（浏览器验收覆盖了同一套页面代码）。

## 结果

- 阅读页改动可合并。Android 返回键代码完成，待真机验收后再标 `validated`。

## 风险与后续事项

- 底栏在 atlas-sidebar 主题的宽屏布局下按视口居中，不按内容列居中（与文章列表翻页条相同）。
- Codex 审计在 v0.2 上仍存在的一条：浏览器多个标签页同时刷新同一订阅时的乱序（浏览器存储没有刷新序号）。Web 在 C4 / D 改走服务器后这套存储会删除，暂不修。
- 首页目录写页内锚点，Web 端浏览器的返回会先逐个走过锚点；不在本次范围。

## 给下一位 Agent 的备注

- 快捷键：`hooks/use_reader_shortcuts.rs` 与 `hooks/reader_shortcuts.js`；切文前记位置的边界在 `ui/reading_position.js`，两处要一起改。
- Android 返回：`hooks/use_mobile_back_navigation.rs`（`android` 模块里的 JNI）与 `android/MainActivity.kt`。

# 高内聚低耦合与过度优化复查

- 日期：2026-09-26
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：515ee11（本轮起点，开始时工作树干净）
- 相关 commit：本记录所在提交（`fix(ui): narrow reader capture and unify navigation state`）
- 相关 tag / release：N/A
- 状态：`validated`（本轮修复；下述既有多标签页问题未关闭）

## 工作摘要

复查近期阅读位置、正文共享、浏览器查询/标记写入及导航优化，同时检查六个 crate 的依赖方向、列表分组投影、刷新任务生命周期和持久化边界。修复无关键盘输入触发正文扫描的问题，合并导航状态，并纠正以本机测量推导通用写入要求的注释。没有引入新缓存、依赖或存储协议。

## 影响范围

- 模块：`rssr-app` 的阅读键盘 hook、位置 DOM bridge、shell 状态和偏好说明；浏览器回归脚本及文档。
- 平台：共享代码影响 Web、Linux/Windows/macOS、Android；实际 UI 验证为 Chromium Web、Linux WebKitGTK。
- 额外影响：无 schema、持久化格式、CLI、构建 workflow 或发布变更。
- CodeGraph：Linux 工具存在，但仓库没有 `.codegraph/`，使用 `rg`、源码与 Git 历史；未建立索引。

## 检查发现与关键变更

### P2：既有浏览器整片写回会覆盖另一标签的标记，未修复

已在同一 Chromium context 的两个真实标签页复现：

1. 用 `reader-demo` 建立两篇文章，初始均未收藏；分别解锁两个标签页，均先加载旧状态。
2. A 收藏文章 1，localStorage 中文章 1 的 `is_starred=true`。
3. B 收藏文章 2，文章 1 的 `is_starred` 被写回 `false`，文章 2 为 `true`。

证据：`target/cohesion-review/multi-tab-final.log`，保护 A 收藏的断言失败，退出码 1。
源码入口：`crates/rssr-infra/src/application_adapters/browser/state/storage.rs::save_entry_flags_slice`。
Git 历史确认 `e248098` 把单条标记的读改写替换为内存快照整片写回；`77c85fe` 已在注释承认跨标签覆盖频率扩大。这是旧优化的适用前提超出单标签页后的正确性问题，不是本轮新增。

保守处理：在用户指南说明当前单标签写入边界。没有声称简单加回一次读改写就能修好：它仍有跨文档并发竞争，且 `save_state_snapshot` 的刷新/设置入口仍会覆盖四个片段。完整修复应统一 Web adapter 的提交/恢复与跨标签一致性，application/domain 语义、原生 SQLite 和公开命令应保持不变。仅为这一问题复制整库回滚、增加 UI 缓存或迁移全部存储都不合适；它需要独立的设计和失败/并发契约验证。

### P2：无关方向键仍扫描正文，已修复

- 原位置 bridge 在 document 捕获全部左右方向键，包括顶栏搜索框内的光标移动、系统修饰键和输入法组词。
- 在 2,000 段正文约 80% 位置，搜索框左右各按一次，修复前发生 **3,212 次**正文元素 `getBoundingClientRect` 调用；新增断言在基线失败（退出码 1）。
- 现在只在 `reader-shortcut-scope` 内、无修饰键且非 composing 的左右键上采集；Rust 阅读快捷键同步放行 composing。
- 修复后 Web 两个视口和 Linux WebKitGTK 的上述读取为 **0**。连续滚动也为 0；正常切文、浏览器返回、顶部/底部恢复及 native capture bridge 回归仍通过。
- 这些数字是 DOM 位置读取次数，不是 FPS 或整页耗时。

### 导航状态内聚与过度约束清理

- 移除独立 `Signal<bool>` 收起状态，统一为 `NavMode::{Normal, Search, Collapsed}`。搜索和收起互斥，收起从搜索返回时恢复普通导航；收起后的过期搜索事件不会重新展开导航。
- 状态转换留在 `shell_state.rs`，shell 负责绑定 Signal，组件和 CSS 接口不变。没有新增通用 reducer/状态机框架。
- `shell_prefs.rs` 原注释把“首帧同步读取”扩大成“必须同步读写”，并用本机 0.5–1.1ms 写盘推断一般情况下不卡顿。现在只说明当前实现的取舍；同步写入代码未变，没有未经测量引入 debounce/后台队列。
- README 中“R 始终可见”与已实现的收起冲突，改为展开导航后可用。

### 保留的优化与边界

- 保留 Reader `Arc<str>`：共享不可变正文，无新增缓存失效协议；标量读取避免整份快照克隆。
- 保留 Web 相邻文章的线性前驱/后继选择：复用列表排序键，O(entries + flags + feeds)，没有持久索引。重跑 wasm 契约覆盖缺日期、同时间戳、删除/缺失来源和稀疏标记。
- 保留列表 memo 的分组键投影：相等性保证数量/顺序/id/分组字段一致；卡片读取最新状态，投影不暴露过期标记。单纯删掉这层封装会损失已有不变量。
- 保留恢复的 2 秒期限、输入取消、消息序号，以及 Android 返回超时和重复请求合并：它们服务真实的异步/取消边界，不是无依据的缓存层。
- 六个 crate 的依赖方向未改变；本轮未找到需要大规模职责迁移的证据。application 的既有原生并发/wasm 串行调度分支仍在，未因追求形式上的统一引入新调度端口。
- 浏览器四片 localStorage 无跨片原子性、原生 feed 下载无字节上限仍是待跟进项，详见 [上轮记录](./2026-09-26-global-review-collapsible-navigation.md)。

## 改动文件与接口

| 文件 | 说明 |
| --- | --- |
| `crates/rssr-app/src/hooks/use_reader_shortcuts.rs` | 阅读命令放行输入法组词事件。 |
| `crates/rssr-app/src/ui/reading_position.js` | 将方向键测量限定到阅读快捷键的作用域和事件条件。 |
| `crates/rssr-app/src/ui/shell_state.rs` | 合并导航三态并覆盖状态转换约束。 |
| `crates/rssr-app/src/ui/shell.rs` | 删除冗余收起 Signal，使用统一转换。 |
| `crates/rssr-app/src/ui/shell_prefs.rs` | 纠正同步读写和旧导航行为的注释，不改变存储实现。 |
| `scripts/browser/rssr_reading_position_acceptance.cjs` | 增加搜索光标、修饰键、composing 及未处理 JS 错误断言。 |
| `scripts/browser/rssr_nav_collapse_acceptance.cjs` | 增加未处理 JS 错误断言，沿用两视口五主题验收。 |
| `README.md` | 修正收起后的 R 可见性与阅读按键说明。 |
| `docs/user-guide.md` | 同步编辑按键行为，明确已复现的多标签写入边界。 |
| `docs/design/frontend-command-reference.md` | 记录统一导航状态和位置采集作用域。 |
| 本文件 | 保存本轮发现、结果、失败尝试和未完成项。 |

无公开 trait、application 端口、`ReaderEntrySnapshot`、`ReaderService::new`、repository、CLI、schema 或 `data-*` 契约变更。`AppNav` 签名和 `app::AppNav` 导出保持不变。
内部 `NavMode` 增加 `Collapsed`；`toggle(self) -> Self` 改为 `toggle_search(self) -> Self`，新增 `close_search(self) -> Self` 与 `toggle_collapsed(self) -> Self`，均为 `pub(crate)`。

## 验证与验收

日志、截图和临时诊断脚本在忽略目录 `target/cohesion-review/`。Rust 构建设置 `CARGO_BUILD_JOBS=1`；Android 使用已安装环境 `target/integration-validation/android-env.sh`，本轮未安装工具。

| 命令 | 真实结果 / 退出码 |
| --- | --- |
| `cargo fmt --all --check` | 通过，0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 通过，0 |
| `cargo test --workspace` | 304 passed、2 ignored、0 failed，0 |
| `cargo check -p rssr-app --target wasm32-unknown-unknown` | 通过，0 |
| `cargo clippy -p rssr-app --target wasm32-unknown-unknown -- -D warnings` | 通过，0 |
| `cargo check -p rssr-app --target aarch64-linux-android` | 通过，0；非实机验证 |
| `dx build --platform web --package rssr-app --locked --debug-symbols false` | 构建成功，0；仍打印既有 dx 0.7.10 / Dioxus 0.7.9 版本不匹配诊断，未升级依赖 |
| `cargo build -p rssr-app` | 通过，0 |
| `bash scripts/run_wasm_contract_harness.sh wasm_subscription_contract_harness` | 经已有 `run-harness.sh` 设置本机 ChromeDriver/wasm-bindgen 环境，7 passed，0 |
| `bun scripts/browser/rssr_nav_collapse_acceptance.cjs` | 基线和修改后均 10/10，0 |
| `bun scripts/browser/rssr_reading_position_acceptance.cjs` | 基线新断言失败，1；修改后首次列表链接等待超时，1；独立重跑两个视口通过，0 |
| `bun target/cohesion-review/native-check.cjs` | 规范初始状态并等待列表数据后，Linux WebKitGTK 通过，0；过程见下文 |
| `bun target/cohesion-review/multi-tab.cjs` | 负向验证复现另一标签收藏被覆盖，1（未修复） |
| `compare -metric AE <before.png> <after.png> null:` | 默认 / Atlas、两视口、展开/收起共 8 对截图，像素差均 0 |
| `git diff --check` | 通过，0 |

浏览器脚本使用已有 Playwright、Chromium 151，`NODE_PATH` 指向本机 Playwright 安装，`CHROME_BIN` 指向缓存 Chromium，`STATIC_BASE=http://127.0.0.1:8099`。导航图片分别输出到 `before/`、`after/`。Chrome MCP 未提供，按项目既有 Playwright 配置使用真实浏览器。

### 界面与故障记录

- Web：360×800、1280×800，默认样式与四套内置主题；搜索词保留、收起后仅 44×44px 按钮、焦点和 Enter 展开、跨路由保持、reload 默认展开、正文节点保留、无横向溢出、未捕获到未处理 JS 错误。
- Web 阅读：2,000 段正文，滚轮/PageDown 不扫描，搜索光标与修饰/composing 按键不扫描/切文；正常方向键切文、返回恢复、native capture 事件、顶部和底部位置均通过。composing 是合成事件，不代表实际输入法面板测试。
- Linux：1280×900 的真实 WebKitGTK 窗口，隔离复制测试 SQLite 数据，Inspector 触发 DOM 操作；编辑/修饰/composing 读取 0，搜索态收起后 44×44px 且仅一按钮，展开为 normal。截图 `native-verified.png`。使用软件渲染兼容开关，不能据此推定 GPU 帧率。
- 阅读脚本首个修改后运行在进入列表时等待文章链接 15 秒超时；没有调整该超时或删除断言，独立复测成功。原因未确认，不将失败记录计为通过。
- 原生临时脚本前两次分别遇到导航已收起、列表路由已进入但数据尚未加载的前置条件；均退出 1。显式展开并等待目标文章链接后通过，产品代码没有为适配脚本增加分支。
- 双标签诊断早期分别遇到重新加载等待超时、第二标签未设置本地 sessionStorage 认证而停在登录页；改用不重新 seed 数据的既有登录 helper 后才完成覆盖问题复现。早期超时不算覆盖问题的证据。
- ChromeDriver `/status` 的可选 curl 探测被工具审批拒绝，未产生进程退出码；改用驱动启动日志及实际 harness 成功连接确认可用。

## 结果、风险与后续事项

- **已验证**：两处 UI 修复、导航视觉等价、上述构建/测试结果，以及既有 Web 双标签覆盖问题。
- **源码支持**：保留的 Arc/线性查询/投影优化有局部明确边界；没有新增平台分支回流到 application/domain。
- **未验证**：Windows/macOS 实机、Android 模拟器/真机、实际输入法面板、硬件 GPU 帧率和所有文章规模。Android 本轮只有编译检查。
- **未运行**：refresh/config exchange wasm harness，本轮未修改相应实现；项目里 2 个默认 ignored 性能基准未额外执行。未重复任务 1 的外部浏览器打开专项，相关实现未改动。
- 优先后续工作是 Web 跨标签和跨片提交一致性；不能把单标签契约 harness 的通过当成多标签正确性。当前修复可独立合入，但全局存储风险仍开放。

## 给下一位 Agent 的备注

- 从 `reading_position.js` / `use_reader_shortcuts.rs` 的事件边界，以及 `shell_state.rs` 的互斥状态入手；不应重新引入滚动时正文扫描。
- 双标签最小复现需要两个标签各自解锁；第二次登录 helper 不能带 `seed`，否则会重置共享数据。
- 仅本地提交，未 push / tag / release。主工作树起点干净；保留 `.handoff/` 排除状态和 task3/4/5 旧 worktree，没有清理或提交其改动。

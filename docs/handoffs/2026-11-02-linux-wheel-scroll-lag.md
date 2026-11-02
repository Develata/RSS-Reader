# Linux 桌面滚轮失效与滚动卡顿

- 日期：2026-11-02
- 作者 / Agent：Claude Code (Opus 5.5)
- 分支：claude/linux-lag-scroll-bug-1935d4
- 当前 HEAD：f6c4a0cd（提交前）
- 相关 commit：0bc61f63
- 相关 tag / release：问题在 v0.2.1 Linux 发布版复现
- 状态：`validated`（单机验证）

## 工作摘要

v0.2.1 Linux 桌面版在“全部文章”页鼠标滚轮完全失效，并在用了 Midnight Ledger 主题、快速滚动时明显卡顿。三个独立原因分别修复：根元素 `overscroll-behavior` 吞滚轮、全页径向渐变在 CPU 渲染下过贵、WebKitGTK 平滑滚动动画跟不上高频滚轮。

## 影响范围

- 模块：
  - `assets/styles/entries.css`
  - `assets/themes/midnight-ledger.css`、`assets/themes/legacy/midnight-ledger-v3.css`、`crates/rssr-app/src/pages/settings_page/themes/theme_preset.rs`
  - `crates/rssr-app/src/bootstrap/native_document.rs`、`crates/rssr-app/src/bootstrap.rs`、`crates/rssr-app/src/app.rs`、`crates/rssr-app/Cargo.toml`（`App` 只调用平台无关的 `bootstrap::use_host_webview_settings()`，Linux 条件编译留在 bootstrap）
- 平台：
  - Linux 桌面（全部三项）；Web / Windows / macOS / Android 仅受 CSS 两项影响
- 额外影响：
  - 已选 Midnight Ledger 的用户需按[用户指南](../user-guide.md)重新点击该预设并保存，新光晕才生效。预设 CSS 文本存于用户设置；v0.2.1 存下的文本即 legacy v2（已核对与用户库一致），本次改动前的 main 文本冻结为 legacy v3，两者都仍识别为 Midnight Ledger

## 关键变更

### 滚轮失效

- 根因：`html:has([data-page="entries"][data-entry-scope="all"]) { overscroll-behavior-y: contain; }`（d0d98dab，为触屏下拉刷新加）。WebKitGTK 2.54 对设了此值的根滚动容器忽略鼠标滚轮。
- 证据：v0.2.1 副本中 52 次滚轮事件全部送达页面、未被 `preventDefault`，滚动 0 次；运行时覆盖为 `auto` 后 94 次滚轮产生 47 次滚动，用户确认恢复。
- 修复：规则移入 `@media (pointer: coarse)`。下拉刷新只消费 touch 事件，桌面无此需要；Android WebView 为 coarse，行为不变。

### 卡顿一：径向渐变（CPU 渲染）

- Dioxus 0.7.9 在 Wayland 默认设置 `GDK_BACKEND=x11` 与 `WEBKIT_DISABLE_DMABUF_RENDERER=1`；本机（NVIDIA 550）WebKit 进程不加载 NVIDIA 驱动、无合成层（`LayerTree` 为空），滚动每帧重绘整个视口。
- Midnight Ledger 的两层全页 `radial-gradient` 是主要开销：合成滚动基线 27–30 ms/帧；只去掉这两层即 16 ms/帧。
- 修复：两层光晕改为角落固定区域（`500px` / `460px`，`no-repeat`），外观基本保持，CPU 路径 16–18 ms/帧。未改 Dioxus 的渲染配置（开启 DMABUF 也能到 16 ms/帧，但会改变所有 Wayland 用户的渲染路径，且 Dioxus 关它是为躲 NVIDIA bug；按讨论不采用）。

### 卡顿二：高频滚轮下的平滑滚动

- 用户录屏与页内记录：快速拨轮时每 50 ms 进来 10–19 个滚轮事件（`deltaY` 89），页面只移动 2–5 px；无任何 JS 写滚动位置、无反向跳动。WebKitGTK 平滑滚动动画每个事件重新起步，事件过密时几乎不前进。
- 验证：运行时以即时滚动替代后，用户确认快速滚动跟手。
- 修复：Linux 下经 wry 取得 WebKitGTK `Settings`，`set_enable_smooth_scrolling(false)`（宿主层，未用 JS 劫持滚轮）。代价是慢速滚动按格跳动，用户确认可接受。

### CI：Web 回归里的日期定时失效（与本修复无关）

- PR #61 与 main `287256da`（仅文档）的 5 个 `web-ui` 在同一处失败：`checkEntriesInitialization` 让首次读取失败后，等不到 `entry-card-title`。main `f6c4a0cd` 在 10-31 用同一 Chrome（156.0.8078.80）全部通过。
- 原因：首次读取（含偏好）失败时，文章页停在 `UserSettings::default()` 的 3 个月归档窗口；`mobile-ui-short` 种子文章固定发布于 2026-08-01。11-01 起分界越过该日期，全部文章被归档。
- 修复：该检查用现有 `edit` 钩子把种子文章发布时间改为前一天，不再依赖当前日期；产品的失败回退行为不变。
- 本机验证（release 构建，本机 Google Chrome，非 CI 的 Chrome for Testing）：`scripts/run_server_web_regression.sh` 默认 regression、`theme-matrix`、`feed-smoke`、`acceptance` 与 newsprint / atlas-sidebar / midnight-ledger / amethyst-glass 四个预设全部通过。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`：通过
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：通过
- `cargo test --workspace --locked`：通过（1192 passed，0 failed）
- `cargo check -p rssr-app --target wasm32-unknown-unknown --locked`：通过
- `cargo check -p rssr-app --target aarch64-linux-android --locked`：未完成（本机无 Android NDK，`ring` 构建脚本失败）；本次 Rust 改动只在 `target_os = "linux"` 下编译，Android 侧由 CI 覆盖

### 手工验收

- 环境：Debian 13，GNOME Wayland，NVIDIA RTX 3060 Mobile（驱动 550.163），WebKitGTK 2.54.0，2560×1440@120 Hz 外接屏；资料库为用户库的副本
- v0.2.1 发布版复现滚轮失效：通过（复现）
- 修复版 debug 构建（Dioxus 默认渲染配置）全部文章页快速 / 慢速滚轮：通过（用户亲测）
- 帧时间以 WebKit 远程检查器（`WEBKIT_INSPECTOR_HTTP_SERVER`）测量 rAF 间隔；与用户体感一致，但只代表本机

## 结果

- 可合并；用户需重新选择 Midnight Ledger 预设才得到新光晕
- 其他平台的 CSS 变化：桌面浏览器不再对“全部文章”页根元素设 `overscroll-behavior`；Midnight Ledger 光晕尺寸固定，不再随页面高度缩放

## 风险与后续事项

- Atlas Sidebar、Amethyst Glass 也有全页径向渐变（Amethyst 还有 `backdrop-filter`），CPU 渲染下同样偏贵，本次未改、未测；Develata 决定留待原生桌面版处理
- 关闭平滑滚动只影响 Linux WebKitGTK；若以后需要平滑感，可考虑宿主层按滚轮频率动态切换该设置（未验证）
- 内置预设升级不会自动替换用户已存的旧文本，属于既有机制
- 计划中的原生 Rust 桌面版会绕开这些 WebView 问题，届时 `use_direct_wheel_scrolling` 随 WebView 删除

## 给下一位 Agent 的备注

- 入口：`crates/rssr-app/src/bootstrap/native_document.rs`、`assets/styles/entries.css` 中 `pointer: coarse` 一段
- Dioxus Linux 渲染环境变量见 `dioxus-desktop-0.7.9/src/app.rs` 的 `disable_dma_buf`；`/proc/<pid>/environ` 看不到它在进程内设置的值

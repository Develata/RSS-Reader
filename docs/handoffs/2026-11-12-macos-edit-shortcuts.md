# macOS 桌面端恢复系统编辑快捷键

- 日期：2026-11-12
- 作者 / Agent：Claude Code (Opus 5.5)
- 分支：claude/macos-shortcut-compat-e1c799
- 当前 HEAD：20521b8f（改动前）
- 相关 commit：见本分支提交 `fix: restore macOS edit shortcuts via the menu bar`
- 相关 tag / release：N/A
- 状态：`draft`（自动化通过；macOS 实机按键未验收）

## 工作摘要

macOS 桌面端里 Cmd+C / V / X / Z / A 和 Cmd+Q / W / M / H 都不起作用，Windows 上对应的 Ctrl 键正常。原因是 `main.rs` 自 1a7b0f69 起一直用 `.with_menu(None)` 去掉了菜单栏：macOS 的这些键只作为主菜单的 key equivalent 分发，WKWebView 没有自己的兜底；Windows 的 WebView2 和 Linux 的 WebKitGTK 则由页面自己处理 Ctrl 编辑键。

## 影响范围

- 模块：
  - `crates/rssr-app/src/main.rs`（桌面入口）
  - `docs/testing/manual-regression.md` §11
- 平台：
  - macOS：新增系统菜单栏
  - Windows / Linux：行为不变，仍无菜单栏
  - Web / Android：不涉及（代码在桌面入口的 cfg 之内）
- 额外影响：N/A

## 关键变更

### 桌面入口

- `desktop_menu()`：在 macOS 上构建应用菜单（关于 / 服务 / 隐藏 / 隐藏其他 / 全部显示 / 退出）、编辑菜单（撤销 / 重做 / 剪切 / 拷贝 / 粘贴 / 全选）、窗口菜单（最小化 / 缩放 / 全屏 / 关闭窗口）。全部使用 muda 的 `PredefinedMenuItem`（AppKit 标准 selector），应用不新增任何自定义快捷键处理。muda 走 `dioxus::desktop::muda` 的再导出，不新增依赖。
- 其它桌面平台的 `desktop_menu()` 返回 `None`，与原行为一致。
- 菜单构建失败时只记一条 warn 日志，然后退回无菜单，不阻止启动。
- 不复用 Dioxus 的 `default_menu_bar()`。它把 Window 放在第一位，会被 AppKit 当成应用菜单；debug 构建还会加一个 Help 菜单。

### Cmd+Q 退出路径

- 退出走 `NSApp terminate:` → tao 的 `applicationWillTerminate` → `AppState::exit()` → `Event::LoopDestroyed` → `bootstrap::close_local_library()`。这条路径来自对 tao 0.34.8 源码的阅读（`platform_impl/macos/app_delegate.rs:131`、`app_state.rs:272`），没有实测。Dock 菜单里的“退出”原本就走同一条路径。

## 验证与验收

### 自动化验证（macOS arm64 本机）

- `cargo fmt --all --check`：通过
- `cargo clippy -p rssr-app --all-targets --locked -- -D warnings`：通过
- `cargo test -p rssr-app --locked`：通过（409 + 37 + 18 + 7 + 5 + 1，1 ignored）
- 启动冒烟：在隔离的 `HOME` 下运行 `target/debug/rssr-app` 12 秒。资料库正常打开，没有 panic，也没有菜单 warn。进程最后被 SIGALRM 结束，所以留下的 `-wal` 不能作为 Cmd+Q 的证据。
- 未执行：wasm32 / Android / Windows / Linux target 检查。本机只装了 `aarch64-apple-darwin`。改动全部在桌面 `main` 的 cfg 之内，非 macOS 分支只是返回 `None`，交给 CI 覆盖。

### 手工验收

- macOS 实机按键（第 11 节新增清单）：未执行。computer use 缺少辅助功能权限。这类按键只经 AppKit 菜单分发，Chromium 下的 Web / wasm 回归无法复现，也无法验证。

## 结果

- 可合并的前提：Develata 在 macOS 上按 `manual-regression.md` §11 的清单实机过一遍。

## 风险与后续事项

- 菜单文字固定为中文，与界面语言一致。以后如果界面做多语言，菜单文字要跟着改。
- 没有自动化测试防止 `with_menu(None)` 回归：muda 在 macOS 上只能在主线程构建菜单，而默认测试线程不是主线程；只断言一张自己写的菜单清单又是同义反复。现在靠代码注释和手工清单兜底。

## 给下一位 Agent 的备注

- 入口：`crates/rssr-app/src/main.rs` 的 `desktop_menu()`。
- `with_menu` 必须放在 `with_window` 之后，因为 Dioxus 只给有窗口装饰的窗口挂菜单。

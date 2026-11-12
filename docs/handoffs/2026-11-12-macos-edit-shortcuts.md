# macOS 桌面端恢复系统编辑快捷键

- 日期：2026-11-12
- 作者 / Agent：Claude Code (Opus 5.5)
- 分支：claude/macos-shortcut-compat-e1c799
- 当前 HEAD：20521b8f（改动前）
- 相关 commit：341bb388
- 相关 tag / release：N/A
- 状态：`validated`（macOS 虚拟机里修复前复现、修复后通过）

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

- 退出走 `NSApp terminate:` → tao 的 `applicationWillTerminate` → `AppState::exit()` → `Event::LoopDestroyed` → `bootstrap::close_local_library()`。这条路径最初是从 tao 0.34.8 源码读出来的（`platform_impl/macos/app_delegate.rs:131`、`app_state.rs:272`），后来在虚拟机里实测：Cmd+Q 退出后数据目录没有残留 `-wal`。Dock 菜单里的“退出”原本就走同一条路径。

## 验证与验收

### 自动化验证（macOS arm64 本机）

- `cargo fmt --all --check`：通过
- `cargo clippy -p rssr-app --all-targets --locked -- -D warnings`：通过
- `cargo test -p rssr-app --locked`：通过（409 + 37 + 18 + 7 + 5 + 1，1 ignored）
- 启动冒烟：在隔离的 `HOME` 下运行 `target/debug/rssr-app` 12 秒。资料库正常打开，没有 panic，也没有菜单 warn。进程最后被 SIGALRM 结束，所以留下的 `-wal` 不能作为 Cmd+Q 的证据。
- 未执行：wasm32 / Android / Windows / Linux target 检查。本机只装了 `aarch64-apple-darwin`。改动全部在桌面 `main` 的 cfg 之内，非 macOS 分支只是返回 `None`，交给 CI 覆盖。

### macOS 虚拟机按键验收

- 环境：Tart 虚拟机 `mac-test`（macOS 27.2 26B123，arm64，vanilla 镜像，自动登录）。SSH 主连接由 Terminal.app 打开，测试经它的套接字执行。虚拟机里 `sshd-keygen-wrapper` 已有辅助功能权限。
- 被测对象：修复前（20521b8f 的 `main.rs`）与修复后（341bb388）的 debug 二进制，SHA-256 前 16 位分别为 `1ad190bc86088456` 和 `756294ebde6cfd2c`。各自包成临时 `.app`，用独立的 `HOME` 启动。
- 驱动：一个临时 Swift 小工具，用 AX 读菜单栏、点“订阅”、聚焦订阅地址输入框并读取它的值，用 `CGEventPost` 发送按键；剪贴板用 `pbcopy` / `pbpaste` 读写。脚本没有进仓库。
- 对照：两版都先输入一个普通字母 `a` 再按 Delete，输入框的值依次为 `a` 和空。这说明合成按键确实送达了输入框，Cmd 组合失效只能是缺少菜单所致。

| 检查 | 修复前 | 修复后 |
| --- | --- | --- |
| 菜单栏 | 只有 Apple 菜单和空的应用菜单 | 应用 / 编辑 / 窗口菜单，快捷键为 H、Q、Z、X、C、V、A、M、F、W |
| Cmd+V 两次 | FAIL（空） | PASS（`abcabc`） |
| Cmd+A、Cmd+X（输入框与剪贴板） | FAIL（剪贴板仍为 `abc`） | PASS |
| Cmd+Z / Shift+Cmd+Z | FAIL | PASS |
| Cmd+C（剪贴板先放入哨兵值） | FAIL（仍为哨兵值） | PASS |
| Cmd+M 最小化 / Cmd+H 隐藏 | FAIL | PASS |
| Cmd+Q 退出，数据目录无 `-wal` | FAIL（进程不退出） | PASS |
| Cmd+W 关窗退出，数据目录无 `-wal` | FAIL | PASS |

修复前一列里，“Cmd+A、Cmd+X 后输入框为空”和“Shift+Cmd+Z 后为空”两项也显示 PASS，但那只是因为输入框本来就是空的，没有意义，表里不计入。

### 未覆盖

- 正文阅读区的 Cmd+C / Cmd+A，以及阅读快捷键 m / f / j / k 不受影响：虚拟机里的资料库是空的，没有文章。阅读快捷键对带修饰键的事件直接放行（`reader_shortcuts/policy.rs`），菜单只绑定 Cmd 组合，两者不冲突，但这一点是推断。
- 正式发布包（release、ad-hoc 签名）没有在虚拟机里跑，测的是 debug 二进制。

## 结果

- 可合并。macOS 上的编辑、窗口和退出快捷键已在虚拟机里复现并验证修复。

## 风险与后续事项

- 菜单文字固定为中文，与界面语言一致。以后如果界面做多语言，菜单文字要跟着改。
- 没有自动化测试防止 `with_menu(None)` 回归：muda 在 macOS 上只能在主线程构建菜单，而默认测试线程不是主线程；只断言一张自己写的菜单清单又是同义反复。现在靠代码注释和手工清单兜底。

## 给下一位 Agent 的备注

- 入口：`crates/rssr-app/src/main.rs` 的 `desktop_menu()`。
- `with_menu` 必须放在 `with_window` 之后，因为 Dioxus 只给有窗口装饰的窗口挂菜单。

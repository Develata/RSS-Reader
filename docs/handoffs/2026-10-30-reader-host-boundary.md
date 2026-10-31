# Reader 宿主入口与生命周期

- 日期：2026-10-30
- 作者 / Agent：Codex
- 分支：`codex/reader-host-boundary`
- 基线 HEAD：`ff628ed03a96e57392b94ad3655cc4cb62be736a`（PR #50）
- 产品 commit：`9e0eb45baed04355af3ffabe478b62940ad23d5b`
- 文档/证据 commit：本记录所在提交，可用 `git log -- docs/handoffs/2026-10-30-reader-host-boundary.md` 查阅
- PR：[Reader 宿主边界 #51](https://github.com/Develata/RSS-Reader/pull/51)，base 为 `codex/reader-rule-boundary`
- 相关 tag / release：无
- 状态：`validated`（本地实现与所列验收；提交此文档时远端 CI 进行中，最终 head 结果见 PR Checks）

## 工作摘要

承接 [Reader 规则独立性](./2026-10-30-reader-rule-boundary.md)，将页面中的图片激活、图片 dialog 和快捷键桥接集中到所属 WebView 适配模块。目标是给未来界面替换留下明确入口，保持规则、视觉和任务生命周期；没有确认新的运行缺陷，也没有混入产品行为修复。长期边界见[原生界面前置说明](../design/native-ui-preparation.md#reader-宿主入口2026-10-30)。

复用 `E:\gitclone\RSS-Reader-worktrees\native-back-policy`，从 `ff628ed0` 创建本地分支。main / origin main 仍为 `4cef074c`，PR #50 open、未合并；新 PR 以 `codex/reader-rule-boundary` 为 base，仅审查本批增量。未向已合并 PR 追加实现，也未发布或改动其他工作树。

## 影响范围

- 模块：`reader_page/webview/`、`image_viewer.rs`、`use_reader_shortcuts.rs`；共享阅读位置仅更新一条路径注释。
- 平台：Web 与原生 Dioxus/WebView；实际验收为 Windows、Android 15 真机、正式同源 Web。
- 额外影响：共享 Reader 宿主场景、正式 Web 运行器、已有原生运行器开关、`reading-workspace` CI 和契约/维护文档。
- 七 crate、Backend、数据库、业务规则、状态来源、CSS、HTML、宿主 JS 内容及阅读位置适配器保持现状。没有新依赖、crate、全局 Host trait 或页面框架。

## 关键变更

- `webview/mod.rs` 提供图片激活、dialog 挂载/关闭和原始按键入口。组件/状态 hook 不再管理 `Eval` 或停止消息；三个脚本按原内容移动到该目录。
- 私有 `use_listener_cleanup` 合并原有 `use_drop` 发送停止消息的重复代码。图片与键盘继续使用原页面 `use_future`；dialog 仍在组件挂载后 `spawn` 等待关闭。没有新增取消、重试或代次语义。
- `ImageViewerState` 仍只有一份；Rust 快捷键解析与 session/navigation 派发保持原处。切文前捕获仍调用现有阅读位置入口，不再包装服务。
- `reader_host_scenarios.cjs` 观察真实监听的 AbortSignal 和 MutationObserver；三次进入 Reader，分别通过按钮、Esc/系统返回、带浮层路由离开，检查资源清理、回焦、body 样式和保存位置恢复。原生运行器增加 `--reader-host true`，只允许与 `--reader-only true` 同用。
- 正式 Web 使用真实同源服务器及新 SQLite 夹具，读取本机图片；新脚本进入既有 `reading-workspace` suite，不新增 CI job。

## 验证与验收

所有原始资料保存在本工作树的 `target/reader-host-boundary-20261030/`。仓库保留[紧凑回执](../testing/evidence/2026-10-30-reader-host-boundary/receipt.json)；下列结果绑定本批产品输入，不沿用 PR #50 的测试结论。

### 自动化验证

| 命令 | 结果 |
| --- | --- |
| `cargo test -p rssr-app --locked` | 418 通过、1 忽略：390 unit + 23 生产规则 harness + 2 主题 + 3 样式；日志 `app-tests.log` |
| `python scripts/check_reader_rule_boundary.py --target-dir target/reader-host-boundary-20261030/rules-build` | 同一 23 项生产规则独立编译通过，不链接 Dioxus/web-sys/Wry/Tao；不是另增 23 项覆盖 |
| `cargo fmt --all --check` | 通过 |
| `cargo clippy -p rssr-app --all-targets --locked -- -D warnings` | 通过 |
| `cargo build -p rssr-app -p rssr-web --locked` | 通过；另构建 `rssr-infra` 的 `browser_fixture_library` |
| `cargo check -p rssr-app --target wasm32-unknown-unknown --locked` | 通过 |
| dx 0.7.9 `build --platform web --package rssr-app --locked --release --debug-symbols false` | 正式 Web 构建通过，无 ERROR |
| dx 0.7.9 `build --platform android --package rssr-app --target aarch64-linux-android --locked` | 独立源码/target 构建通过，无 ERROR；既有 `LOCAL_DATA_DIR_NAME` 未用警告保留 |
| `node --check` 检查本批三个 CJS；`git diff --check`；本地文档链接/锚点/JSON 检查 | 通过；330 份 Markdown、1553 个相对链接检查，本批无新增失效链接或锚点 |

### 实际宿主与正式 Web

| 平台 / 版本 | 基线 / 候选 | 场景与回执目录 |
| --- | --- | --- |
| Windows WebView2 `156.0.4314.50` | 各 6/6 | 1280 / 390 宽度，图片关闭/焦点/位置、M/F 保留正文、输入排除、J/方向键切文与返回，加三种宿主生命周期；`windows-baseline-03` / `windows-candidate-02` |
| Android 15 真机 `caa0c283`，WebView `156.0.8078.47` | 各 3/3 | 实际 392 宽度，同上；图片关闭使用真实 `adb keyevent 4`；`android-host-baseline-02` / `android-host-candidate-01` |
| Windows Chrome `155.0.8059.40`，正式 Web | 各 2/2 | 1280 / 390 两视口，各三种关闭/离页模式；`web-baseline-02` / `web-candidate-02` |

全部最终报告 `pageErrors` / `consoleErrors` 为空。Reader 运行时键盘 1、图片输入 2、observer 1；dialog 打开时额外 2 个监听；每次离页后 Reader 资源均为 0。探针区分离页后新建的 Entries 监听，不把其他页面的资源算作 Reader 泄漏。

Windows 使用 `run-windows.ps1 -Variant baseline|candidate -RunName <上述目录>`；Android 使用 `run-android.ps1` 的同名参数，均调用已有身份校验运行器及 `--reader-only true --reader-host true`。正式 Web 使用 `RSSR_WEB_BIN` / `RSSR_FIXTURE_BIN` / `PUBLIC_DIR` / `ARTIFACT_DIR` 指向各自隔离产物，运行 `node scripts/browser/rssr_reader_host_acceptance.cjs`。具体启动、构建和保全脚本与日志留在上述 target 目录；[稳定复跑入口](../testing/native-control-acceptance.md)描述夹具与进程约束。

17 对截图有 16 对逐字节相同。Windows 的 `1280-reader-after-shortcuts.png` 有 47 个不同像素，范围 `[160,26,180,36)`，仅在返回箭头内部；已目视查看两图，其他区域完全相同。保留两份原图、SHA256 与 `visual-difference.json`，不声称全数截图相同，也不据此推断已确认绘制差异的根因。

这是脚本驱动的真实宿主验收，未进行物理键盘、系统 IME 或辅助技术手工验收。浮层期间离页为受控路由调用，不是点击穿透 inert；监听探针保留原应用回调，但并非完全无干预的用户场景。

### 产物与保全

- Windows 基线 EXE 复用前批产品产物，SHA256 `b13817f72ffc6f0a1242adae8f64c8a0beae226dd55240091396a817b53a3fd7`；候选为 `5e3c5e46bd9f45290049890debcbb76e7b605d8d0b43c76d8245d194cc3e96d3`。
- Android 只安装 `io.github.develata.rssreader.backpolicytest`。候选 APK `reader-host-final.apk` SHA256 `54ca6d4644020b3c2a830bb90aef5d0a840cc073bf736dd1830ef13fc3e7fbd6`；APK 内 `libmain.so`、生成工程 JNI 与本轮编译输出 SHA256 均为 `64ecc2c6ed543fccdabcd503fa58bd02538a8fc017a0dd3d992358d3eb5ed6b5`。核对生成 MainActivity、依赖源码路径与 461 项产品输入，仅测试 identifier / BuildConfig typealias 有隔离覆盖。
- `product-files.json` 实际文件 SHA256 为 `fbc3b32b0beca8ddce2e30a794f28e55fbe86c7ac687aeec98f2f5280e1e8df0`。build ID `reader-host-ff628ed0-e65954551bac` 是本次标识，其后缀不作为清单文件哈希；清单摘要以实际文件字节为准。
- Web 基线的全部 17 个 public 文件对照前批清单逐项核验；旧 index 按清单恢复后 SHA256 为 `c56207f4a17be9ba6dd6dd656a3cae66ef5c3298be0ff1527ca91d1e987aa271`。服务器代码本批未改，基线服务端是指定旧 build ID 的重新构建，不是复用原服务器 EXE。候选服务端及 public 分别留存，不混用 build guard 身份。
- 构建使用任务内 dx 0.7.9、进程级 JDK 21 和 NDK `28.2.13676358`，未改全局 dx/JDK。Android 目标缓存仅复制到新任务目录，源码与最终 APK 重新核验。
- 主 checkout `020f9d4031febc709cf1fdca2e7360698ba19f26` 的分支、HEAD、index、status 和 41 个文件/缺失路径均与开始快照一致。Android 安装前已备份并校验旧测试 APK、files；每次场景后恢复并核验原 6 个资料文件。日常包 SHA256、原有 adb forward/reverse 均保持。测试产生的资料和失败证据保留，无旧目录清理。

### 保留的失败与采样修订

1. `windows-baseline-01` 的新探针把离页后 Entries 新建的按键监听计入 Reader；修正为记录注册时的页面归属，原报告保留。
2. `windows-candidate-01` 的关闭位置断言使用按键前采样，而原 dialog 规则保存挂载时的位置。补充实际 keydown、body.top 保存值与关闭后位置；用实际滚轮输入结束初始位置恢复，再进行图片定位。仍以 3px 容差比较 dialog 保存值与恢复值，所有最终基线/候选通过。
3. 首次失败缺少 dialog 保存值，不能据此确认产品滚动缺陷或把采样修改算作产品修复。三个产品脚本逐字节未改，首轮报告及后续采样事实均保留。

## 结果

本地功能与宿主回归通过，代码自审未发现阻塞问题。当前适配层仍依赖 Dioxus/WebView；本批证明入口收敛与所测行为保持，没有证明整页可直接用原生控件替换，也没有性能结论。产品已提交并推送，PR #51 保持 open；7 个本地分支、2 个工作树、6 个真实远端分支。

CI 以 PR 最终 head 为准，不把前一提交或 PR #50 的成功结果沿用到本批。精确 head 的远端回执在完成后留于本地证据目录并更新 PR 描述，避免仅为记录 CI 再产生新的未验 head。Pages workflow 仅接受 main 目标 PR，本依赖型 PR 不触发它，也不发布。

## 风险与后续事项

- 下一小批处理 Reader 快捷键定义去重，保持同步默认动作、输入过滤、布局与切文前位置捕获；不顺带创建全应用键盘框架。
- 随后明确正文内容、图片资源、阅读锚点与 HTML/DOM 的兼容边界，实测候选渲染器另立范围。
- 未覆盖 macOS/Linux GUI、系统 IME、物理键盘、辅助技术或正式签名包升级。Android 实际系统返回与构建检查已分别记录。
- PR #50 仍为前置依赖；没有自动合并授权，不在本批改 main。

## 给下一位 Agent 的备注

从 `reader_page/webview/mod.rs`、`image_viewer.rs` 和 `hooks/use_reader_shortcuts.rs` 阅读本批连接点；纯规则从前批独立测试入口查阅。复跑必须使用新的 target 运行目录、隔离资料库及明确的产物身份。

本机 `target/debug/rssr-web.exe` 最后用于旧 build ID 的基线重建；候选服务器留在 `target/reader-host-boundary-20261030/web-candidate-bin/rssr-web.exe`，候选 public 仍为正常 dx 输出。不要直接把前者与候选 public 配对。Android 测试包当前装候选，原测试资料已恢复，日常包未动。

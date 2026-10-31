# Settings 草稿、逐字段保存与异步回填边界

- 日期：2026-10-31
- 作者 / Agent：Codex
- 分支：`codex/settings-rule-boundary`
- 实现 HEAD：`ec4a605b`；本批基线 `8dcf710c21ebfd191c3662790c9354184f436138`（开始时与 `origin/main` 一致，已含 PR #59）
- 相关 commit：`ec4a605b`（实现、测试与契约）；后续仅补本记录的提交回执，不改产品源码
- 相关 tag / release：N/A
- 状态：`validated`（原批本地验收；后续已通过 PR #60 合并，见[复审与整合记录](./2026-10-31-settings-review-merge.md)）

## 工作摘要

继续让未来 Rust 原生界面复用现有交互规则。本批核对 Settings 的编辑、保存和回填，去除一条经过 Dioxus 组件模块的依赖，并分离草稿投影决定与界面发布。没有确认新的产品运行缺陷；独立编译失败是复用阻碍的证据，不记作用户故障修复。

复用 E 盘 `E:/gitclone/RSS-Reader-worktrees/native-back-policy`，在干净的最新 main 上建立本任务分支。没有创建工作树；主 checkout 的其他任务继续保留。

## 影响范围

- 模块：`crates/rssr-app/src/pages/settings_page/` 的私有规则和 session 装配；`tests/settings_rules.rs` 独立入口。
- 平台：共享页面逻辑；本轮实际宿主覆盖 Windows，Web 和 Android 另作编译检查。
- 额外影响：已有 `rssr-api` CI job 增加无框架编译步骤，更新设计契约、当前状态与交接索引。无新 crate、依赖、配置格式、数据库、Shell 或渲染器迁移。

## 关键变更

### 规则与适配职责

- `draft_edits.rs` 和原有门禁保持不变。`draft_projection.rs` 只接收普通事实，返回 stored → submitted → pending 草稿与数值提示代次、CSS 导入失效、预设和预览的发布决定。它不持有另一份草稿，不执行副作用。
- 未编辑的数值字段即使回填相同值也更新提示代次；编辑字段保留解析提示。CSS 只有实际变化才使旧文件读取失效；读取/下载与保存成功分别保留原来的预设选择、预览规则，包括保存时的预设值相等判定。
- `LoadedSettings::resolve` 在实际应用时比较 App 设置版本，包含写入结束才释放的暂存读取。版本变化返回 `ShowCurrent`，由 session 读取现有 ThemeController，不能再次应用旧读取结果。
- `save_policy.rs` 直接引用 domain 的同一 CSS 校验函数，去除对含 Dioxus 组件的 `themes` 聚合模块的依赖。保存仍只发送编辑字段、仅规范化提交副本；失败恢复 submitted 于后来 pending 下方，结果未知继续保留原提示语义。
- session 收集事实，按原顺序发布数值代次、导入身份、预设/预览、草稿和已存设置，再更新主题。保存/下载的准入、任务作用域、Shell 草稿恢复、文件选择和预设识别仍归当前适配；不声称整页或 runtime 已独立于 Dioxus。

### 独立性证明与 review

- 新检查脚本仅构建真实 `rssr-api`/application/domain 依赖图，用 rustc 编译同一生产源码，不复制模型或 CSS 校验器、不引入 Cargo package。构建依赖不得含 Dioxus、web-sys、Wry、Tao。
- 基线探针以 `E0432 unresolved import super::themes` 失败；修正后 18 项通过。包含原有模块测试及保存期间后续编辑、规范化、成功丢弃旧读取、失败/结果未知恢复、暂存读取与全局版本变化的组合输入序列。
- 保留现有 Dioxus 装配测试，逐段核对保存成功与失败、读取/下载、CSS 预览、字段提示和发布顺序。review 未发现本批新增的行为差异或待修问题；现有 `save/session.rs`、`sync/session.rs`、JS/CSS 与宿主脚本未改。

## 验证与验收

证据根目录：`target/settings-rule-boundary-20261031/`。`verification.json` 汇总身份和回执；`source-files.json`、`source.patch` 记录本批源码，`SHA256SUMS` 索引日志及验收文件。

### 自动化验证

| 命令 / 场景 | 实际结果 | 证据 |
| --- | --- | --- |
| 基线 `cargo test -p rssr-app --locked settings_page` | 74/74 通过 | `baseline-settings-tests.log` |
| 基线无框架编译探针 | 预期失败；确认 `save_policy → themes` 依赖阻碍独立编译 | `baseline-rule-probe.log`、`baseline-rule-probe-exit.txt` |
| `python scripts/check_settings_rule_boundary.py` | 18/18 通过；真实依赖图不含 UI 框架 | `rule-probe.log`、`settings-rule-boundary.json` |
| `cargo test -p rssr-app --locked` | 467 项通过、1 项既有手工性能测量忽略；含 78 项匹配 `settings_page` 的测试（76 项页面、2 项 Shell）和 18 项独立入口测试 | `app-tests.log` |
| `cargo clippy -p rssr-app --all-targets --locked -- -D warnings` | 通过，无警告 | `clippy.log` |
| `cargo build -p rssr-app --locked` | Windows debug EXE 构建通过；该 EXE 用于下述实际宿主验收 | `native-build.log` |
| `cargo check -p rssr-app --target wasm32-unknown-unknown --locked` | 通过；本轮未运行正式 Web 浏览器验收 | `wasm-check.log` |
| `cargo check -p rssr-app --target aarch64-linux-android --locked` | NDK 28.2.13676358 / API 24 编译通过；保留既有 infra `LOCAL_DATA_DIR_NAME` 未使用警告 | `android-check.log` |
| `actionlint .github/workflows/ci.yml` | 通过；新增步骤尚未在远端运行 | `actionlint.log` |
| `cargo fmt --all --check`、`git diff --check`、文档链接检查 | 通过；没有新增失效链接或锚点 | `format.log`、`diff-check.log`、`docs-check.log` |

### 实际宿主自动化验收

环境为 Windows / WebView2 `156.0.4314.50`；使用当轮新 EXE、独立 WebView profile、复制的合成 SQLite 夹具（100 篇文章、6 个 localhost 来源），核对 CDP 端口与进程归属。未打开日常资料库。

- `node scripts/browser/rssr_theme_selector_windows.mjs --app-bin target/debug/rssr-app.exe --seed-dir target/back-policy-native-20261030/seed --artifact-dir target/settings-rule-boundary-20261031/windows-theme --port 9495`：5/5 通过。原生宿主中的主题弹窗取消、受信任 CDP 键盘选择只预览、回填保留 pending、不伪造 input、显式保存和重开；兼查密度退役。证据 `windows-theme/acceptance.json`、UIA 树及截图。
- `node scripts/browser/rssr_shell_settings_acceptance.mjs --app-bin target/debug/rssr-app.exe --seed-dir target/back-policy-native-20261030/seed --artifact-dir target/settings-rule-boundary-20261031/windows-lifecycle --port 9495`：3/3 通过。启动存储外观、外部设置只读重载、Settings 重读保留编辑和离页恢复存储外观。证据 `windows-lifecycle/acceptance.json` 及截图。
- 两次验收产物 SHA-256 均为 `0061bd10cb8a13f4f01953578533a3f9a6174e441da4ce567a57c5fa0fbbdcd2`；页面异常和控制台错误均为空。
- 外部 SQLite 修改和 DOM focus/visibility/online 是受控输入，不声称覆盖 OS 前后台、物理键盘、系统 IME 或辅助技术。本轮没有 Android 真机/模拟器、Linux/macOS GUI 验收，也没有人工手点验收。

## 结果

- 原批本地实现、review 与相称验证完成，当时尚未推送；后续正式 Web 补验与 PR #60 合并单列于[复审与整合记录](./2026-10-31-settings-review-merge.md)，不把后续结果改写为原批验收。
- 主 checkout `E:/gitclone/RSS-Reader` 的 `feat/reading-workspace-20261024` / `020f9d40`、index、状态及 41 个已有改动/缺失路径已在提交前后逐项核对保持不变，回执为 `primary-before.json`、`primary-preservation.json`。
- 0.2.x 的 `list_density` 兼容参数未动；已批准的 0.3.x 破坏性移除继续单列。

## 风险与后续事项

- 普通 Rust 测试证明本批规则可独立调用；ThemeController、页面 Signal、文件 I/O、任务取消和路由仍属于当前界面装配，不能从本批推导整页原生替换完成。
- 后续先整合本批，再检查主题导入/预览的功能内宿主入口是否存在真实复用阻碍；复用当前文件请求身份和取消规则。没有实际第二种实现前不增加通用 Host trait 或页面框架。
- 远端 CI、正式 Web 浏览器以及 Android 实际宿主未作为本批本地通过项；若后续扩展宿主适配或发布，需按最终源码补充相应验收。

## 给下一位 Agent 的备注

- 从 `draft_projection.rs`、`request_lifecycle.rs`、`session.rs` 的依赖方向和 `tests/settings_rules.rs` 开始；长期约束见[原生前置边界](../design/native-ui-preparation.md#settings-草稿与回填边界2026-10-31)与 [Settings 交互契约](../design/frontend-command-reference.md)。
- 继续复用现有 E 盘工作树，读取当前 HEAD/dirty/main。历史证据及主 checkout 归属不因本地提交改变，不清理另一任务的改动。

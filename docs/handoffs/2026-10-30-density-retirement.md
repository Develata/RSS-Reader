# 列表密度退役与旧 Agent skill 清理

- 日期：2026-10-30
- 作者 / Agent：Codex
- 分支：复用 `codex/shell-settings-lifecycle`
- 工作树：`E:/gitclone/RSS-Reader-worktrees/native-back-policy`
- 基线 HEAD：`200ebfe907c9dc1ccb8e2196478e3b7dd19fd233`；main / origin/main 为 `663ef4ee5d19458c815a6921f0e418321cf75464`
- 相关 commit：pending
- 相关 tag / release：N/A；不升版本、不发布
- 状态：`validated`；实现、review、普通 Rust / 正式 Web / Windows 与 Android 实际宿主专项完成

## 工作摘要

用户批准移除列表密度选择，并指定 0.3.x 破坏性删除设置参数；另明确要求清理已提交的 `.agents/`。本批是独立的功能退役，不属于此前“行为等价”的返回策略提取。列表密度原先横跨设置、主题投影、应用根状态和四份 CSS，并非值得继续抽象的独立模块。

现在移除界面入口和紧凑呈现，统一原默认“舒适”间距。0.2.x 保留旧值的存储、API、CLI 与配置包兼容；0.3.x 删除类型和参数的完成标准已归入[维护待办](../maintenance-backlog.md#03x-移除旧列表密度参数)，避免另增一份设计计划。没有新增 crate、渲染器、业务状态镜像或数据库迁移。

## 影响范围与关键变更

- `preferences.rs` 删除下拉框及解析；`ThemeAppearance` 和 App 根节点不再投影设置中的密度。四份样式表删除 10 个 compact 分支，保留原默认样式。
- 根节点暂留固定 `data-density="comfortable"`，仅为 0.2.x CSS 兼容。旧 compact 配置不再改变布局；依赖 compact 属性的自定义 CSS 分支不再匹配。
- `draft_edits` 消费旧草稿时只丢弃密度项，其他编辑沿用 `stored → submitted → pending`。密度不成为新 UI 编辑或 UI 保存补丁。旧库值仍可完整读取、导出及由旧 API/CLI 写回，保存其他字段不会改写它。
- 旧密度专用测试变量改为仍有效的启动视图字段，保留保存中编辑、失败重试、异步读取、跨登录/重新加载草稿恢复的原断言。
- CLI `save-settings --list-density` 暂留，帮助标明无界面效果、0.3.x 删除。主题选择、启动视图、阅读字号与显式保存规则保持。
- 同步用户指南、主题选择器、前端命令、CSS 基线和原生适配契约；历史交接与格式 v2 兼容夹具不回写。

## `.agents/` 清理与工作区保全

按本轮明确授权删除 `.agents/skills/speckit-{analyze,checklist,clarify,constitution,implement,plan,specify,tasks,taskstoissues}/SKILL.md` 共 9 个跟踪文件。核对目录在当前工作树内、无其他未跟踪内容，运行时代码与当前文档无有效引用；`.specify/` 不属这次删除范围。

删除前 `git archive HEAD -- .agents` 保存到 `target/density-retirement-20261030/agents-before.tar`，SHA256 为 `9d0fd7957dc6b2d06a0c0d9dec0aafca8df0e1c8133d19ab46c11e4cffa00976`。逐份核对 9 个归档文件与基线 Git blob 一致（归档按属性使用 CRLF，比较时仅规范化换行）。历史验收资料和原先分支清理备份保留。

本批仍为 2 个工作树、4 个本地分支。主 checkout 的 HEAD `020f9d40`、40 条既有状态、暂存区 SHA256 和 42 个文件/缺失路径均与之前备份一致；另观察到其他任务新增未跟踪交接 `2026-10-30-main-javascript-audit.md`，没有读取修改、暂存或提交它。回执为 `target/density-retirement-20261030/primary-preservation.json`。

## 验证与验收

本批本地原始证据统一在 `target/density-retirement-20261030/`，不能用之前 PR #47 或主题评估的通过结果代替。测试资料均为合成隔离库；紧凑回执见[验收汇总](../testing/evidence/2026-10-30-density-retirement/receipt.json)。

| 命令 / 场景 | 实际结果 | 证据 |
| --- | --- | --- |
| `cargo test -p rssr-app --locked` | 387 通过、1 忽略；含新纯 Rust 与 Dioxus 草稿回归 | `app-tests-final.log` |
| `cargo test -p rssr-infra --locked --test test_library_settings_repository --test test_config_package_codec --test test_config_package_schema_consistency --test test_library_config_package_io` | 15 通过 | `compat-tests.log` |
| `cargo clippy -p rssr-app -p rssr-cli --all-targets --locked -- -D warnings` | 通过 | `clippy.log` |
| `cargo build -p rssr-app -p rssr-web -p rssr-cli --locked`；同源 fixture example 构建 | 通过，产物与当前产品文件绑定 | `native-server-build.log`、`fixture-build.log` |
| `dx build --platform web --package rssr-app --locked --release --debug-symbols false` | 通过，正式 Web，不启用 pages-demo | `web-build.log` |
| `rssr_server_acceptance.mjs --only retired-density-layout-and-drafts,settings-preview-explicit-save,build-mismatch-keeps-edits,stopped-departure-then-saved,slow-departure-lands-late,later-edit-not-kept-stays --long` | 6/6 通过；新场景在 1280/390 两宽度检查 Entries/Feeds/Settings，两旧密度值布局一致、无横向溢出；旧单字段/混合草稿恢复与显式保存通过 | `web-01/assertions.json`、12 组 HTML/PNG |
| `rssr_native_preparation_acceptance.cjs` | 1536/390 共 8/8 通过；保存、回填、下载、数值反馈、主题文件路径 | `web-preparation/acceptance.json` |
| `rssr_theme_selector_windows.mjs`，新 Windows EXE / WebView2 实际宿主 | 5/5 通过；旧 compact 使用 72px 默认行高 / 10px 上内边距，密度控件消失；原生宿主弹窗、取消、选择、重读、保存重开 | `windows-01/acceptance.json`、UIA 树 |
| `rssr_shell_settings_acceptance.mjs`，实际 Windows 宿主 | 3/3 通过；启动、外部写入后重读、保留待存字段 | `windows-lifecycle/acceptance.json` |
| `dx build --locked --platform android --package rssr-app --target aarch64-linux-android --debug-symbols false --verbose`，独立 target / JDK 21 / NDK 28.2 | 编译、打包、包名与库文件散列核对通过；`adb install -r -t` 成功 | `android-build-isolated.log`、`artifact-identity.json`、`android-install.log` |
| `rssr_theme_selector_android.mjs`，Android 15 / API 35，2201123C 真机独立包 | 4/4 通过；旧 compact 仍为默认间距且无密度控件，系统 ListView 选择 / 实际返回键取消 / 预览 / 保存重开 | `android-01/acceptance/acceptance.json`、XML / PNG |
| `rssr-cli save-settings --help`、修改过的 JS `node --check` | 通过，旧参数标明弃用 | `cli-save-help.txt` |
| `cargo fmt --all --check`、`git diff --check`、文档链接检查 | 通过；326 份 Markdown，无新增错误 | `documentation-check.json` |

### Review 与失败尝试

自审逐字段保存、只读重读、旧 wire 格式和 CSS 回退，没有发现待修的阻断问题，也没有更改它们的架构。新增密度过滤后，发现“只有密度的旧草稿”会因没有剩余编辑而未消费缓存；先用 Dioxus 测试得到失败，再让 session 在交接草稿后统一调用已有 `drafts::forget()`。其 `LEAVING` 防护仍有效，新的回归与跨登录 / 重新加载场景通过。失败证据 `legacy-draft-before.log` 属于本批中间实现问题，不称为主线既有缺陷。

Android 首次复用共享 target 时，CLI 报构建成功但 APK 散列仍是旧包；依赖文件指向上一次源码快照。未安装该旧候选，改用本批独立 `android-target/` 完整构建。新产物核对包名、散列后才安装。`dx 0.7.10` 与锁定 Dioxus `0.7.9` 的版本提示、Android 的既有未使用常量警告及 Gradle 弃用提示保留在日志，未通过升级依赖或改产品规避。真机首张 UIAutomator 树读取失败一次，沿用已有最多一次的只读重试后成功，报告保留该次尝试；交互断言未降低。

## 产物身份与覆盖边界

- 当前产品快照以基线 HEAD 加 `source.patch` 记录；补丁 SHA256：`3b5304e7828601cd170838e743a1f0e61d2a5e0a955e9b6d5fc174afcb4d12cd`。455 个产品输入文件逐一核对在 `product-files.json`；Android 副本仅额外改变测试包 identifier / Kotlin BuildConfig 引用。文档后续更新不改变这些产品输入。
- Windows EXE SHA256：`da145a222a5849c23cdd36975539502a02ccfd2794ed15f1ff49743e851975eb`。
- 新 Android APK SHA256：`0044b89d149ba9a4e103bd2c96d85cb7b07eb6a698b9a5d85351f8dae55c7db5`；包 `io.github.develata.rssreader.backpolicytest`，Activity `dev.dioxus.main.MainActivity`，min SDK 24 / target 34。
- Android 旧测试包、测试 files 与资料库已备份校验；完成后原测试资料库恢复，SHA256 回到 `13a5449fa88b4fd9332670ca7bff4834c9ea740e906e825751ad48a487ebca0a`，只移除本批 CDP forward，原 forward / reverse 保持。日常包 `io.github.develata.rssreader` 未改，SHA256 `32111b52c2396e53cd7afba5578f2b0b9aa2b9ee96e6024a061b0b4d4ee6fa8e`。
- Web 截图只算浏览器呈现；Windows CDP 输入和 UIA 不等于物理键盘 / IME 验收。未重跑 macOS/Linux GUI、全量 Android 生命周期、屏幕阅读器、发布包升级或性能测试。本批不声明 CI、Docker 或正式发布验收。

## 结果与后续

实现和相称测试完成后仅提交本任务文件，按已有授权推送当前分支；本批没有并入 main 或向已合并的 PR #40 / #47 追加实现。下一步的产品收口为 0.3.x 删除旧设置参数，范围以维护待办为准。

下一位 Agent 从本交接、[当前状态](../current-status.md)和[原生适配契约](../design/native-control-adapter-contracts.md)继续；不要根据仓库名或旧 handoff 切换、清理主 checkout，也不要把临时保留字段重新加回 UI。

# 任务 3：按当前筛选批量标为已读

- 日期：2026-09-25
- 作者 / Agent：Codex
- 分支：detached worktree `.handoff/worktrees/task3`
- 当前 HEAD：2ba826a
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`draft`

## 工作摘要

实现全部分页的当前筛选批量已读，预览冻结查询与完整未读 ID 集合；确认时原子校验集合，变化需重新确认。环境检查未全通过，按用户决定保留改动但不提交。

## 影响范围

- 模块：domain 仓储契约、application 列表用例、SQLite / 浏览器 adapter、文章页、CLI、说明文档。
- 平台：Web / Linux / Windows / macOS / Android；CLI。
- 额外影响：无数据库迁移，无依赖版本升级；CLI 新增既有 workspace 的 time 依赖。

## 关键变更

- `MarkReadPreview { query: EntryQuery, unread_entry_ids: Vec<i64> }`；`MarkReadOutcome::Applied { changed_count: u64 } | SelectionChanged { preview: MarkReadPreview }`。
- `EntryIndexRepository::preview_mark_read(&self, &EntryQuery) -> Result<MarkReadPreview>`、`mark_read_if_unchanged(&self, &MarkReadPreview) -> Result<MarkReadOutcome>`；EntriesListService 与 EntriesPort 同名透传，mock 构造同步更新。其余仓储方法未变。
- SQLite BEGIN IMMEDIATE 中比较排序 ID 后一次 UPDATE；浏览器锁内比较，一次 flags localStorage 写入，失败恢复内存快照。不使用已加载分页推算范围。
- UI PreviewMarkRead / ConfirmMarkRead 命令；确认、取消、busy 去重、筛选变化失效；成功重新加载当前查询、夹紧分页、刷新权威来源计数。反馈移至折叠筛选区外。
- 稳定接口：`data-layout="entry-bulk-read"`，`data-state="idle|confirm"`；`data-action="preview-mark-filtered-read|confirm-mark-filtered-read|cancel-mark-filtered-read"`。样式独立 CSS、按钮 ≥44px。
- CLI 新增 `mark-read (--all|--feed-id <id>) [--search] [--read-filter all|unread|read] [--starred-filter all|starred|unstarred] [--archive-filter active|all|archived] [--yes]`，未带 yes 只预览。

## 验证与验收

### 自动化验证

日志位于共享 `target/task3-validation/`；checks.tsv 记录各次运行，末尾七项为最终结果。

| 命令 | 退出码 | 结果 |
|---|---:|---|
| cargo fmt --all --check | 0 | 通过 |
| cargo clippy --workspace --all-targets -- -D warnings | 101 | 缺 GTK/WebKit 等开发 pkg-config 库 |
| cargo test --workspace | 101 | 同上，未执行完 workspace 测试 |
| cargo check -p rssr-app --target wasm32-unknown-unknown | 0 | 通过 |
| cargo clippy -p rssr-app --target wasm32-unknown-unknown -- -D warnings | 0 | 通过 |
| cargo check -p rssr-app --target aarch64-linux-android | 101 | 缺 aarch64-linux-android-clang / Linux NDK |
| git diff --check | 0 | 通过 |
| cargo test -p rssr-application -p rssr-infra -p rssr-cli | 0 | 全部通过 |
| cargo clippy -p rssr-application -p rssr-infra -p rssr-cli --all-targets -- -D warnings | 0 | 通过 |
| cargo test -p rssr-infra --test test_bulk_read -- --nocapture | 0 | 2 项通过；5 万条首次测量预览 108ms、写入 441ms，仅代表本机本次 |
| bash scripts/run_wasm_contract_harness.sh wasm_subscription_contract_harness | 0 | 5 项通过，含共享筛选案例、集合变化、失败回滚、一次写入 |
| dx build --package rssr-app --platform web | 0 | 构建成功；dx 0.7.10 报与 Dioxus 0.7.9 不匹配，未安装升级 |
| bun target/task3-validation/browser.cjs | 0 | 两视口全部断言通过 |

Wasm harness 默认自动 driver 初次失败 connection reset；后以本机 ChromeDriver 151 服务 9518、`CHROMEDRIVER_REMOTE=http://127.0.0.1:9518`、取消代理并使用既有 wasm-bindgen 0.2.126 runner 重跑成功。日志 wasm-subscription-remote.log。
CLI 初次 fixture 数据库未创建失败（code 14）；改用已有 SQLite URL 参数 `?mode=rwc` 后，预览 3 篇未写入、--yes 后 3 篇全部 read_at 非空；实际断言退出 0。

### 手工验收

- 真实 Chromium 151 + Playwright（bun 执行），360×800、1280×800；60 篇跨三页，预览全量60；单条先标已读后确认要求重确认59；重复点击只一次 flags 写入；60篇最终全已读；未读筛选空态正确、来源数0、再次预览0隐藏确认。
- 注入 Storage 写入异常：错误反馈可见，持久化 flags 没有变化；修改筛选关闭待确认。确认按钮≥44px，页面无横向溢出。截图 confirm-360.png / confirm-1280.png，结果 browser.json；已查看移动截图。
- 未运行：当前代码的原生桌面界面验收，缺开发库无法构建。未运行：Android 实机/模拟器，本轮已确认只做编译检查，且缺 NDK。

## 结果

已验证 Web 主流程、两种存储契约与 CLI；不是全平台通过，不可按验收门槛提交。无 commit / push / tag。

## 风险与后续事项

- 环境安装需用户另行允许，未安装、未解包：原生依赖建议 `sudo apt-get install libgtk-3-dev libwebkit2gtk-4.1-dev libxdo-dev`；Android 建议 `sdkmanager --install "ndk;27.3.13750724"`，随后按 CI 配置环境。
- 浏览器沿用现有单实例内存模型；未宣称多标签页并发一致性。
- 独立 worktree 未合并任务2；环境修复后需集成两个页面改动，再跑回归。`target` 为本地共享构建目录的未跟踪 symlink，不属于提交内容。

## 给下一位 Agent 的备注

先看 domain MarkReadPreview / MarkReadOutcome，再看两个 repository 实现与共享 cases，最后 session / reducer。不要提交 root 中尚未全验收的任务2；不要清理工作树或共享 target。

## 改动文件清单

- `Cargo.lock`：为CLI复用workspace已有time依赖。
- `README.md`：同步本任务的用户流程、行为口径和限制。
- `assets/styles/entries.css`：增加可换行操作区、长文本布局和44px触控目标样式。
- `crates/rssr-app/src/pages/entries_page/bulk.rs`：呈现批量已读预览、确认、取消和忙态。
- `crates/rssr-app/src/pages/entries_page/controls.rs`：连接页面状态、交互事件、异步结果与渲染。
- `crates/rssr-app/src/pages/entries_page/facade.rs`：连接页面状态、交互事件、异步结果与渲染。
- `crates/rssr-app/src/pages/entries_page/intent.rs`：连接页面状态、交互事件、异步结果与渲染。
- `crates/rssr-app/src/pages/entries_page/mod.rs`：连接页面状态、交互事件、异步结果与渲染。
- `crates/rssr-app/src/pages/entries_page/reducer.rs`：连接页面状态、交互事件、异步结果与渲染。
- `crates/rssr-app/src/pages/entries_page/session.rs`：连接页面状态、交互事件、异步结果与渲染。
- `crates/rssr-app/src/pages/entries_page/state.rs`：连接页面状态、交互事件、异步结果与渲染。
- `crates/rssr-app/src/ui/commands/entries.rs`：连接命令执行、结果映射或反馈生命周期。
- `crates/rssr-app/src/ui/runtime/entries.rs`：连接命令执行、结果映射或反馈生命周期。
- `crates/rssr-app/src/ui/runtime/services.rs`：连接命令执行、结果映射或反馈生命周期。
- `crates/rssr-application/src/entries_list_service.rs`：扩展统一用例/组合端口并透传本任务结果。
- `crates/rssr-application/src/feed_service.rs`：同步测试仓储替身，明确不支持未使用的批量操作。
- `crates/rssr-application/src/import_export_service/tests.rs`：补充或适配用例、存储契约和失败路径验证。
- `crates/rssr-application/src/reader_service.rs`：同步测试仓储替身，明确不支持未使用的批量操作。
- `crates/rssr-application/src/subscription_workflow.rs`：同步测试仓储替身，明确不支持未使用的批量操作。
- `crates/rssr-cli/Cargo.toml`：为CLI复用workspace已有time依赖。
- `crates/rssr-cli/src/main.rs`：增加按筛选预览及确认执行的mark-read命令。
- `crates/rssr-domain/src/entry.rs`：定义并导出批量已读预览/结果与仓储契约。
- `crates/rssr-domain/src/lib.rs`：定义并导出批量已读预览/结果与仓储契约。
- `crates/rssr-domain/src/repository.rs`：定义并导出批量已读预览/结果与仓储契约。
- `crates/rssr-infra/src/application_adapters/browser/adapters/entry.rs`：实现或复用浏览器适配逻辑，保持平台差异在边界内。
- `crates/rssr-infra/src/application_adapters/browser/query.rs`：实现或复用浏览器适配逻辑，保持平台差异在边界内。
- `crates/rssr-infra/src/db/entry_repository.rs`：在写事务内校验筛选集合并一次批量更新。
- `crates/rssr-infra/tests/support/bulk_read_cases.rs`：补充或适配用例、存储契约和失败路径验证。
- `crates/rssr-infra/tests/test_bulk_read.rs`：补充或适配用例、存储契约和失败路径验证。
- `crates/rssr-infra/tests/wasm_subscription_contract_harness.rs`：补充或适配用例、存储契约和失败路径验证。
- `docs/design/frontend-command-reference.md`：同步命令、状态流转与反馈规则。
- `docs/design/theme-author-selector-reference.md`：登记新增稳定选择器与样式接口。
- `docs/handoffs/2026-09-25-task3-filtered-mark-read.md`：记录本任务接口、验收证据和剩余阻塞。
- `docs/user-guide.md`：同步本任务的用户流程、行为口径和限制。

## 最终环境备注

本轮创建的临时HTTP fixture、SPA验收服务和ChromeDriver已停止，截图与日志保留在target中。最终PATH未找到sdkmanager、adb或xvfb-run，ANDROID_HOME/ANDROID_SDK_ROOT未配置；NDK安装命令以已有Linux Android command-line tools为前提，本轮没有安装。各worktree仍以2ba826a为HEAD，全部改动未暂存，未commit/push。

## 2026-09-26 主线集成复验

已在任务 2 提交 `4d0532e` 上整合本任务，保留原隔离 worktree。列表 facade、筛选副作用与 CSS 的三处重叠已合并：保留分页恢复和批量确认各自语义。以下补验结果覆盖此前缺 GTK/WebKit / NDK 的阻塞状态；完整日志见 `target/integration-validation/task3-*`。

- 合并后 fmt、workspace clippy（-D warnings）、workspace test、wasm check / clippy、Android ARM64 check、工作树与暂存区 diff check 全部退出 0。
- subscription wasm harness 重跑退出 0：5 passed，包含批量筛选一致性和失败回滚；未使用旧 bundle 代替合并后契约。
- test_entry_state_and_search 单独执行退出 0；test_bulk_read --nocapture 退出 0，5 万条本次预览 182ms、应用 598ms，仅代表本机本轮。
- 合并后 Web 构建与 browser.cjs 退出 0；360×800 / 1280×800 的 60 条跨三页、59 条重确认、单次写入、零结果和失败回滚通过。
- 合并后任务 2 positions.cjs 回归退出 0，两视口列表/正文/分页恢复与主动输入取消均保持正确。
- 本轮确认范围已通过，任务 3 达到本地提交条件；原隔离 worktree 仍保留未提交副本，不做清理。

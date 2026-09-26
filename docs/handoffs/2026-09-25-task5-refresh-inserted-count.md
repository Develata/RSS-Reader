# 任务 5：刷新真实新增计数

- 日期：2026-09-25
- 作者 / Agent：Codex
- 分支：detached worktree `.handoff/worktrees/task5`
- 当前 HEAD：2ba826a
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`draft`

## 工作摘要

存储层返回真实新增数量，手动刷新显示新增/零新增、部分失败与全部失败；保持自动刷新静默及阅读页图标规则。缺原生依赖/NDK，全平台检查未通过，不提交。

## 影响范围

- 模块：application refresh service/contract，SQLite与browser刷新存储，app host/shell/feeds，CLI，测试与文档。
- 平台：Web / 桌面 / Android / CLI。
- 额外影响：无schema迁移、无新依赖、无上次计数持久化。

## 关键变更

- SQLite 在 BEGIN IMMEDIATE 写事务内比较同feed行数前后差，真实新插入计数；冲突更新及同批重复键不计入。旧正文解析和分库写入路径保留。
- 浏览器 upsert 创建新索引才递增，不从解析条目数推算。批次结果在 end_batch 成功后才发布；落盘失败沿用既有逻辑改写失败，计数不以成功反馈呈现。
- 全局 / 单订阅 / CLI反馈接入；成功3秒、错误6秒。单订阅反馈以revision和消息匹配，旧定时器不能清除新反馈。RefreshFlight、begin/end/abort生命周期、自动刷新静默、Reader视觉规则不变。
- 文案“新增 N 篇文章”/“没有新文章”；部分失败保留成功订阅新增数和失败信息，全失败仅错误。CLI --all输出各成功feed及合计，失败仍非零退出。

### 公开契约

- `RefreshCommitOutcome { inserted_count: u64 }`（新增，Default为0）。
- `RefreshStorePort::commit(&self, feed_id: i64, commit: RefreshCommit) -> Result<RefreshCommitOutcome>`。
- `RefreshFeedResult::Updated` 保留 `entry_count: usize`（解析数量）与 `localization_entries`，新增 `inserted_count: u64`；`RefreshFeedOutcome::inserted_count(&self) -> u64`。
- `RefreshAllSummary.inserted_count: u64`。
- infra `EntryUpsertOutcome { contents: Vec<ResolvedEntryContent>, inserted_count: u64 }` 与 `SqliteEntryRepository::upsert_entries_with_outcome(&self, i64, &[ParsedEntry]) -> DomainResult<EntryUpsertOutcome>`；旧 upsert_entries / upsert_entries_and_resolve_contents 签名、返回语义不变。
- browser state `upsert_entries(&mut BrowserState, i64, Vec<ParsedEntry>) -> anyhow::Result<u64>`。
- host内部 RefreshAllExecutionOutcome新增inserted_count/total_count/failed_count，RefreshFeedExecutionOutcome新增inserted_count。UI新增单订阅反馈ClearRefreshStatus意图与revision；不是domain契约。
- trait其余方法、数据库schema、CLI参数、稳定data-*接口均不变。

## 验证与验收

### 自动化验证

真实日志：共享 `target/task5-validation/`。

| 命令 | 退出码 | 结果 |
|---|---:|---|
| cargo fmt --all --check | 0 | 通过 |
| cargo clippy --workspace --all-targets -- -D warnings | 101 | 缺 GTK/WebKit 开发pkg-config库 |
| cargo test --workspace | 101 | 同上，未执行完 |
| cargo check -p rssr-app --target wasm32-unknown-unknown | 0 | 通过 |
| cargo clippy -p rssr-app --target wasm32-unknown-unknown -- -D warnings | 0 | 通过 |
| cargo check -p rssr-app --target aarch64-linux-android | 101 | 缺NDK的aarch64-linux-android-clang |
| git diff --check | 0 | 通过 |
| cargo test -p rssr-application -p rssr-infra -p rssr-cli | 0 | 全部通过，含共享新增计数案例 |
| cargo clippy -p rssr-application -p rssr-infra -p rssr-cli --all-targets -- -D warnings | 0 | 通过 |
| cargo check -p rssr-app --tests --target wasm32-unknown-unknown | 0 | 测试代码编译，不是执行 |
| cargo test -p rssr-infra --test test_feed_refresh_flow | 0 | 2项通过 |
| bash scripts/run_wasm_contract_harness.sh wasm_refresh_contract_harness | 0 | 20项通过，含新增共享案例与原批次不变量 |
| dx build --package rssr-app --platform web | 0 | 成功；dx0.7.10与Dioxus0.7.9版本警告未掩盖 |
| bun target/task5-validation/browser.cjs | 0 | 两视口全部断言通过 |

另补强批次落盘失败不发布非零计数后，cargo test -p rssr-application 与对应all-targets clippy -D warnings 再跑通过。
Wasm harness采用既有ChromeDriver151 remote9518及wasm-bindgen0.2.126；无工具安装。
CLI真实HTTPfixture验证 `refresh --all` 首次1/再次0，`refresh --feed-id 1`新增两篇2/仅正文修改0，输出及数据库均由实际操作产生，断言退出0。初次遗漏既有--all必选参数返回2，改命令后通过。

### 手工验收

- Chromium151 / Playwright（bun执行），360×800、1280×800：真实页面+受控feed响应。新增1、重复0、正文更新0、再新增2；成功2秒时仍可见、3.3秒清除；部分失败3.3秒仍显示、6.3秒清除；全失败只错误，无新增文案。
- 刷新中进入/entries/1，仍保留该文章，live region含完整新增反馈但视觉宽度1px，R图标反馈；单feed显示新增数且3秒清除；自动刷新不显示手动反馈。
- 无横向溢出，截图new-360.png/new-1280.png；结果browser.json。测试初版未等重载后的自动刷新结束，自动刷新先消费新增导致预期不符；通过观测实际请求完成再启动手动操作修正fixture时序，未修改产品代码来迁就测试。
- 未运行：桌面实际GUI（缺开发库），Android模拟器/实机（本轮只编译且NDK缺失）。

## 结果

Web反馈、CLI、native及browser存储计数均有执行证据；不是全平台验收完成。未commit/push/tag。

## 风险与后续事项

- SQLite索引/正文两库既有部分失败边界保留：索引已提交但后续正文失败时该feed报失败，其新增不计成功汇总；不宣称两库原子提交。
- browser沿用批次失败时保留内存脏状态并重试的既有语义，不新增跨标签页同步。
- 环境安装命令见任务3交接，未经授权未安装/解包。需修复环境后集成任务2/3/4/5并重新验证；当前worktree独立。
- `target`为未跟踪本机symlink，不属于提交内容。

## 给下一位 Agent 的备注

集成任务4时，保留其apply_source_output公共复用路径，并在该路径使用本任务commit返回值。不要丢掉任何独立worktree改动或提前提交失败任务。

## 改动文件清单

- `README.md`：同步本任务的用户流程、行为口径和限制。
- `crates/rssr-app/src/bootstrap.rs`：同步平台装配和host capability结果透传。
- `crates/rssr-app/src/bootstrap/native.rs`：同步平台装配和host capability结果透传。
- `crates/rssr-app/src/bootstrap/refresh_flight.rs`：同步平台装配和host capability结果透传。
- `crates/rssr-app/src/bootstrap/web.rs`：同步平台装配和host capability结果透传。
- `crates/rssr-app/src/pages/feeds_page/intent.rs`：连接页面状态、交互事件、异步结果与渲染。
- `crates/rssr-app/src/pages/feeds_page/reducer.rs`：连接页面状态、交互事件、异步结果与渲染。
- `crates/rssr-app/src/pages/feeds_page/session.rs`：连接页面状态、交互事件、异步结果与渲染。
- `crates/rssr-app/src/pages/feeds_page/state.rs`：连接页面状态、交互事件、异步结果与渲染。
- `crates/rssr-app/src/ui/mod.rs`：连接命令执行、结果映射或反馈生命周期。
- `crates/rssr-app/src/ui/runtime/feeds.rs`：连接命令执行、结果映射或反馈生命周期。
- `crates/rssr-app/src/ui/runtime/mod.rs`：连接命令执行、结果映射或反馈生命周期。
- `crates/rssr-app/src/ui/runtime/shell.rs`：连接命令执行、结果映射或反馈生命周期。
- `crates/rssr-app/src/ui/shell.rs`：连接命令执行、结果映射或反馈生命周期。
- `crates/rssr-application/src/lib.rs`：扩展统一用例/组合端口并透传本任务结果。
- `crates/rssr-application/src/refresh_service.rs`：扩展统一用例/组合端口并透传本任务结果。
- `crates/rssr-application/src/subscription_workflow.rs`：扩展统一用例/组合端口并透传本任务结果。
- `crates/rssr-cli/src/main.rs`：输出每个订阅及合计的真实新增数量。
- `crates/rssr-infra/src/application_adapters/browser/adapters/refresh.rs`：实现或复用浏览器适配逻辑，保持平台差异在边界内。
- `crates/rssr-infra/src/application_adapters/browser/state/entries.rs`：实现或复用浏览器适配逻辑，保持平台差异在边界内。
- `crates/rssr-infra/src/application_adapters/refresh.rs`：适配本任务的跨层实现与契约。
- `crates/rssr-infra/src/db/entry_repository.rs`：在同一索引事务内统计真实新增，保留原正文接口。
- `crates/rssr-infra/tests/support/refresh_count_cases.rs`：补充或适配用例、存储契约和失败路径验证。
- `crates/rssr-infra/tests/test_application_refresh_store_adapter.rs`：补充或适配用例、存储契约和失败路径验证。
- `crates/rssr-infra/tests/test_subscription_contract_harness.rs`：补充或适配用例、存储契约和失败路径验证。
- `crates/rssr-infra/tests/wasm_refresh_contract_harness.rs`：补充或适配用例、存储契约和失败路径验证。
- `crates/rssr-infra/tests/wasm_subscription_contract_harness.rs`：补充或适配用例、存储契约和失败路径验证。
- `docs/design/frontend-command-reference.md`：同步命令、状态流转与反馈规则。
- `docs/handoffs/2026-09-25-task5-refresh-inserted-count.md`：记录本任务接口、验收证据和剩余阻塞。
- `docs/user-guide.md`：同步本任务的用户流程、行为口径和限制。

## 最终环境备注

本轮创建的临时HTTP fixture、SPA验收服务和ChromeDriver已停止，截图与日志保留在target中。最终PATH未找到sdkmanager、adb或xvfb-run，ANDROID_HOME/ANDROID_SDK_ROOT未配置；NDK安装命令以已有Linux Android command-line tools为前提，本轮没有安装。各worktree仍以2ba826a为HEAD，全部改动未暂存，未commit/push。

## 2026-09-26 主线集成复验

- 在任务 4 提交 `3930289` 上整合（已含任务 2、3）；保留候选选择状态与反馈清除 revision。任务 4 的首次入库走同一 apply_source_output，取得实际存储返回的 inserted_count。
- 新插入计数测试补齐 NewFeedSubscription.site_url=None；无 schema、CLI 参数或旧 upsert 方法签名变化。
- fmt、workspace clippy（-D warnings）、workspace tests、wasm check / clippy、Android ARM64 check、工作树与暂存区 diff check 全部退出 0；最终 workspace 为 303 passed、2 ignored。
- 最终三个 wasm harness 均退出 0：refresh 20、subscription 6、config exchange 3 项通过。覆盖实际新增、同批重复、重复刷新、内容更新、失败批次收尾、订阅发现与批量已读共享契约。
- CLI 构建、setup、四次刷新和独立数据库断言全部退出 0；真实 HTTP 8102 测得新增 1 / 0 / 2 / 0，最终 SQLite 恰为 3 篇。
- 最终 Web build 退出 0；同一最终 bundle 串行运行任务 5、4、3 browser.cjs、任务 4 direct-browser.cjs 和任务 2 positions.cjs，全部退出 0。覆盖 360×800 / 1280×800 新增提示、3秒/6秒期限、部分失败、全失败、自动刷新静默、Reader 仅图标、候选发现、批量已读与分页/正文位置恢复；最新 360px 截图已查看。
- 同步修正前端命令文档原有“成功约1秒”为约3秒，用户指南明确历史版本与当前行为，避免只追加章节造成时间说明矛盾。
- 本轮确认范围通过，任务 5 达到本地提交条件；Android 本轮没有运行验收。日志见 target/integration-validation/task5-* 与 final-*。

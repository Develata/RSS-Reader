# 工程审计（2026-10）、三项最小修复与独立评审跟进

> 历史交接：本记录仅说明当时的实现与验收，旧路径和命令可能已退役。当前状态见[文档索引](../../../README.md)，归档范围见[历史索引](../README.md)。

- 日期：2026-10-09
- 作者 / Agent：Claude（Opus 5.5）
- 分支：`audit/2026-10-09`（worktree `target/audit-20261009/worktree`，基于 `origin/main` `e4c4a82`）
- 当前 HEAD：见下方“相关 commit”的最后一个提交
- 相关 commit：`1456b2b`、`28c0441`、`64bdbed`、`0ac5ec7`、`6d8785b`、`7b69a56`、`f814528`、`8ef3a36`，以及包含本次更新的文档提交
- 相关 tag / release：N/A（基线即 v0.1.23 源码快照）
- 状态：`validated`（本地验证；未推送、未开 PR）

## 工作摘要

对 v0.1.23 基线做全仓工程审计，并逐项核对 GPT 2026-10-09 交接意见。完整结论见 [工程审计报告（2026-10）](../../../engineering-audit-2026-10.md)。在审计分支上修复了三项局部、平台中立且可红→绿验证的缺陷，并为八个需要设计决策的缺陷提交了确定性红测试（`#[ignore]`）。报告随后经 `codex-reviewer`（gpt-6-astra，max）独立评审，12 条意见逐条核实后全部采纳，处置见报告第 9 节。

## 影响范围

- 模块：
  - `crates/rssr-infra/src/db/entry_repository.rs`（标题搜索 LIKE 转义）
  - `crates/rssr-infra/src/db/feed_repository.rs`、`application_adapters/refresh.rs`、`application_adapters/browser/adapters/refresh.rs`（304 保留校验器）
  - `crates/rssr-infra/src/application_adapters/browser/query.rs`、`tests/test_persistence_atomicity.rs`（wasm lib 测试可编译）
  - 测试：`rssr-infra/tests/test_entry_state_and_search.rs`、`test_refresh_contract_harness.rs`、`test_audit_20261009_refresh_invariants.rs`、`wasm_refresh_contract_harness.rs`；`rssr-application/src/app_state_service.rs`、`rssr-app/src/pages/entries_page/session.rs` 的测试模块
- 平台：
  - Windows / macOS / Linux / Android（原生搜索与 304）；Web（浏览器 304 语义、wasm 测试目标、`wasm_refresh_contract_harness` 新用例）
- 额外影响：
  - 文档：新增审计报告；`docs/README.md` 索引；`docs/user-guide.md` 补充“标题搜索按字面匹配”

## 关键变更

### 标题搜索

- `%`、`_`、`\` 作为字面量匹配（`ESCAPE '\'`），列表、计数、批量已读预览与实际写入一致；恢复与 Web 字面 `contains` 的一致性。Unicode 大小写差异未改，待决策。

### 304 Not Modified

- 原生新增 `record_not_modified_for_generation`，只更新 304 中出现的 ETag / Last-Modified；浏览器适配器同语义。200 与失败路径不变。

### wasm 测试目标

- 浏览器查询测试夹具补 `feed_generations`；`test_persistence_atomicity.rs` 增加原生门控；wasm 契约 harness 新增“304 省略校验器”用例。

### 红测试（`#[ignore = "audit-2026-10-09: …"]`）

- AppState 并发丢更新；在途查询覆盖已读写入；源未变刷新冲掉本地化正文；同一订阅的存储结果不属于最后开始的抓取（策略中立、全程看门狗）；无 User-Agent；XML 声明 GBK 乱码；HTTP 与 XML 都声明 GBK 时重复解码；Atom 链接按位置选择。

### 独立评审跟进

- 同源刷新红测试改为策略中立，避免按 single-flight 修复后挂起；AppState 夹具交错未建立时直接失败；报告中 D1、G1、B2、F1、F3、A1、B6 与若干证据标签按评审修订；B2 经探针证实范围大于初稿。

## 验证与验收

### 自动化验证

- 基线 `cargo test --workspace --locked -j 4`：通过，356 passed / 0 failed / 3 ignored
- 基线 `cargo clippy --workspace --all-targets --locked -j 4 -- -D warnings`：通过
- 基线 `cargo check -p rssr-infra --target wasm32-unknown-unknown --lib --tests --locked`：失败（`E0063 missing field feed_generations`），修复后通过
- 三项修复的新测试：在未修复代码上失败、修复后通过（红→绿）；浏览器端 304 在本地 headless Chrome 的 `wasm_refresh_contract_harness` 中同样红→绿（修复后 29 passed / 0 failed，还原后 28 passed / 1 failed）
- `8ef3a36`：`cargo fmt --all --check` 通过；`cargo clippy --workspace --all-targets --locked -j 3 -- -D warnings` 通过；`cargo clippy -p rssr-infra --target wasm32-unknown-unknown --lib --tests -- -D warnings` 通过
- `8ef3a36`：`cargo test --workspace --locked -j 3` 通过，358 passed / 0 failed / 11 ignored
- `cargo clippy -p rssr-app --target wasm32-unknown-unknown -- -D warnings`：通过（`7b69a56`；之后的提交未改 rssr-app 的非测试代码）
- 八个红测试 `-- --ignored`：全部按所述原因失败；同源测试在模拟“按 feed 串行化”的修复后 2.03 秒通过、无挂起（临时副本，未提交）

### 手工验收

- 真实浏览器部署、Android、macOS、Linux：未执行
- 另外两个 wasm 契约 harness 与 UI 断言脚本：未执行

## 结果

- 三项修复可单独合并，不依赖其余发现；红测试不影响 CI（默认忽略）。
- 搜索行为对用户可见：含 `%`、`_` 的搜索词不再扩大命中范围。

## 风险与后续事项

- 优先级与执行顺序见审计报告第 6 节；需决策事项 7 项：无正文条目去留、订阅自定义名、搜索大小写契约、迁移换行符兼容方案、后台图片私网访问、GUI/CLI 共享数据库、Web 正文保留期限。
- 报告已经过一次跨模型独立评审；修订后的版本未再次送审。
- 本地构建注意：16 GB 内存下默认并行曾触发 `os error 1455`，建议 `CARGO_INCREMENTAL=0` 与 `-j 3`/`-j 4`。本地运行 wasm harness 需与 `Cargo.lock` 一致的 `wasm-bindgen-cli 0.2.126`（本轮隔离安装在会话草稿目录，未改全局 0.2.128），通过 `RSSR_WASM_BINDGEN_TEST_RUNNER` 指定。

## 给下一位 Agent 的备注

- 入口：`docs/engineering-audit-2026-10.md`；红测试清单见其附录 A，评审处置见第 9 节。
- 修复某项缺陷时先运行对应红测试确认失败；多数修复后去掉 `#[ignore]` 即可，AppState 测试需随新端口换夹具，同源刷新的跨进程保证需另写分库夹具。
- 审计 worktree 位于主仓 `target/audit-20261009/`（含独立 `target`、`target-wasm` 构建目录），可按需清理；主工作区 `fix/native-outbound-request-safety` 的未提交改动与本分支无关，未触碰。

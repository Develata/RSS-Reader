# 工程审计（2026-10）与三项最小修复

- 日期：2026-10-09
- 作者 / Agent：Claude（Opus 5.5）
- 分支：`audit/2026-10-09`（worktree `target/audit-20261009/worktree`，基于 `origin/main` `e4c4a82`）
- 当前 HEAD：见下方“相关 commit”的最后一个提交
- 相关 commit：`1456b2b`、`28c0441`、`64bdbed`、`0ac5ec7`，以及包含本记录的文档提交
- 相关 tag / release：N/A（基线即 v0.1.23 源码快照）
- 状态：`validated`（本地验证；未推送、未开 PR）

## 工作摘要

对 v0.1.23 基线做全仓工程审计，并逐项核对 GPT 2026-10-09 交接意见。完整结论见 [工程审计报告（2026-10）](../engineering-audit-2026-10.md)。在审计分支上修复了三项局部、平台中立且可红→绿验证的缺陷，并为七个需要设计决策的缺陷提交了确定性红测试（`#[ignore]`）。

## 影响范围

- 模块：
  - `crates/rssr-infra/src/db/entry_repository.rs`（标题搜索 LIKE 转义）
  - `crates/rssr-infra/src/db/feed_repository.rs`、`application_adapters/refresh.rs`、`application_adapters/browser/adapters/refresh.rs`（304 保留校验器）
  - `crates/rssr-infra/src/application_adapters/browser/query.rs`、`tests/test_persistence_atomicity.rs`（wasm lib 测试可编译）
  - 测试：`rssr-infra/tests/test_entry_state_and_search.rs`、`test_refresh_contract_harness.rs`、`test_audit_20261009_refresh_invariants.rs`；`rssr-application/src/app_state_service.rs`、`rssr-app/src/pages/entries_page/session.rs` 的测试模块
- 平台：
  - Windows / macOS / Linux / Android（原生搜索与 304）；Web（浏览器 304 语义、wasm 测试目标）
- 额外影响：
  - 文档：新增审计报告；`docs/README.md` 索引；`docs/user-guide.md` 补充“标题搜索按字面匹配”

## 关键变更

### 标题搜索

- `%`、`_`、`\` 作为字面量匹配（`ESCAPE '\'`），列表、计数、批量已读预览与写入一致；恢复与 Web 字面 `contains` 的一致性。Unicode 大小写差异未改，待决策。

### 304 Not Modified

- 原生新增 `record_not_modified_for_generation`，只更新 304 中出现的 ETag / Last-Modified；浏览器适配器同语义。200 与失败路径不变。

### wasm 测试目标

- 浏览器查询测试夹具补 `feed_generations`；`test_persistence_atomicity.rs` 增加原生门控。

### 红测试（`#[ignore = "audit-2026-10-09: …"]`）

- AppState 并发丢更新；在途查询覆盖已读写入；源未变刷新冲掉本地化正文；同一订阅旧刷新覆盖新提交；无 User-Agent；XML 声明 GBK 乱码；Atom 链接按位置选择。

## 验证与验收

### 自动化验证

- 基线 `cargo test --workspace --locked -j 4`：通过，356 passed / 0 failed / 3 ignored
- 基线 `cargo clippy --workspace --all-targets --locked -j 4 -- -D warnings`：通过
- 基线 `cargo check -p rssr-infra --target wasm32-unknown-unknown --lib --tests --locked`：失败（`E0063 missing field feed_generations`），修复后通过
- 两项修复各自的新测试：在未修复代码上失败、修复后通过（红→绿）
- 分支 `cargo fmt --all --check`：通过
- 分支 `cargo test --workspace --locked -j 3`：通过，358 passed / 0 failed / 10 ignored
- 分支 `cargo clippy --workspace --all-targets --locked -j 3 -- -D warnings`：通过
- 分支 `cargo clippy -p rssr-infra --target wasm32-unknown-unknown --lib --tests -- -D warnings`：通过
- 分支 `cargo clippy -p rssr-app --target wasm32-unknown-unknown -- -D warnings`：通过
- 七个红测试 `-- --ignored`：全部按所述原因失败

### 手工验收

- 真实浏览器、Android、macOS、Linux：未执行
- wasm 契约 harness 与 UI 断言脚本：未执行

## 结果

- 三项修复可单独合并，不依赖其余发现；红测试不影响 CI（默认忽略）。
- 搜索行为对用户可见：含 `%`、`_` 的搜索词不再扩大命中范围。

## 风险与后续事项

- 优先级与执行顺序见审计报告第 6 节；需决策事项：无正文条目去留、订阅自定义名、搜索大小写契约、迁移换行符兼容方案、后台图片私网访问、GUI/CLI 共享数据库。
- 审计报告尚未经过跨模型独立评审。
- 本地构建注意：16 GB 内存下默认并行会触发 `os error 1455`，建议 `CARGO_INCREMENTAL=0` 与 `-j 3`/`-j 4`。

## 给下一位 Agent 的备注

- 入口：`docs/engineering-audit-2026-10.md`；红测试清单见其附录 A。
- 修复某项缺陷时先运行对应红测试确认失败，修复后去掉 `#[ignore]`。
- 审计 worktree 位于主仓 `target/audit-20261009/`（含独立 `target`、`target-wasm` 构建目录），可按需清理；主工作区 `fix/native-outbound-request-safety` 的未提交改动与本分支无关，未触碰。

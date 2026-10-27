# CLI 输出契约测试：工作区目录名在进程内唯一

- 日期：2026-10-21
- 作者 / Agent：Claude Code（Opus），与 Develata
- 分支：`v0.2-architecture`（工作树 `target/audit-20261009/worktree`）
- 当前 HEAD：修改基于 `3b9f425`
- 相关 commit：`test(cli): give each stdout-contract workspace a unique name`（本记录同一提交）
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

2026-10-20 在 Windows 上跑 `cargo test --workspace -j 2 --no-fail-fast`（提交 `f42394d`）时，`rssr-cli` 的 `json_output_is_parseable_and_export_can_be_imported` 以 `Os { code: 183, kind: AlreadyExists }` 失败；单独重跑三次都通过。原因：`test_stdout_contract.rs` 的 `Workspace::new` 用 `进程号 + SystemTime 纳秒` 命名临时工作区并 `fs::create_dir`，同一进程里并行线程上的两个测试读到同一时钟值，得到同一目录名，后一个创建失败。现在名字里加一个进程内递增序号，同一进程内的两次调用必然得到不同名字，与时钟精度无关。

## 影响范围

- 模块：
  - `crates/rssr-cli/tests/test_stdout_contract.rs`（仅测试）
- 平台：
  - 原生测试（Windows 上实际出现；其它平台同样可能）
- 额外影响：
  - N/A（产品代码不变）

## 关键变更

### `Workspace::new`

- 新增 `static NEXT: AtomicU64`，目录名为 `rssr-cli-输出 contract-{进程号}-{纳秒}-{序号}`，与 `crates/rssr-web/src/auth.rs` 的 `ScratchDir` 做法一致（没有另写重试循环）。
- 保留 `fs::create_dir`（不用 `create_dir_all`）：万一仍撞名，测试会明确失败，而不是两个测试悄悄共用一个工作区。
- 保留路径里的非 ASCII 字符与空格（`输出 contract`、`数据 dir`），它们是测试要覆盖的路径形态。
- 保留纳秒：进程号会被复用，若上次运行崩溃没删掉目录，纳秒仍能把新旧工作区分开。

### 同一模式的排查（`crates/*/tests`）

- 其它用时间戳命名临时目录的测试辅助函数：`rssr-cli/tests/test_backup_process.rs`、`rssr-infra/tests/` 下的 `test_library_backup.rs`、`test_library_import.rs`、`test_library_refresh_tickets.rs`、`test_library_store.rs`——名字都带调用处的标签；逐一统计后，每个测试二进制内标签各不相同（含 `dev8`/`dev9` 循环与 `two_processes(label)` 透传），同一进程内不会撞名，未改动。
- `test_library_app_state_persistence.rs`、`test_library_concurrent_refresh_writes.rs` 各只有一个测试使用该前缀，进程内不会撞名，未改动。
- `support/disk_full.rs` 按标签取固定目录并先删除，设计上每个二进制至多一个这样的用例，不属于该模式。
- 范围外（未改，仅记录）：`crates/*/src` 的单元测试也有同类辅助函数（`rssr-infra` 的 `outbound/policy_file.rs`、`store/{backup,files,fixture,import,local}.rs`）；`rssr-web/src/auth.rs` 已带序号。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`：通过
- `cargo clippy -p rssr-cli --all-targets --locked -j 2 -- -D warnings`：通过
- `cargo test -p rssr-cli --test test_stdout_contract --locked -j 2`，连续 5 次：每次 `5 passed; 0 failed`

### 手工验收

- N/A

## 结果

- 可合并。撞名是低概率竞争，通过的运行本身证明不了修复；修复成立的依据是构造：同一进程内序号两两不同，目录名因此两两不同。

## 风险与后续事项

- 若要彻底消除标签约定带来的潜在撞名（将来有人复用标签、而辅助函数用 `create_dir_all` 会让两个测试悄悄共用目录），可把上述辅助函数统一加序号；目前没有实际撞名，未做。

## 给下一位 Agent 的备注

- 入口：`crates/rssr-cli/tests/test_stdout_contract.rs` 的 `Workspace::new`；参照实现 `crates/rssr-web/src/auth.rs` 的 `ScratchDir::new`。
- 本次编辑经 shell 精确替换完成（本会话根目录不在该工作树，桌面应用的工作树隔离钩子会拦截 Edit/Write），之后跑过 `cargo fmt --all --check`。

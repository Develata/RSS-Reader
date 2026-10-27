# 资料库执行器：坏连接替换测试的偶发超时

- 日期：2026-10-16
- 作者 / Agent：Claude（Opus），与 Develata 协作
- 分支：`v0.2-architecture`（工作树 `target/audit-20261009/worktree`）
- 当前 HEAD：见 git log（修改基于 `2ffab93`）
- 相关 commit：见 git log（`test(store): ...`）
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

`a_connection_that_cannot_be_cleaned_up_is_replaced` 在 `fa3a0ad` 上的一次全工作区并行测试（`-j 4 --no-fail-fast`）中以 `Err(ConnectionTimeout)` 失败（`insert_feed(...).expect("replacement writer")`），单独运行与重跑均通过。查明根因并让该测试不再受负载影响；它证明的内容不变。

## 影响范围

- 模块：
  - `crates/rssr-infra/tests/test_library_store.rs`（只改这一个测试的准入时限）；
  - `crates/rssr-infra/src/store/executor.rs`（只改模块文档，行为不变）。
- 平台：原生（桌面 / Android / CLI）的测试；生产行为不变。
- 额外影响：N/A

## 根因

- 执行器的工作线程在 `serve` 里先运行任务、任务内先把结果发给调用方，**之后**才调用 `Slot::reset_after_task`。连接恢复不了空闲状态时，`take()` 丢弃它：rusqlite 的 `close()` 先 `remove_hooks`（释放授权回调），再 `sqlite3_close`（带着未结束的写事务，内部回滚并关闭文件句柄）。
- 调用方拿到 `NotFound` 后立即提交下一个写任务，其 `start_until = now + admission_wait`；此时唯一的写工作线程还在关闭坏连接，下一个任务处于 `Queued`。关闭耗时超过准入时限（测试 `quick()` 为 200 ms）时，调用方的 `cancel_if_queued` 成功，返回 `ConnectionTimeout`——与失败现象一致。
- 更正原假设：**新连接的打开不在准入时限内**。`Slot::connection()` 在任务从 `Queued` 转为 `Running`、且通过 `start_until` 检查之后才运行；转为 `Running` 后调用方改为不限时地等结果（`receive.await`），重开连接只受任务本身的执行约束。落入准入时限的只有“丢弃坏连接”这一步（加上线程调度）。
- 为什么偶发（推断，未直接测量）：正常情况下这次关闭远小于 200 ms；全工作区 `-j 4` 时 Windows 上的线程调度延迟和文件句柄关闭（可能含杀毒软件对已写文件的关闭时扫描）把它拉长。

### 复现（确定性，未提交）

在 HEAD 快照的临时副本中给 `test_library_store.rs` 加三个测试：授权回调闭包捕获一个 `SlowDrop` 守卫，其 `Drop` 先睡眠再置位一个 `AtomicBool`；连接被丢弃时 `remove_hooks` 释放闭包，守卫的睡眠就落在工作线程的清理阶段。

- 守卫睡 100 ms：失败任务返回时标志仍为 `false` → 结果先于丢弃送达；
- 守卫睡 400 ms + `quick()`（200 ms）：下一个写任务确定返回 `ConnectionTimeout`；
- 守卫睡 400 ms + `admission_wait = 5 s`：下一个写任务成功、`feeds` 计数为 1。

`cargo test -p rssr-infra --test test_library_store --locked -- repro_`：3 passed。

## 关键变更

### 测试

- 该测试改用 `StoreOptions { admission_wait: Duration::from_secs(5), ..quick() }`，与同文件中需要更长准入时限的两个测试一致，并注明原因。
- 它证明的不变：拒绝 `ROLLBACK` 的授权回调使连接无法复位，下一个写任务只能在**新**连接上成功（旧连接会因事务未结束而 `BEGIN IMMEDIATE` 失败）；计数为 1 证明坏任务的插入没有提交。若工作线程在丢弃时死亡，下一个任务仍会在 5 s 后超时，测试照样失败。

### 执行器文档

- 模块文档补一句：复位 / 丢弃发生在调用方拿到结果之后，因此计入下一个任务的准入时限；重开发生在那个任务运行时。

### 未做的事（有意）

- 没有把“发送结果”挪到“复位连接”之后：那会给每个任务的结果多一段复位延迟，属于执行器语义变化；生产上没有实际暴露（见下），不值得。
- “显式等待替换完成”没有可观察的挂点（需要改执行器），不做。

## 生产暴露

结论（推断，依据如下）：没有超出一般排队的暴露。

- 生产只用 `StoreOptions::default()`（`admission_wait = 30 s`）：`rssr-cli/src/main.rs` 的 `open_library_in(&data_dir, StoreOptions::default())`，以及 `store/local.rs` 的 `open_local_library`（GUI 经此打开）；全仓 `rg "StoreOptions|admission_wait"` 再无其它构造。
- 健康连接的复位是一次 `is_autocommit` 检查加一次 `PRAGMA query_only`；坏连接的丢弃是一次 `sqlite3_close`。两者都远小于 30 s；它们与其他调用方排在前面的任务一样计入准入时限，这符合 `admission_wait` 的定义（任务开始前在队列中的全部等待）。
- 坏连接本身罕见：只在 `ROLLBACK` 失败或 `query_only` 设置失败时出现。

## 验证与验收

### 自动化验证

运行于 `2ffab93` + 本次两处修改（Windows 11）。

- `cargo fmt --all --check`（两个文件）：通过；
- `cargo clippy -p rssr-infra --all-targets --locked -- -D warnings`：通过；
- `cargo test -p rssr-infra --test test_library_store --locked`：25 passed；
- `cargo test -p rssr-infra --lib --locked store::`：16 passed；
- 套件二进制（`test_library_store-*.exe` 复制出来直接运行）重复 60 次：全工作区构建期间 20/20、全工作区测试执行期间 40/40，均 25 passed；
- `cargo test --workspace --locked -j 4 --no-fail-fast`：退出码 0，614 passed、0 failed。
- 复现测试（见“根因”）：3 passed，仅在临时副本中，未提交。
- 未执行：WSL / Linux（本次未改执行器行为）。

### 手工验收

- N/A

## 结果

- 可合并：只改一个测试的参数与一段文档，生产行为不变。

## 风险与后续事项

- `quick()` 的 200 ms 准入时限对所有“连续提交任务”的测试都包含工作线程的清理与调度时间；目前只有本测试的清理（关闭连接）足够重。新增类似测试时，若前一个任务会让连接被丢弃，应给足准入时限。
- 未在 Linux（WSL）上运行：本次没有改执行器行为，只改测试参数与文档。

## 给下一位 Agent 的备注

- 入口：`crates/rssr-infra/src/store/executor.rs` 的 `serve`、`Slot::reset_after_task`、`LibraryStore::run`。
- 若以后要让调用方的后续任务不承担前一任务的清理，可在任务闭包里先 `slot.reset_after_task()` 再发送结果；这是执行器语义变化，需按 `store/AGENTS.md` 跑 `test_library_store`、`store::` 单元测试并在 WSL 上验证。

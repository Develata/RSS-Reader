# 资料库文件层：检查点探测测试受负载影响

- 日期：2026-10-20
- 作者 / Agent：Claude Code（Opus），与 Develata
- 分支：`v0.2-architecture`（工作树 `target/audit-20261009/worktree`）
- 当前 HEAD：见提交记录（修改基于 `2f13a16`）
- 相关 commit：见 git log（`test(store): ...`）
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

`store::files::tests::probes_accept_a_library_while_it_is_checkpointed` 在全工作区并行测试里接连失败：2026-10-20 在 Windows 上 `cargo test --workspace -j 2` 两次分别只做了 58 与 48 次检查点，要求至少 64 次；单独运行都通过。改为按检查点数等待，测试不再受机器负载影响，它证明的内容不变。

## 影响范围

- 模块：`crates/rssr-infra/src/store/files.rs`，只改这个测试。
- 平台：原生（桌面 / Android / CLI）的测试；生产行为不变。
- 额外影响：N/A

## 根因

测试让一个写线程在 WAL 模式下反复插入、`wal_checkpoint(TRUNCATE)`，每 64 次检查点之后 `DELETE` 加 `VACUUM` 收缩一次，主线程同时用 `check_library` 不停地探测。旧写法固定探测 1.5 秒，再断言写线程至少做了 64 次检查点，也就是至少收缩过一次。`rssr-infra` 的单元测试并行运行，机器忙时 1.5 秒内到不了 64 次，于是失败。这与被测的探测无关。

## 关键变更

- 探测一直进行，直到写线程做完第 64 次检查点和随后的那次 `VACUUM`（计数到 65），用多久都行。写线程每次检查点后把计数存进一个原子量，主线程据此判断。
- 60 秒的上限只用来终止停滞的写线程，测试不靠它来定时。写线程如果 panic，探测立即结束，`join` 报出原因。
- 断言保持原样：至少 65 次检查点，并且至少探测过一次。

## 验证与验收

### 自动化验证

日志在会话暂存目录 `evidence-checkpoint-test/`。

- Windows（rustc 1.99.0）：
  - `cargo clippy -p rssr-infra --all-targets --locked -- -D warnings`：通过。
  - 单独运行这个测试 3 次：通过，每次约 0.85 秒（旧写法固定 1.5 秒）。
  - `cargo test -p rssr-infra --lib -j 2` 连跑 5 次，与其他 110 个单元测试并行：每次 111 passed / 0 failed。
  - `cargo test -p rssr-infra --test test_library_store`：25 passed。
- WSL Debian（rustc 1.97.0；按 `store/AGENTS.md`，文件层的改动要在 Linux 上跑一遍）：`cargo test -p rssr-infra --lib -j 2` 连跑 3 次，每次 117 passed / 0 failed；`--test test_library_store`：26 passed / 1 ignored。
- 这次修改之前，E 盘只剩 10 GB，本地 Rust 构建被磁盘闸拦下；经 Develata 同意删除了几处构建产物：两个旧的 target 子目录、本工作树的增量缓存，以及主 checkout 的 `target/debug`（其中一个 DLL 被别的进程占用，未删）。

### 手工验收

- 不适用。

## 结果

- 可合并；只改测试。

## 风险与后续事项

- 测试的耗时现在随机器负载变化，平时更短；60 秒上限只在写线程停滞时触发。

## 给下一位 Agent 的备注

- 入口：`crates/rssr-infra/src/store/files.rs` 的测试模块。类似的先例见 `2026-10-16-store-broken-connection-test-flake.md`。

# macOS 本地测试：锁探测类型与认证库临时目录

- 日期：2026-11-12
- 作者 / Agent：Claude Code（Opus 5.5）
- 分支：claude/suspicious-feynman-e9454e
- 当前 HEAD：6a863b8d（改动前）
- 相关 commit：见本分支提交
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

修复两处只在 macOS 本地出现、Linux CI 看不到的测试失败；产品行为不变。

## 影响范围

- 模块：
  - `crates/rssr-infra/tests/test_library_store.rs`（`lock_state_child`）
  - `crates/rssr-web/src/auth.rs`（`test_support::ScratchDir`）
- 平台：
  - macOS（Linux 行为等价）
- 额外影响：
  - 仅测试代码；N/A

## 关键变更

### 锁探测比较类型

- `libc::F_UNLCK` 在 Linux 是 `c_int`，在 macOS 是 `c_short`，原写法 `c_int::from(l_type) == F_UNLCK` 在 macOS 编译失败（E0308/E0277）。
- 改为 `lock.l_type == libc::F_UNLCK as libc::c_short`，与上一行设置 `l_type` 的写法一致；`F_UNLCK` 取值很小，两端比较语义不变，断言强度不变。

### rssr-web 测试临时目录

- macOS 默认 `TMPDIR` 在 `/var/folders/...`，而 `/var` 是指向 `/private/var` 的符号链接；`AuthStore::open` 有意拒绝带符号链接的路径（见 `docs/deployment/web.md`），导致 66 项测试报 "unable to open database file"。
- `ScratchDir::new` 在 Unix 上先对 `std::env::temp_dir()` 做 `canonicalize()` 再拼接子目录；生产侧的符号链接拒绝未放宽。rssr-web 内其他测试均经由 `ScratchDir`。
- 规范化只在 `#[cfg(unix)]` 下进行：Windows 的 `canonicalize` 返回 `\\?\` 前缀路径，SQLite 对它的处理未经核实，Windows 行为保持原样。

## 验证与验收

### 自动化验证

改前复现（把两个文件还原到 6a863b8d 后实测）：

- `cargo test -p rssr-infra --test test_library_store --locked --no-run`：E0308 与 E0277（`can't compare i32 with i16`），编译失败
- `cargo test -p rssr-web --locked`：多项报 `unable to open database file: /var/folders/.../auth.sqlite3`；失败总数 66 来自发起会话的报告，本会话未重新计数

改后：

环境：macOS 27.2 arm64，rustc 1.99.1，默认 `TMPDIR=/var/folders/hp/.../T/`。

- `cargo fmt --all --check`：通过
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：通过
- `cargo test --workspace --locked`：通过（合计 1199 passed，0 failed；`opening_again_keeps_this_process_file_locks` 通过，子进程探测实际执行）
- `cargo test -p rssr-web --locked`：103 passed，0 failed

### 手工验收

- N/A（仅测试代码）

## 结果

- 可合并；无用户或发布影响。

## 风险与后续事项

- 未在 Linux、Windows 本地复跑；Windows 上测试路径与改前相同；Linux 上 `F_UNLCK as c_short` 是 `c_int → c_short` 的取值不变转换，由 CI 覆盖。
- rssr-infra / rssr-cli 其他测试也直接用 `std::env::temp_dir()`，目前在 macOS 通过（那些路径不做符号链接拒绝）；若以后给它们加同样的拒绝，需要同样规范化基目录。

## 给下一位 Agent 的备注

- 入口：`crates/rssr-web/src/auth.rs` 的 `test_support` 模块。

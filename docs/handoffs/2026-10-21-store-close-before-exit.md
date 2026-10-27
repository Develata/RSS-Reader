# 资料库：程序退出前关闭并等待连接；身份探测认出检查点中途的主文件

- 日期：2026-10-21
- 作者 / Agent：Claude Code（Opus），与 Develata
- 分支：`v0.2-architecture`（工作树 `target/audit-20261009/worktree`）
- 当前 HEAD：见提交记录（修改基于 `2397e55`）
- 相关 commit：`2c5c268`（`fix(store): close the library and wait for its connections before exiting`）、`afd4339`（`fix(store): know a library left mid-checkpoint by its application id`）、`a8b7693`（`refactor(store): drop the probe's corruption retry`）；本记录另行提交
- 相关 tag / release：N/A
- 状态：`validated`（两部分；第二部分由 Develata 2026-10-21 确认采用 C）

## 工作摘要

2026-10-20 构建 C4c 浏览器回归时发现：`examples/browser_fixture_library` 运行 60 次，有 3 次 `library.sqlite3` 停在 94208 字节（应为 98304，日志完整），其中 1 次身份探测报“数据库文件已损坏”，`rssr-web` 拒绝启动。原因确认如下，并完成第一部分修复：资料库有了显式的 `close`，CLI、`rssr-web`、桌面端与夹具工具在退出前都调用它并等待。第二部分（身份探测是否区分“检查点中途的主文件 + 有效日志”与真正的损坏）属于存储边界变更，先做分析与原型实测，Develata 确认采用 C 后实现：探测连接设 `writable_schema`，检查点中途的主文件按应用 id 认作资料库，其余交给 SQLite 用日志恢复。

## 原因（已确认）

- 代码阅读：`store/executor.rs` 的工作线程用 `std::thread::Builder` 启动、句柄被丢弃；最后一个 `LibraryStore` 句柄释放后，各线程在 `recv` 出错时返回，连接随 `Slot` 在这些线程上释放，没有人等它们。捆绑的 SQLite 3.53.2 中，`sqlite3PagerClose` → `sqlite3WalClose`：拿到数据库文件的排他锁（普通的回滚模式锁，只有所有进程里的最后一个连接拿得到；CLI 在 `rssr-web` 运行时关闭不会写回）就做一次 PASSIVE 检查点，把日志页按页号升序写回主文件，成功后删除 `-wal`/`-shm`。进程在这期间从 `main` 返回会终止这些线程，主文件停在写回中途：第 1 页已经记着更多页数，文件还没长到那么长。
- `files::check_library` 只读主文件（`immutable=1`，设计如此），SQLite 在 `lockBtree` 里发现“文件头页数 > 文件页数”即报 `SQLITE_CORRUPT`；探测重试 2 秒（`SETTLE`）后按损坏拒绝打开。普通方式打开同一文件时 SQLite 用日志恢复，完全正常。
- 确定性复现（临时原型测试，未提交，Windows 与 WSL 结果一致）：建一个 WAL 库（关闭自动检查点）写入 20 行、日志把文件撑大，保存日志；完整检查点后取主文件；再造出“主文件少最后一页 + 原日志”。今天的探测：`DatabaseCorrupt`；`LibraryStore::open`：约 2.02 秒后 `Corrupt`；普通读写打开：`integrity_check` 为 ok、20 行齐全，日志写回后被删除。
- 进程级确定性复现（WSL）：给库留下约 24.9 MB 未写回的日志，启动 `rssr-web` 后 `kill -TERM`。去掉正常停止时的关闭（变异 M7），3/3 次主文件停在写回中途、日志原样留着、探测报“database disk image is malformed”；有关闭时 3/3 完整。
- 桌面端此前根本不关闭连接：tao 的事件循环结束后 dioxus 直接 `process::exit`，`APP_SERVICES` 是静态量、从不释放。不会撞上关闭时的检查点，但也从不在退出时写回日志。

## 影响范围

- 模块：`crates/rssr-infra/src/store/`（`executor.rs`、`error.rs`、`local.rs`、`fixture.rs`、`AGENTS.md`）、`crates/rssr-cli/src/main.rs`、`crates/rssr-web/src/main.rs`、`crates/rssr-app/src/{main.rs,bootstrap.rs,bootstrap/native.rs}`；测试 `test_library_store.rs`、`test_stdout_contract.rs`、`rssr-web` 的 `api_tests.rs`；文档 `docs/design/v0.2-architecture.md`；第二部分：`crates/rssr-infra/src/store/files.rs` 及其单元测试。
- 平台：原生（桌面 / CLI / 服务器）。Android 没有退出时机（系统结束进程），不变。Web 页面无关。
- 额外影响：`StoreError` 新增 `Closed`（“资料库已关闭”），映射为 `DomainError::Persistence`。

## 关键变更

### 存储：`LibraryStore::close`

- `close(&self)`（异步）与 `close_blocking(&self)`：任何句柄都能调用，对所有句柄生效。先停止每个队列（不再接受任务：之后经任何句柄提交的任务得到 `StoreError::Closed`），已排队的任务照常执行（各自受准入时限与运行期限约束），然后等待每个工作线程结束；线程在最后一个任务之后显式 `Connection::close`，关闭失败会报出来（之前由 `Drop` 静默处理）。返回时每个连接都已关闭，最后一个连接已把日志写回主文件并删除日志。重复或并发调用在第一次等待结束后返回 `Ok`。
- 只丢弃句柄的行为不变（线程排空队列后关闭连接，但没人等）；文档写明程序退出前要 `close`。`close_blocking` 不能在存储任务内部调用（工作线程会等自己）。
- `Queue` 保存工作线程句柄；发送端放在 `Mutex<Option<_>>` 里，关闭时取走。`on_thread` 多了线程名参数（打开、导入、关闭各自命名）。

### 各程序

- CLI：命令主体移到 `run`，`main` 在命令成功或失败后都 `close`；组装用例失败时也关闭。关闭失败只记警告，不改变命令结果（已提交的写入不受影响，下次打开 SQLite 会恢复日志）。
- `rssr-web`：`open_backend` 返回后端与资料库；正常停止时结束运行（`end_run`）与关闭资料库同时进行，共用 `END_RUN_BUDGET`（3 秒），总停止预算不变（4 + 3 + 1 秒，留 2 秒给 Docker 的 10 秒）。出错退出的路径（认证库打不开、组装失败等）也关闭资料库。超时只记警告：此时进程退出仍可能截断检查点。
- 桌面端：`Config::with_custom_event_handler` 在 `Event::LoopDestroyed`（事件循环的最后一个事件，之后 dioxus 即 `process::exit`）调用 `bootstrap::close_local_library()`，它对已打开的资料库 `close_blocking`。Android 不编译这段。
- 夹具（`store::fixture::write_library`）：去掉“丢弃句柄后另开连接做 `wal_checkpoint(TRUNCATE)`”的变通，改为写完后 `close`（写入失败也关闭）。

## 第二部分：身份探测（Develata 确认采用 C，已实现）

第一部分只覆盖正常退出。进程被 `SIGKILL`（Docker 停止超过 10 秒、OOM）、崩溃、断电、Android 系统结束进程，或者提交时的自动检查点被打断，主文件同样会停在写回中途；SQLite 自己能恢复，而今天的探测会让共用数据目录的所有程序都拒绝这个库，直到有人用普通 SQLite 打开它。候选做法与实测（Windows 与 WSL，同一份原型）：

| 做法 | 检查点中途 + 日志 | 检查点中途、无日志 | 旁边的文件 | 与 `files.rs` 的理由 |
| --- | --- | --- | --- | --- |
| 现状：`immutable=1` | 损坏，拒绝 | 损坏，拒绝 | 不动 | — |
| C：同一探测连接加 `PRAGMA writable_schema = ON` | 读到应用 id，接受 | 读到应用 id，接受；随后写连接的 `journal_mode = WAL` 报损坏，打开失败，不建 `-wal`/`-shm`（身份已确认后的 `remove_stale_scratch` 会照常建 `.create-lock`） | 不建、不改 | 仍是 `immutable=1` 只读、经 SQLite 自己的文件句柄，理由都保留 |
| A：损坏时退回 `mode=ro`（非 immutable）+ `NO_CKPT_ON_CLOSE` | 接受 | 损坏 | 新建或改写 `-shm`（32 KB）；无日志时还建空 `-wal` | 在确认身份之前改动旁边的文件，违背“不建、不改 `-wal`/`-shm`” |
| A2：`mode=ro&readonly_shm=1` | Windows 接受但建空 `-shm`；Linux 无 `-shm` 时 `CANTOPEN` | Windows 损坏，Linux `CANTOPEN`；都建空 `-wal` | 会建文件 | 同上，且依赖 `-shm` 是否存在、平台行为不同 |
| B：自己读 100 字节文件头 | 接受 | 接受 | 不动 | 另开文件描述符再关闭会丢掉本进程在该文件上的 POSIX 锁；要安全就得经 SQLite 的文件句柄做 FFI（`unsafe`） |

C 的依据：SQLite 的 `lockBtree`（捆绑源码 `sqlite3.c` 第 76541 行）在 `writable_schema` 打开时不再把“文件头页数 > 文件页数”当损坏，而用文件实际页数；探测只读第 1 页文件头。应用 id（偏移 68）建库时写入、之后不变，检查点写回的第 1 页无论新旧都带着它，所以检查点中途的主文件仍能确认身份。外部库与非数据库文件的结果不变（实测：外部 WAL 库应用 id 为 0，按“不是资料库”拒绝；垃圾文件、只有魔数的文件仍是 `NotADatabase`）。C 放弃的只是探测对“文件比文件头短”的判断，这件事交给写连接：有日志时 SQLite 恢复，没有日志（真截断的文件）时写连接打开即报损坏、拒绝打开，在 Windows 与 Linux 上都不建 `-wal`/`-shm`（实测裸写连接；`open_file` 在探测通过后还会经 `remove_stale_scratch` 建惰性的 `.create-lock`，此时身份已确认）。两种候选对“无日志的截断文件”最终都拒绝，差别只在拒绝发生在哪一步、以及是否在身份确认之前改动旁边的文件；若希望探测本身继续识别截断，那是“维持现状”，不是 A。

实现（`afd4339`）：

- `files::read_application_id` 打开探测连接后设 `PRAGMA writable_schema = ON`（连接只读且 immutable，这个标志改不了任何东西）。`check_library` 的文档注释改写。随后经 Develata 同意删除 `SETTLE` 的 2 秒重试（`a8b7693`）：探测只读第 1 页文件头，SQLite 读应用 id 前检查的字段在新旧第 1 页里都一样，超前的页数又被读过去，已没有能构造出的状态走到重试；探测报损坏时立即按损坏拒绝。
- `a_probe_that_meets_a_checkpoint_waits_for_it` 改为 `a_probe_knows_a_library_left_mid_checkpoint`：主文件少最后一页时认作资料库，文件不变、不建任何旁边的文件；同样少一页但应用 id 是别的程序的，仍按“不是资料库”拒绝。
- 新增 `a_library_left_mid_checkpoint_opens_and_recovers_from_its_log`：用存储写 20 行、保存日志、关闭，再造出“少最后一页 + 日志”；`LibraryStore::open` 成功、20 行齐全，关闭后日志已写回、文件长度复原、`integrity_check` 为 ok。同一个短文件不带日志时被拒绝为 `Corrupt`，文件不变，不留 `-wal`/`-shm`。
- 如果编译 SQLite 时默认打开了 `SQLITE_DBCONFIG_DEFENSIVE`，`writable_schema` 无效，这两个测试会失败，而不是静默退回旧行为。

## 验证与验收

### 自动化验证

日志在会话暂存目录 `scratchpad/logs/`。

- Windows（rustc 1.99.0）：
  - `cargo fmt --all --check`：通过。
  - `cargo clippy --workspace --all-targets --locked -j 2 -- -D warnings`：通过。
  - `cargo test --workspace --locked -j 2`：764 passed / 0 failed / 2 ignored。
  - `cargo test -p rssr-infra --locked -j 2 --test 'test_library_*'`：27 个测试程序，172 passed / 0 failed / 1 ignored。
  - `cargo test -p rssr-infra --lib store::`：20 passed（含夹具的两个测试，现在断言写完后没有 `-wal`）。
  - `cargo test -p rssr-cli --test test_stdout_contract`：5 passed（新增 `the_library_is_closed_before_the_cli_exits`：成功与失败的命令之后都没有 `-wal`/`-shm`）。
  - `cargo check -p rssr-app --target wasm32-unknown-unknown --locked`：通过。
  - `cargo check -p rssr-app --target aarch64-linux-android --locked`（NDK 28.2 的 clang）：通过，无警告。
- WSL Debian（rustc 1.97.0，POSIX 锁）：
  - `cargo test -p rssr-infra --locked -j 2 --test 'test_library_*'`：27 个测试程序，174 passed / 0 failed / 2 ignored。
  - `cargo test -p rssr-infra --lib store::`：24 passed。
  - `rssr-web` 停止冒烟，新数据目录（3 次）：运行中有 `library.sqlite3-wal`/`-shm`；`kill -TERM` 后退出码 0，两者都已删除，日志依次为“rssr-web 已停止”“资料库已关闭”。空库的日志很小，这个冒烟区分不出有没有关闭（见变异检查 M7）。
  - `rssr-web` 停止冒烟，大日志（3 次，脚本 `scratchpad/wslsmoke/`）：先启停一次建库，再用 Python 以 `wal_autocheckpoint = 0` 写入 3000 行已删除的订阅（每行 8000 字节，不触发刷新），不关连接直接 `os._exit`，留下 24934272 字节的日志；然后启动 `rssr-web` 并 `kill -TERM`。修复后：3/3 主文件 24858624 字节、没有 `-wal`/`-shm`、`immutable=1` 探测读到应用 id、日志有“资料库已关闭”。
- 新测试：
  - `closing_finishes_queued_tasks_and_waits_for_every_connection`：写连接被占住、另有一个任务排在后面时关闭；300 毫秒后 `close` 仍未返回；之后的读写得到 `Closed`；放开后两个任务都正常结束，`close` 返回时 `-wal`/`-shm` 已删除；再关一次返回 `Ok`；重新打开有 2 行。
  - `a_connection_that_cannot_be_closed_is_reported`：任务泄漏一条语句，`close` 返回 `Busy` 而不是假装关好。
- 变异检查（提交 `2c5c268` 之后逐个改回旧行为，补丁与日志在 `scratchpad/mut/`）：
  - M1 关闭时不等工作线程：`closing_finishes_…` 在“close waits for the running task and the queued one”失败。
  - M2 关闭时不取走发送端：同一测试在“之后的写入得到 `Closed`”的断言失败（排队 10 秒后得到 `ConnectionTimeout`）。
  - M3 关闭后提交返回 `TaskFailed`：同一断言失败。
  - M4 工作线程不显式关闭连接（只靠 `Drop`）：`a_connection_that_cannot_be_closed_is_reported` 得到 `Ok(())`，失败。
  - M5 CLI 只丢弃不关闭：`the_library_is_closed_before_the_cli_exits` 在“after a command that succeeded”失败（`-wal`/`-shm` 还在）。
  - M6 夹具只丢弃不关闭：`every_seed_becomes_a_library` 在“the log is written back”失败。
  - M7 `rssr-web` 正常停止时不关闭资料库：新数据目录的冒烟 3/3 仍通过（存活，原因见上）；大日志冒烟 3/3 失败：主文件停在 98304 / 266240 / 483328 字节，24934272 字节的日志原样留着，`immutable=1` 探测报“database disk image is malformed”——也就是 2026-10-20 的现象在 `rssr-web` 上的确定性复现。

- 第二部分（提交 `afd4339` 前后）：
  - Windows：`cargo fmt --all --check`、`cargo clippy -p rssr-infra --all-targets --locked -j 2 -- -D warnings` 通过；`cargo test -p rssr-infra --test 'test_library_*'`：27 个测试程序，173 passed / 0 failed / 1 ignored；`cargo test -p rssr-infra --lib`：113 passed。
  - WSL：`--test 'test_library_*'`：27 个测试程序，175 passed / 0 failed / 2 ignored；`--lib store::`：24 passed。
  - 变异 M8（去掉 `writable_schema`）：两个新测试都在自己的断言上失败（“still a library”与“opens”，都得到 `Corrupt`）。
  - 端到端（WSL，脚本 `scratchpad/wslsmoke/web_recover.sh`）：用不关闭资料库的 `rssr-web`（M7）造出 3 个真实的检查点中途的库（主文件 2039808 / 950272 / 1646592 字节，日志 24934272 字节）；修复后的 `rssr-web` 3/3 正常启动、没有“打开资料库失败”，停止后主文件 24858624 字节、3000 行、`integrity_check` 为 ok。

- 删除重试（`a8b7693`）：
  - Windows：fmt、`clippy -p rssr-infra` 通过；`files::tests::probes_accept_a_library_while_it_is_checkpointed`（另一连接反复写入、检查点、`VACUUM` 时不停探测）单独连跑 5 次再 3 次都通过；`--lib` 113 passed；`--test 'test_library_*'` 27 个程序 173 passed / 0 failed；`--test test_library_store` 28 passed。
  - WSL：同一压力测试连跑 5 次通过；`--test 'test_library_*'` 175 passed / 0 failed / 2 ignored；`--lib store::` 24 passed。
  - 对照：同时去掉 `writable_schema` 与重试，压力测试在 Windows 3/3、WSL 5/5 失败（“a library being checkpointed is still a library”，`Corrupt`）。也就是说重试过去只覆盖探测撞上进行中的检查点这一种情况，现在由 `writable_schema` 处理。
  - 过程中的失误：做对照时用 `git checkout -- files.rs` 还原，把当时还没提交的删除重试一并还原掉了；随后重新应用同一份改动，在这份树上重跑上面的 Windows 检查后才提交。

### 手工验收

- 桌面端关窗后的文件状态：未执行（`LoopDestroyed` 钩子只经编译与代码阅读确认；dioxus-desktop 0.7.9 的 `launch.rs` 先调用自定义处理函数，再处理 `LoopDestroyed`）。

## 结果

- 可合并：正常退出的程序不再截断关闭时的检查点；被截断的（强杀、崩溃、断电、Android 系统结束进程、自动检查点被打断）也不再让共用数据目录的程序拒绝资料库，SQLite 在下次打开时用日志恢复。

## 风险与后续事项

- `close` 会等已排队的任务；最坏情况受准入时限（默认 30 秒）与运行期限（写 60 秒）约束。`rssr-web` 只等 3 秒，超时后进程退出仍可能截断检查点；桌面端不设上限（窗口已经关闭）。
- 探测不再识别“文件比文件头短”，这件事交给写连接：日志丢失的截断文件仍被拒绝为损坏，但拒绝发生在写连接第一次读取时（此前 `remove_stale_scratch` 已在旁边建了惰性的 `.create-lock`，身份已确认）。
- 探测不再重试：如果将来出现别的、会让无锁读取暂时报损坏的情况，打开会立即失败（报“数据库文件已损坏”），而不是先等 2 秒。现有的压力测试覆盖了检查点与 `VACUUM`。
- CLI 被 Ctrl-C 中断时不关闭资料库（与之前相同）。

## 给下一位 Agent 的备注

- 入口：`crates/rssr-infra/src/store/executor.rs` 模块注释的“Closing”一段、`LibraryStore::close`；`crates/rssr-infra/src/store/files.rs` 的 `check_library`。
- 原型测试（第二部分的候选对比）在会话暂存目录 `scratchpad/proto/zz_probe_prototype.rs`，没有提交；它的状态构造已改写进上面两个测试。

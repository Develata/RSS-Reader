# Web 端刷新落盘合并为整轮一次

- 日期：2026-07-27
- 作者 / Agent：Claude Code
- 分支：main
- 当前 HEAD：57753d1
- 相关 commit：57753d1
- 相关 tag / release：v0.1.12
- 状态：`released`

## 工作摘要

Web 端「刷新全部」此前每提交一个订阅就把**整个库**（全部订阅 + 全部条目索引 + 全部标记 + 全部正文）重新序列化写回 `localStorage` 一次，且是主线程同步写。一轮刷新 N 个订阅就是 N 次全库重写，订阅一多刷新期间页面直接卡住。本次把整轮刷新收进一个写入批次，落盘压成整轮一次。

## 影响范围

- 模块：
  - `crates/rssr-application/src/refresh_service.rs`（`RefreshStorePort` 新增一对默认空实现的方法；`refresh_all` 重构）
  - `crates/rssr-infra/src/application_adapters/browser/adapters/refresh.rs`（浏览器存储实现批次）
  - `crates/rssr-infra/tests/wasm_refresh_contract_harness.rs`（新增 4 条契约测试）
- 平台：
  - Web（行为改变）
  - Windows / macOS / Linux / Android（**行为不变**，见下）
- 额外影响：
  - 无迁移、无配置变化、无 UI 变化

## 关键变更

### `RefreshStorePort` 新增批次协议

```rust
async fn begin_batch(&self) -> Result<()> { Ok(()) }
async fn end_batch(&self) -> Result<()> { Ok(()) }
```

两者都带默认空实现，因此 SQLite 存储、`subscription_workflow` 里的桩、契约 harness 里的桩全部不需要改动一行。

语义：批次期间的 `commit` 只需保证改动对后续读取可见，真正落盘可以推迟到 `end_batch`。事务型存储（SQLite）每次 `commit` 就已经落盘，没有可推迟的东西，所以默认实现是空的；需要整片重写的存储（浏览器 `localStorage`）才靠它省下那些重复写。

**不变量**：只与 `end_batch` 成对出现，且只在 `refresh_all` 里成对出现。重复打开必须安全——实现方要先把上一个没关掉的批次落盘。

### `refresh_all` 重构与批次守卫

把原来 cfg 分叉的循环体整体抽成 `refresh_targets(targets, input) -> Vec<RefreshFeedOutcome>`（wasm 串行 / 原生 `JoinSet` 并发的代码逐字搬运，未改逻辑；并发分支再拆成 `refresh_targets_concurrent`），`refresh_all` 变成：

```rust
let targets = self.store.list_targets().await.context("读取订阅列表失败")?;

let begun = self.store.begin_batch().await;
let _batch = RefreshBatchGuard { store: self.store.as_ref() };
begun.context("打开刷新写入批次失败")?;

let mut outcomes = self.refresh_targets(targets, input).await;
self.end_batch_or_mark_failed(&mut outcomes).await;
Ok(RefreshAllOutcome { feeds: outcomes })
```

**`RefreshBatchGuard` 是这次改动的关键，不是可选的加固。** 初版只靠「begin 与 end 之间不写 `?`」来保证批次一定收尾，这个论证是错的：Web 端刷新任务是 Dioxus 作用域绑定的 `spawn`（`rssr-app` 的 `spawn_projected_ui_command` → `ui/helpers.rs`），用户在刷新途中离开订阅页，组件一卸载整个 future 就被 drop，`end_batch` 永远等不到。批次会永久停在打开状态，此后**批次外**的提交（单订阅刷新、添加订阅的首刷）全都只改内存不落盘，而且不报错——用户看到的是「刷新按钮没反应」，刷新页面后新订阅条目全空。这不是假想路径，是当前 UI 上一次普通导航就能触发的。

守卫在 `Drop` 里调用同步的 `abort_batch`，因此 future 被取消时也一定收尾。它刻意**不设「解除」开关**：那会留出「已解除但 `end_batch` 尚未完成」的取消窗口。代价是正常路径上守卫析构会多调一次 `abort_batch`，因此该方法的契约里明确要求幂等。

`begin_batch` 的结果也刻意在守卫建立**之后**才 `?`：那次调用即使失败，实现方也可能已经把批次置为打开。

### `end_batch` 失败的语义

`end_batch_into_outcomes` 在落盘失败时把本轮所有非 `Failed` 的结果改写成 `Failed`，并带上真正的写入错误。理由：

- 抓取确实成功了，但结果没写进存储——重开页面时这些条目并不存在，报成功是在骗人。
- 下游会照着这份结果去做正文图片本地化。
- 这与改动前的行为一致：那时某个 feed 的 `commit` 写入失败，同样会让该 feed 报 `Failed`。

整轮仍然返回 `Ok`，不把 per-feed 结果丢掉——这是 `refresh_all` 原有的不变量。

### 浏览器存储实现

`BrowserRefreshStore` 增加 `Arc<RefreshWriteBatch>`（两个 `AtomicBool`：`active` / `dirty`）：

- `commit`：照旧改内存；**批次内**只标脏后返回，批次外仍然立刻 `save_state_snapshot`。标脏发生在**动内存之前**——`RefreshCommit::Updated` 会先改完 feed 元数据再走 `upsert_entries`，后者失败时带 `?` 返回，把标脏留在末尾就会漏掉这份已经改了的元数据。
- `end_batch`：清 `active`，`flush_if_dirty()`。
- `abort_batch`（同步，供守卫析构调用）：清 `active`，尽力冲刷，失败只能记日志——`Drop` 里没有地方上报错误。幂等。
- `begin_batch`：先冲掉上一个没关掉的批次，然后**无条件**置 `active`。冲刷失败也照样开张：带着 `?` 返回而把 `active` 留在上一轮的 `true`，会让存储停在吞写状态，而调用方只看到一个「开批次失败」的错误，根本察觉不到后续提交都被吃了。
- `flush_if_dirty`：写失败时把脏标记**还原**。清标记发生在写之前，不还原的话这批尚未落盘的改动就再也不会被重试。
- 空批次不写：整轮所有订阅都返回 304 是常态，那种情况下不该白白整片重写一次全库。

只用 `AtomicBool` 而没有再加一把锁：该适配器只在 wasm 上编译，浏览器单线程执行，不存在真正的竞争，原子量只是为了能在 `&self` 上改。批次状态放在 `Arc` 里是因为本类型是 `Clone` 的，克隆副本必须与原件共享同一个批次。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`：通过
- `cargo clippy --workspace --all-targets -- -D warnings`：通过
- `cargo test --workspace`：通过（238 passed / 0 failed；较上次 +3，为新增的 application 层测试）
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- wasm 浏览器契约 harness（Chrome 150 headless，本机实跑）：
  - `wasm_refresh_contract_harness`：19 passed / 0 failed
  - `wasm_subscription_contract_harness`：3 passed / 0 failed
  - `wasm_config_exchange_contract_harness`：3 passed / 0 failed
- `cargo check -p rssr-app --target aarch64-linux-android`：**未执行**（本机无 NDK，`cc-rs: failed to find tool "clang.exe"`）。本次未触碰移动端相关代码，交由 CI 的 `android-smoke` 覆盖。

### 新增测试

application 层（`refresh_service.rs`）：

- `refresh_all_wraps_the_whole_round_in_a_single_write_batch`——断言调用序列恰好是 `begin, commit:1, commit:2, commit:3, end, abort`。批次要是退化成「每个订阅一对 begin/end」，写入次数一次没少，这条会红；末尾的 `abort` 来自守卫析构。
- `a_failed_batch_write_turns_the_whole_round_into_failures`——`end_batch` 报错时每个 feed 都必须是 `Failed` 且带上真正的错误原因。
- `a_cancelled_round_still_closes_the_batch`——用一个永不返回的 source 加 `tokio::time::timeout` 制造真实的 future 取消，断言调用序列是 `begin, abort`。这条直接钉住上面那个「离开订阅页导致静默丢写」的回归。

浏览器契约层（`wasm_refresh_contract_harness.rs`，真实浏览器内跑）：

- `browser_refresh_store_defers_the_write_until_the_batch_ends`——两半都断言：批次内改动**立刻对内存可见**（页面读的是同一份共享状态，推迟落盘不能让刷新中途的界面读到旧值），但 `localStorage` **尚未**写入；`end_batch` 后才写入。
- `browser_refresh_store_batch_without_commits_writes_nothing`
- `browser_refresh_store_reopening_a_batch_flushes_the_unclosed_one`——安全网那条。
- `browser_refresh_store_commit_outside_a_batch_still_writes_immediately`——这条是「本次改动不影响单订阅刷新与添加订阅」的依据。
- `browser_refresh_store_abort_flushes_and_restores_immediate_writes`——批次被中断后既要尽力落盘，**也要恢复到「批次外立刻写」的状态**。后半句才是那个静默丢写回归的直接断言。
- `browser_refresh_store_abort_after_end_batch_writes_nothing`——`abort_batch` 幂等，守卫无解除开关的前提。

### 手工验收

Web 部署态实跑：`dx bundle --platform web --release` 出包，`rssr-web` 托管，Chrome 实际操作，三个真实订阅（Rust Blog / This Week in Rust 成功，Mozilla Blog 因 CORS 失败——正好构成一轮混合结果）。

- **写入次数实测**：在 wasm 模块加载**之前**用 `initScript` 钩住 `Storage.prototype.setItem`，一轮三订阅的「刷新全部」记录到 **4 次 `setItem`、628,949 字节**，即整轮恰好一次全量快照。改动前同样一轮是 3 次提交 × 4 个 key = 12 次、约 1.9 MB。按订阅数线性放大。
- **中断轮次后仍能落盘**（本次最关键的回归点）：点「刷新全部」后 120 ms 切到文章页（订阅页组件卸载 → Dioxus 丢弃刷新任务 → future 被取消），再回到订阅页刷新单个订阅——`last_fetched_at` 从 `09:32:08` 前进到 `09:32:57`，确实写进了 `localStorage`。没有批次守卫时这次写入会被静默吞掉。
- 添加订阅 + 首刷（批次外路径）：通过，10 条条目落盘。
- 混合轮次：失败订阅的 `fetch_error` 与成功订阅的时间戳都正确落盘。
- 硬刷新后：3 个订阅、14 条条目、14 份正文、失败记录全部完好。
- 同一轮顺带验收了另一个待发布提交（shell prefs）：搜索词 `rust` 与导航收起状态都跨刷新保持。

- 桌面端实跑：未执行。桌面端走 SQLite 存储，三个新方法都是默认空实现，`refresh_targets` 的并发分支代码逐字搬运未改；行为不变由 `cargo test --workspace` 与 CI 覆盖。

### 踩到的坑（供下一位参考）

第一次做写入次数测量时拿到「0 次写入」，但界面时间戳明明前进了。**没有直接采信这个数字**，而是去读 `localStorage` 里的持久化状态做交叉验证，发现存储其实已经更新——是钩子没生效：`localStorage.setItem = fn` 对 `Storage` 这种 exotic 对象会变成存一个名为 `setItem` 的**存储项**而不是覆盖方法；改钩 `Storage.prototype.setItem` 后 JS 侧生效了，但 wasm-bindgen 的胶水在模块初始化时就已经拿到了方法引用，仍然抓不到。最终靠 `navigate_page` 的 `initScript` 在模块加载前打补丁才测准。若当时采信那个 0，就会去追一个根本不存在的 bug。

## 结果

- 已随 `v0.1.12` 发布。
- 用户影响：仅 Web 端。刷新期间的主线程写入从「订阅数 × 全库」降到「整轮 1 次全库」（实测 3 订阅一轮 4 次 `setItem` / 629 KB，改动前为 12 次 / ~1.9 MB），且整轮全 304 时降到 0 次。桌面 / Android / CLI 行为逐字节不变。
- 代价：落盘时机从「每个订阅提交后」推迟到「整轮结束」。刷新中途关标签页会丢掉本轮已抓取但未落盘的条目。RSS 抓取是幂等的，下一轮刷新会重新取回，因此实际损失是「多刷一次」而不是数据损坏。这一取舍是本次改动唯一的负面影响，已与用户确认接受。

## 风险与后续事项

- **批次泄漏**：已由 `RefreshBatchGuard` 兜住（取消、`?` 早退、`begin_batch` 自身失败三条路径都覆盖）。仍然只有 `refresh_all` 一个调用点；将来若新增调用点，同样必须用守卫而不是手写 begin/end 配对。
- **刷新任务本身仍会被取消**：守卫保证的是「批次一定收尾、已抓到的部分尽力落盘」，不是「刷新会继续跑完」。Web 端在刷新途中离开订阅页，剩余订阅这一轮就不刷了。要改这一点得把 `spawn_projected_ui_command` 换成不随组件卸载而取消的任务，那是**用户可感知的行为变化**（刷新在后台继续），超出本次范围，未做。
- **跨标签页**：`save_state_snapshot` 一直是整片覆盖，多标签页并存时后写的覆盖先写的。这是既有问题（见 `browser/state/storage.rs:73-76` 的说明），本次未改变其性质——只是写入频率降低了。真要支持多标签页应走 `storage` 事件做同步。
- **仍可继续优化（本次刻意未做）**：`save_state_snapshot` 无条件写四个 key，但一次刷新提交只可能动到 `core` 与 `entry_content`，`app_state` / `entry_flags` 一定没动。按脏 slice 分粒度写还能再省一截。批次已经把写入次数压到 1，这一项的边际收益变小，故未纳入本次范围。
- **未纳入本次范围（用户明确拒绝）**：Web 刷新网络并发化、文章列表 SQL 分页、目录展开态双写。前者的收益在批次落地后已大幅缩水（主线程序列化才是瓶颈，不是网络串行）；后两者用户判断负面影响过大，不做。

## 给下一位 Agent 的备注

- 入口文件：`crates/rssr-application/src/refresh_service.rs` 的 `RefreshStorePort` 与 `refresh_all`；浏览器侧看 `crates/rssr-infra/src/application_adapters/browser/adapters/refresh.rs` 的 `RefreshWriteBatch`。
- 想弄清「为什么 Web 刷新会卡」，直接读 `crates/rssr-infra/src/application_adapters/browser/state/storage.rs:55-61`——`save_state_snapshot` 一次写四个 key，其中 `entry_content` 是全部正文。
- 本地跑 wash harness：`scripts/run_wasm_contract_harness.sh` 是按 Linux CI 写的（用 `google-chrome` 与 Unix 风格的 `--user-data-dir`），在 Windows 上会 404。绕法是自己在 `crates/rssr-infra/` 下写一份 `webdriver.json`（`binary` 用正斜杠路径、去掉 `--user-data-dir`），再直接调 `wasm-bindgen-test-runner <artifact>`；chromedriver 主版本必须与本机 Chrome 一致。
- 本机内存吃紧：`cargo test --workspace` 全量重建时并行度太高会 OOM（`rustc-LLVM ERROR: out of memory` / `LNK1102: 内存不足`），并留下截断的 rlib 让后续构建报一堆 "required to be available in rlib format"。遇到就 `rm -rf target/debug` 后加 `-j 2` 重跑。

# 刷新响应上限与重复正文写入优化

- 日期：2026-09-26
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：b43bad0（本轮起点，工作树干净）
- 相关 commit：本记录所在提交（`fix(refresh): bound feed responses and skip unchanged content writes`）
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

继续上轮建议：给原生 feed 刷新补齐响应大小上限，并降低 Web 在重复取得相同正文时的整片写入成本。改动留在 infra，正文时间戳规则在 SQLite 与 Web 保持一致；保留上一轮的 Web Locks、最新状态合并、逐订阅持久化和单一提交头协议。

## 影响范围

- 模块：infra HTTP 响应读取、SQLite 正文 upsert、browser refresh/state、契约与浏览器验收。
- 平台：桌面、Android、CLI 使用原生路径；Web 使用浏览器路径；rssr-web 代理已有 8 MiB 流限制，无需修改。
- 文档：README、用户指南、前端命令参考、CLAUDE 架构说明及本记录。
- 本轮未安装依赖，未修改数据库 schema、存储格式、UI 布局、主题选择器或配置。

## 关键变更

### 文件与职责

| 文件 | 变更 |
| --- | --- |
| `crates/rssr-infra/src/feed_body.rs` | 集中响应流累计、8 MiB 限制及字符集/BOM 解码，限制解压后的字节与转码后的文本。 |
| `crates/rssr-infra/src/lib.rs` | 注册跨平台私有响应读取模块。 |
| `crates/rssr-infra/src/fetch/client/feed_http.rs` | 原生刷新使用有界读取，逐请求保证 30 秒总超时。 |
| `crates/rssr-infra/src/subscription_probe.rs` | 添加订阅复用同一读取实现，保留原有公开上限常量。 |
| `crates/rssr-infra/src/application_adapters/browser/adapters/refresh.rs` | Web 刷新使用有界读取；正文没有变化时不标记 CONTENT 写入。 |
| `crates/rssr-infra/src/application_adapters/browser/state/entries.rs` | 逐字段比较合并结果与哈希，报告真实正文变化，移动已拥有的字符串。 |
| `crates/rssr-infra/src/application_adapters/browser/state.rs` | 导出浏览器 upsert 结果类型。 |
| `crates/rssr-infra/src/db/entry_repository.rs` | SQLite 冲突更新仅在正文记录实际变化时执行。 |
| `crates/rssr-domain/src/entry.rs` | 注释明确正文记录更新时间区别于抓取时间；无平台逻辑。 |
| `crates/rssr-infra/tests/test_feed_response_limits.rs` | 本地 HTTP 验证边界、分块、gzip、字符集、正文超时与失败保留数据。 |
| `crates/rssr-infra/tests/fixtures/feed-response-oversized.gz` | 8175 字节的确定性 gzip 夹具，解压为 8 MiB + 1 字节；生成方法在测试注释中。 |
| `crates/rssr-infra/tests/support/refresh_content_cases.rs` | SQLite/Web 共用相同正文、部分字段、空字符串、同哈希不同内容、标题变化契约。 |
| `crates/rssr-infra/tests/test_application_refresh_store_adapter.rs` | 原生执行共享契约，并以拒绝 UPDATE 的触发器证明相同正文没有重写。 |
| `crates/rssr-infra/tests/wasm_refresh_contract_harness.rs` | 浏览器执行共享契约、响应流错误/上限验证及重复正文写入测量。 |
| `scripts/browser/rssr_refresh_cost_acceptance.cjs` | 真实 Chromium 操作刷新、重复、更新、超限、重试，检查阅读快照与窄屏。 |
| `README.md` | 说明刷新响应上限及相同正文免写的边界。 |
| `docs/user-guide.md` | 更新刷新错误、正文存储成本与恢复说明。 |
| `docs/design/frontend-command-reference.md` | 记录时间戳、响应失败和完整提交语义，说明没有新 UI 接口。 |
| `CLAUDE.md` | 修正旧的 Arc/Mutex 描述，记录现有 BrowserStore 协调与发布职责。 |
| 本交接记录 | 汇集实现、测量、验证与未覆盖边界。 |

### 契约

- 浏览器 helper：`upsert_entries(&mut BrowserState, i64, Vec<ParsedEntry>) -> anyhow::Result<EntryUpsertOutcome>`，此前返回 `Result<u64>`。
- 新浏览器结果类型：`EntryUpsertOutcome { pub inserted_count: u64, pub content_changed: bool }`；SQLite 同名既有类型保持原样。
- `SqliteEntryRepository::upsert_contents` 签名不变，返回实际新增/变化的正文记录数；相同记录返回 0。
- 两端 `EntryContent.updated_at` 在正文记录（包括哈希）变化时更新；订阅抓取时间、文章索引更新时间照常更新。缺失字段保留缓存，空字符串可替换；不只依赖哈希判断内容相同。
- `MAX_SUBSCRIPTION_BYTES` 公开名称、类型、8 MiB 数值均不变；内部共用 `MAX_FEED_RESPONSE_BYTES` 和 `read_feed_text`。
- application/domain trait、端口、ReaderEntrySnapshot、CLI 命令、data-*、SQLite schema 和 Web 存储格式均不变。

## 验证与验收

### 自动化验证

执行日志位于 `target/refresh-cost/`。使用 `CARGO_BUILD_JOBS=1`；wasm harness 通过现有 `target/integration-validation/run-harness.sh` 设置固定 chromedriver 与 wasm-bindgen 版本，然后执行表中实际脚本。

| 命令 | 结果 | 退出码 |
| --- | --- | --- |
| `cargo fmt --all --check` | 通过（含最终检查） | 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 最终通过 | 0 |
| `cargo test --workspace` | 311 passed、2 ignored | 0 |
| `cargo check -p rssr-app --target wasm32-unknown-unknown` | 通过 | 0 |
| `cargo clippy -p rssr-app --target wasm32-unknown-unknown -- -D warnings` | 通过 | 0 |
| `cargo check -p rssr-app --target aarch64-linux-android` | 加载既有 android-env.sh 后通过；不是设备验收 | 0 |
| `cargo clippy -p rssr-infra --tests --target wasm32-unknown-unknown -- -D warnings` | 通过 | 0 |
| `bash scripts/run_wasm_contract_harness.sh wasm_refresh_contract_harness` | 27 passed | 0 |
| `bash scripts/run_wasm_contract_harness.sh wasm_subscription_contract_harness` | 18 passed | 0 |
| `bash scripts/run_wasm_contract_harness.sh wasm_config_exchange_contract_harness` | 3 passed | 0 |
| `dx build --platform web --package rssr-app --locked --debug-symbols false` | 生成实际验收包 | 0 |
| `cargo test -p rssr-infra --test test_feed_response_limits --test test_application_refresh_store_adapter --test test_subscription_discovery` | 15 passed，包含正文停滞的 30 秒真实超时 | 0 |
| `cargo test -p rssr-infra --test test_feed_response_limits` | 最终测试辅助代码修改后复验，7 passed | 0 |

- 首轮 workspace clippy 退出 101：新增测试未使用 socket.read 的字节数，已改为检查实际读取，最终 clippy 退出 0；修改后的 HTTP 限制测试 7 项复验通过。
- `target/refresh-cost/validate.sh` 汇总进程退出 1，因为保留了首轮 clippy 失败；不将这个首次矩阵进程记为全部成功。最终复验分别记在 `clippy-final.log`、`limits-final.log`。
- dx 0.7.10 仍打印与项目 Dioxus 0.7.9 不一致的诊断，但实际构建退出 0；本轮未改工具版本。
- workspace 中 2 个 ignored 测量用例未主动运行；本轮单独执行并记录与刷新写入直接相关的 wasm 测量。

### 性能测量

同一 Chromium wasm debug harness，12 个订阅共 120 篇文章，每篇约 2 KiB HTML + 2 KiB text。先完成首次提交，再记录三轮相同 200 响应的提交，不含网络与解析。

| 实现 | 每轮 setItem | 每轮写入字符数 | 三轮耗时 ms |
| --- | --- | --- | --- |
| 修改前 | 36 | 6,836,000 / 6,836,166 / 6,835,920 | 396 / 370 / 332 |
| 修改后 | 24 | 646,695 / 646,748 / 646,722 | 120 / 110 / 84 |

该样本的字符写入量减少约 90.5%，平均提交耗时约 366 → 105 ms。数字是本机三轮测量，不是 FPS、生产性能承诺或所有刷新场景的加速比例；首次新增/正文真正变化仍会写正文片。日志：`repeated-before.log`、`repeated-after.log`。

### 手工 / 浏览器验收

平台：Linux 上真实 headless Chromium，经 Playwright 运行构建后的 Web 包，视口均为 360×800 / 1280×800。使用本地 SPA server 与受控 feed 响应，不依赖外站在线状态。

- `bun scripts/browser/rssr_refresh_cost_acceptance.cjs`：退出 0，两种宽度均通过首次入库、相同正文无 CONTENT 发布、变化正文正确写入且不增加篇数、8 MiB 超限失败保留正文/索引/标记/成功时间、恢复后重试清除错误。
- 同一脚本同时打开 Reader 标签：另一标签刷新期间正文保持旧快照，重新加载后读取新正文；两个宽度无横向溢出、无 pageerror。截图位于 `target/refresh-cost/browser/`，已检查手机截图。
- `bun scripts/browser/rssr_storage_consistency_acceptance.cjs`：退出 0，两种宽度的不同文章标记、同文章不同标记、提交头故障、重试、权威未读数均通过；截图位于 `target/refresh-cost/storage-browser/`。
- `bun scripts/browser/rssr_reading_position_acceptance.cjs`：退出 0，两种宽度的返回、键盘、顶部/底部位置恢复均通过；滚动与编辑期间扫描计数均为 0。脚本中的 nativeCaptureBridge 是 Web 上模拟事件桥接，不是 Android 设备验收。

## 结果

实现、编译矩阵、HTTP 复验及刷新/存储/阅读位置浏览器验收完成，随本记录在本地提交。提交包含上表 20 个文件；开始时工作树干净，没有任务外改动进入提交。

## 风险与后续事项

- 已验证与推断分开：流读取限制、写入跳过和测试样本测量有直接证据；对实际大库卡顿的改善幅度需要用户工作负载测量。
- 大量正文确实发生变化时，Web 仍全量序列化正文片；本轮没有引入分片迁移、异步延迟提交或另一套缓存索引。逐订阅成功后才报告新增数，失败不留下部分 Web 提交。
- 正文上限不是总进程内存配额，编码转换和解析仍可能有额外分配。超过 8 MiB 的源会明确失败；这是将已有添加订阅/代理限制补齐到刷新路径。
- 未运行：本轮原生 GUI 手工交互、Android 模拟器/实机、macOS/Windows 实机、Firefox/Safari；没有改 UI/host，原生共享逻辑用测试与 Android 编译验证，不把编译当作设备验收。
- 没有 push、tag、release 或远端发布；`.handoff/` 排除状态和历史工作区保留。

## 给下一位 Agent 的备注

- 入口：`feed_body.rs`、browser `state/entries.rs`、`db/entry_repository.rs::upsert_contents`。
- CodeGraph CLI 存在，但本仓库无 `.codegraph/` 索引，因此使用 rg 和源码；没有安装或建索引。
- 测量正文变化场景时必须保留完整提交语义，不能通过恢复上一轮移除的延迟全库 flush 减少写入次数。
- 新测试脚本使用现有 Playwright 安装和本地 SPA server；不要向已有 BrowserStore 背后直接写旧 localStorage 键来准备夹具。

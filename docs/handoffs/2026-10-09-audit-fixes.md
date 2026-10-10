# 2026-10 工程审计发现的修复

- 日期：2026-10-09
- 作者 / Agent：Claude（Opus 5.5）
- 分支：`audit/2026-10-09`（worktree `target/audit-20261009/worktree`）
- 当前 HEAD：见“相关 commit”的最后一个提交
- 相关 commit：
  - 第一批：`6f00aaa`、`5cf52f5`、`2e382ea`、`2a4857b`、`28f880d`、`5a0c402`、`bbf09b3`、`dc8ec81`、`861585a`
  - 第一轮独立评审的处理：`fbe9c8f`、`77a5f1a`、`062f335`、`1f197bc`、`ee1daa0`、`bb9e66a`、`782580e`
  - 第二批：`e10b99e`、`347890e`、`83e2136`、`abc2f8f`、`2d1bef5`、`0847630`、`e04484c`
  - 第二轮独立评审的处理：`e93b968`、`d1def2e`、`5286d30`、`69eedf2`、`2418c14`
  - 第三批：`f1592e7`（D3）、`307e31f`（浏览器模块单测真正执行）、`364df0d`（D1 配额提示）、`0313d20`（C2）
  - 第三轮独立评审的处理：`af3ae73`（F1/F2）、`d1b8782`（C1 派生正文）、`92eca6c`（A1 取消窗口）、`5648b32`（A2 测试）、`493a158`（A3 第二种完成顺序）
  - 第四轮独立评审的处理：`9317689`（F1 断开的校验照样记失败、会话先验签名）、`6692802`（A1 带内容的落库开始前就登记序号）、`6e3a106`（C1 NULL 源哈希的正文可写回）、`beb3d17`（A3 缺席条目的写入一律重查）、`f34955e`（D1 提示措辞）
  - 第五轮独立评审的处理：`f67ec42`（登录校验先取名额再派生任务；断开测试改为闸门控制；会话载荷用类型保证先验签名）
  - 第六轮独立评审的处理：`57f94f0`（名额已满时第一次 poll 即返回 `Busy` 的测试）
- 相关 tag / release：N/A
- 状态：`validated`（本地验证；未推送、未开 PR）

## 工作摘要

按 [工程审计报告（2026-10）](../engineering-audit-2026-10.md) 逐项修复已确认的缺陷，每项单独提交并配红→绿测试；审计时提交的 8 个 `#[ignore]` 红测试已全部解除忽略并通过。每批修复都送 `codex-reviewer`（gpt-6-astra，max）独立评审，意见逐条核实后修复，共六轮。进度总表见报告第 10 节。

## 影响范围

- 模块：
  - `rssr-domain`：`AppStateRepository` 端口（`save` 换成原子 `update`）
  - `rssr-application`：`refresh_service.rs`（按订阅串行、抓取序号）、`subscription_workflow.rs`、`app_state_service.rs`
  - `rssr-infra`：`feed_body.rs`、`feed_normalization.rs`、`xml_declaration.rs`、`fetch/client/feed_http.rs`、`subscription_probe.rs`、`composition.rs`、`config_sync/webdav.rs`、`html/reader.rs`、`html/live_display.rs`、`db/app_state_repository.rs`、`db/entry_repository.rs`、`fetch/client/body_asset_localizer.rs`、`application_adapters/{mutations.rs,non_refresh.rs,browser/adapters/app_state.rs,browser/state/entries.rs,browser/state/storage.rs}`
  - `rssr-app`：`pages/entries_page/*`、`pages/feeds_page/*`、`ui/runtime/feeds.rs`、`ui/shell.rs`、`bootstrap/native.rs`
  - `rssr-web`：`auth/*`、`main.rs`、`proxy.rs`
  - `rssr-cli/src/main.rs`
- 平台：Windows / macOS / Linux / Android / Web（解码、链接选择、正文消毒、srcset、AppState、刷新协调两端共用；登录限速只在 Web 部署服务）
- 额外影响：`docs/user-guide.md`、`docs/design/frontend-command-reference.md`、`docs/deployment/web.md`

## 关键变更

### 订阅解析与抓取

- B5：既无摘要也无正文的条目保留为仅索引条目（产品决策）。
- B3：订阅刷新、订阅发现、WebDAV 与 `rssr-web` 代抓统一发送 `RSS-Reader/<版本> (+仓库地址)`。
- B2：`read_feed_text` 只做一次解码（BOM → HTTP charset → XML 声明 → UTF-8，RFC 7303 §3.2）；`parse_feed_xml` 在交给 feed-rs 的 XML 文本前总是补一个 UTF-8 声明，quick-xml 据此锁定 UTF-8，原声明不再触发二次解码。分类规则与 feed-rs 的分派一致（第一个 `<` 或 `{`）。
- B4：站点地址只取 alternate（或 RSS 无 rel 的 `<link>`），没有时留空；条目地址优先 alternate。

### 刷新与正文

- A1：同一订阅的刷新在 `RefreshService` 内逐个进行（抓取与提交在同一把锁内）；`refresh_all` 拿到锁后重读目标并校验 generation；添加订阅的预抓取结果用激活前领取的抓取序号判断，已被更新的刷新取代时不再写入。只协调同一进程内。
- C1：`content_hash` 只标识源正文（标题、纯文本、HTML、文章地址各带长度前缀，带版本；地址变了，按旧地址解析的相对图片就不再对应）；本地化写回只替换正文、保留源哈希（按 `IS` 比较，旧版本留下的 NULL 哈希也能写回），并要求存储正文仍是本次本地化读到的那份（防止同一份源的两个本地化任务互相覆盖）；一张图都没换成本地副本时不写回；刷新在源哈希未变时保留存储的正文，后台本地化跳过已本地化的条目。
- C2：后台本地化按尝试计数：为一篇文章发过图片请求（不论成败、超时）计一次，正文已派生或根本没发请求（无图、超预算）的不计；每个订阅每轮最多 5 次尝试、180 秒；同时最多 2 个订阅，同一订阅已有排队或进行中的一轮时不再追加。
- A1（补充）：带来内容的落库在开始前就登记抓取序号，不论成功、失败还是被取消（SQLite 适配器分几步提交，中途出错时前面的步骤可能已经落库），更早的预抓取结果不能再覆盖它。

### 应用状态与列表

- A2：`AppStateRepository::update` 在一次原子读改写里应用修改（SQLite 单个 IMMEDIATE 事务，Web 一次 `store.update`），端口不再提供整份写入。
- A5：删除订阅、导入配置不再因应用状态无法解析而整体失败。
- A3：已读/收藏写入成功时若有在途列表查询，作废它并递增 `requery_revision` 重查；查询先于写入完成时（写入完成时条目已不在列表里，说明列表及附带的归档计数是写入前读的）也重查。
- A4：归档分界的“现在”只在挂载、手动刷新、批量已读与查询条件变化时采样。

### 正文消毒

- F3：`data:` 只在 `img`/`source` 的 `src`/`srcset` 且媒体类型为完整 `image/<subtype>` 时保留；`srcset` 逐候选检查；媒体类型按 Fetch 规则从含 query 的序列化 URL 中取。
- srcset 按 HTML 标准的描述符状态机切分并校验描述符，无效候选既不被选中也不保留。

### Web 部署服务

- F1：登录失败按 TCP 对端地址计数（信任代理时取 `X-Forwarded-For` 最右项）；成功登录过的浏览器持签名的已知设备标记，单独计数。计数不看用户名（否则只有真实用户名会被限速，泄露用户名是否存在）；普通客户端最多单独跟踪 4096 个，超出后新的客户端地址共用一份溢出计数，已知设备不受这个阈值限制。校验名额在派生任务之前以不等待的方式取得（满了直接拒绝，不派生任务），任务持有名额直到记完失败，所以同时存在的校验任务不超过名额数；客户端中途断开也会记下失败。
- F2（部分）：Argon2 在阻塞线程池里执行，对任何用户名都执行一次；名额由独立任务持有到校验与记账结束，客户端断开不会提前归还名额；已知设备另有一个名额，其他来源占满时仍能校验。会话 cookie 先验签名再读用户名与有效期（用户名只能从验签后构造的 `VerifiedPayload` 读取）。

### Web 本地存储

- D3：浏览器批量 upsert 每批建一次查找表（去重键、外部 ID 计数、旧 URL 身份、正文位置），O(k·N) 降为 O(N + k)；差分测试用旧的线性实现做对照。
- D1（部分）：`localStorage` 配额超限或存储受限时给出明确提示（`QuotaExceededError` / `NS_ERROR_DOM_QUOTA_REACHED`），不再只显示 `JsValue` 调试串。用量显示与 IndexedDB 迁移未做。
- 浏览器模块的单元测试改为 `#[wasm_bindgen_test]`：此前的 `#[test]` 在 wasm32 上从未执行。

### CLI 与界面

- H1：`remove-feed --keep-entries`。
- E1：JSON 配置导入替换设置后，由 App 作用域的任务把新设置写入全局主题；离开订阅页不再取消导入。

## 验证与验收

### 自动化验证

- 第一批边界（`861585a`）：`cargo test --workspace --locked` 378 passed / 0 failed / 7 ignored；三个浏览器契约 harness 29/29、20/20、3/3。
- 第一轮评审处理后的边界（`062f335`）：`cargo test --workspace --locked -j 2` 393 passed / 0 failed / 5 ignored；三个浏览器 harness 29/29、20/20、3/3；`cargo clippy --workspace --all-targets -- -D warnings`、`cargo clippy -p rssr-app --target wasm32-unknown-unknown -- -D warnings`、`cargo fmt --all --check` 通过。
- 第二批与第二轮处理：逐提交运行受影响 crate 的测试与 clippy（原生与 wasm），浏览器端改动跑对应 harness。
- 第三批与第三轮处理（逐提交，实测）：`rssr-web` 28 passed / 0 failed / 1 ignored；`rssr-application` 58 passed；`rssr-infra` lib 65 passed、`test_audit_20261009_refresh_invariants` 9/9、`test_feed_refresh_flow` 3/3、`test_native_outbound_requests` 14/14、`test_app_state_persistence` 1/1；`rssr-app` 的 bootstrap 测试 16/16、entries_page 45/45；`cargo clippy -- -D warnings` 原生与 wasm32 通过，`cargo fmt --all --check` 通过。
- 第三轮处理后的边界（`493a158`，实测）：`cargo test --workspace --locked -j 2` 412 passed / 0 failed / 3 ignored（40 个测试二进制，460 秒）；三个浏览器 harness 30/30、21/21、3/3（headless Chrome 154.0.8037.93 + ChromeDriver 154.0.8037.92 + wasm-bindgen-test-runner 0.2.126，均为本地固定版本，未改全局安装）；`rssr-infra` wasm lib 测试 14/14（node）；`cargo clippy --workspace --all-targets -- -D warnings`、`rssr-app`/`rssr-infra` wasm32 clippy、`cargo fmt --all --check` 通过。
- 第四轮处理（实测）：`rssr-web` 30 passed / 1 ignored；`rssr-application` 58；`rssr-infra` lib 65、`test_audit_20261009_refresh_invariants` 10/10、`test_feed_refresh_flow` 3/3、`test_app_state_persistence` 1/1；`rssr-app` 120 passed / 1 ignored；`wasm_refresh_contract_harness` 30/30；四个 crate 的原生 clippy 与 wasm32 clippy 通过。
- 第四轮处理后的边界（`f34955e`，实测）：`cargo test --workspace --locked -j 2` 415 passed / 0 failed / 3 ignored（40 个测试二进制）；三个浏览器 harness 30/30、21/21、3/3；`rssr-infra` wasm lib 测试 14/14；全工作区与 wasm32 clippy、`cargo fmt --all --check` 通过。
- 第五、六轮处理（`f67ec42`、`57f94f0`，只改 `rssr-web`，实测）：`rssr-web` 30 passed / 1 ignored，clippy 与 fmt 通过。
- 变异验证（实测，每次恢复后复跑通过）：C2 计数（把无图文章也计数 → 0 次请求）；F1/F2 三个测试（名额留在调用方、表满不记录、设备共用名额）；C1 三个测试（去掉正文条件、哈希忽略地址、仅规范化也写回）；A1（只在成功后登记）、A2（普通 `BEGIN`）、A3（忽略缺席条目）；第四轮的断开记账（在处理函数里校验与记账）、A1 部分写入（只在成功后登记）、NULL 哈希（`=` 比较）、A3 归档计数。第五轮的闸门测试（在请求里记账、提前释放名额，各自失败）；第六轮的满名额测试（在任务里取名额 → 第一次 poll 为 `Pending`，失败）。每个测试都在对应修复撤掉后失败。会话先验签名的顺序无法用行为测试观察，改由类型保证（`VerifiedPayload`），未做时序测量。
- 红→绿（实测）：8 个审计红测试解除忽略后通过；A2 在 SQLite（改成延迟事务即失败）与浏览器（改成先读后写即丢更新）两端各有一个红测试；E1 卸载测试在任务归页面作用域时失败；srcset、`data:` 媒体类型、声明固定等用例在修复前的实现上失败。

### 手工验收

- 未执行（无布局、焦点或导航改动；界面行为由 VirtualDom 会话测试覆盖）。

## 结果

- 三批修复与五轮独立评审意见均已处理并提交。第四、五轮结论均为 PASS / no blocker：第四轮的 4 项 SHOULD_FIX 与 2 项可执行的 NICE_TO_HAVE 已修复，其余写入下方残余风险；第五轮确认第四轮的代码缺陷全部关闭，余下 2 项测试问题与 1 项任务数量上限问题由 `f67ec42` 处理；第六轮确认生产代码无问题，只提出 3 项回归保护不足，其中满名额测试已补，另两项见下方残余风险。到此为止没有发现值得再投入的代码缺陷，评审循环停止。
- 过程记录：中途本机 C 盘被另一个会话的失控后台输出（24 GB）占满、提交内存耗尽，构建以 os error 1455 失败；经你同意删除该输出文件后恢复。为此删除过本审计自建的重复构建缓存 `target/audit-20261009/target`（18 GB，可再生），之后构建的临时文件放在 E 盘。

## 风险与后续事项

- B4：没有稳定 ID 的条目若所选链接因新规则改变，去重键变化，下一次刷新出现一次重复。
- B2：只识别 ASCII 兼容字节流中的 XML 声明；无 BOM 的 UTF-16/UTF-32 不做探测。
- A1：只协调同一进程内的刷新；CLI 与图形界面同时刷新同一个数据库、或 Web 多个标签页仍可能交错。两端的刷新都逐订阅落盘（浏览器端用默认的空批次钩子，代价见 D2）。带内容的落库一开始就登记序号：若那次落库其实什么也没写（例如一开始就因数据库被锁而失败、或在写入前被取消），添加订阅时预抓取的首刷结果会被丢弃，下一次刷新重新抓取。
- C1：升级后每个订阅下一次刷新会因哈希算法变化重写一次当前 feed 中条目的正文（本地化正文此前每次刷新都会被冲掉，这里只是最后一次）。条目地址在某次刷新中缺失时（存储沿用旧地址），源哈希也会变化，派生正文被重算一次。
- C2：尝试窗口按 feed 顺序：图片持续失败的文章每轮都会占用尝试次数，直到源正文变化，排在后面的文章可能一直轮不到后台本地化（阅读时的按需本地化不受影响）。部分图片成功的文章算作已本地化，后台不再补其余图片；打开阅读页时按需补，但内联图片会让正文变大，超过阅读页的 1.5 MiB 预算后按需本地化直接跳过，剩余图片可能一直不补。排队中的旧条目列表不会被新一轮替换，被丢掉的新条目要等下一次带回正文的刷新（返回 304 的刷新不会触发本地化）。“只把新增或正文变化的条目送去本地化”未做。
- D1：配额满时只给出提示；正文仍在 `localStorage` 一片里，接近上限时每次带正文变化的刷新都会失败。删除订阅也要先发布剩余正文的新副本，空间非常紧张时同样会失败，没有可靠的应用内恢复手段；需要导出配置后清理站点数据，或等 IndexedDB 迁移。
- E1：导入读回设置与设置页保存极端交错时（读回完成到 App 任务被轮询之间有保存落地），全局主题可能停在导入时的值，直到之后重新加载设置（打开设置页）或重启；未加版本校验。
- F1：没有已知设备标记的新浏览器在攻击期间仍会被限速；反向代理后未开启信任代理头时，所有新客户端共用一份计数；计数表满时所有未单独计数的新客户端共用溢出计数，可能被一起限速。被盗的设备标记可以重放并耗尽该设备自己的计数。登录入口的用户名时序差已消除；其余路径未做时序测量。两项回归保护刻意未做：会话“先验签名再读用户名”的顺序只由代码结构保证，没有测试能在顺序被改回时失败（行为上无法区分，需要往生产代码里加观察点）；“名额一直持有到记完失败”只在校验期间有确定性测试，记账那几微秒的窗口没有。
- 待办：B6 单独一轮；G1（迁移校验和）、“内网背景图”与 D6（Unicode 折叠，依赖 G1 的迁移决定）需要你确认；D2、D4、D5、D7、E2、E3、G2–G6、H2、H3、B7、F2 其余部分（`/logout` 用 GET、CSP）未处理。

## 给下一位 Agent 的备注

- 入口：报告第 10 节进度表；每项修复的提交信息写明了机制、取舍与测试。
- 在本机复跑浏览器验证：CI 用的 wasm-bindgen 是 0.2.126，全局装的是 0.2.128；Scoop 的 `chromedriver` 默认是 155，而本机 Chrome 是 154（版本不配时 harness 报 `http status: 404`）。复跑时不要改全局安装，按本次的方式临时指定：
  - `RSSR_WASM_BINDGEN_TEST_RUNNER=<0.2.126 的 wasm-bindgen-test-runner.exe 绝对路径>`（本次用 `cargo binstall --root <临时目录> --version 0.2.126 wasm-bindgen-cli` 装在会话临时目录，会话结束后不保留，需要重装）；
  - `PATH` 前置 `~/scoop/apps/chromedriver/154.0.8037.92`；`CARGO_TARGET_DIR` 指向独立的 wasm 目标目录；然后 `bash scripts/run_wasm_contract_harness.sh <harness>`；
  - `rssr-infra` 的 wasm lib 测试：`CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=<同一个 runner> cargo test -p rssr-infra --lib --target wasm32-unknown-unknown --locked`。
- 独立评审原文在会话 scratchpad 的 `review/batch1-review.md`、`review/round2-review.md`、`review/round3-review.md`、`review/round4-review.md`、`review/round5-review.md`、`review/round6-review.md`（不入库）；处理结论记在本记录与报告第 10 节。

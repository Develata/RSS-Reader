# 2026-10 工程审计发现的修复

> 历史交接：本记录仅说明当时的实现与验收，旧路径和命令可能已退役。当前状态见[文档索引](../../../README.md)，归档范围见[历史索引](../README.md)。

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
  - 你确认后的处理：`38198b1`（G1 迁移校验和不计换行差异）、`0f2b2cf`（D6 两端按 Unicode 小写搜索）
  - 第七轮独立评审的处理：`be5a102`（折叠列的部分索引、过期值触发器与当场补回、`instr` 字面查找）、`ba69e79`（迁移锁路径按 sqlx 解析 URL）、`fbd54e8`（升级验收脚本预期 0008，未运行：需要官方 v0.1.21 包）
  - B6（自定义订阅名不被刷新覆盖，你已批准）：第一单元 `f7f28b4`（`50776c7` 文档）；第十轮独立评审（第一单元，PASS / no blocker）的处理 `d872fdc`；第二单元与 D5 `8a64904`；第十一轮独立评审（PASS / no blocker）的处理 `f48ef80`；第十二轮（PASS / no blocker，只剩两处文档措辞）的处理 `de62375`；之后经你同意下载官方 v0.1.21 包完成了升级验收与 B6 真实二进制对照（见“手工验收”）。你安排的 Astra 评审（`bdeeae4..0ebea13`）：PASS / no blocker，没有可行动缺陷，记录见 [2026-10-12-b6-astra-review.md](2026-10-12-b6-astra-review.md)。见下方“订阅名称”一节；第三单元（发布说明要点）在“风险与后续事项”。
  - 第八、九轮独立评审的处理：`420459f`（回填查询固定走部分索引；真实 0007 旧库升级与回填失败续跑测试）、`589d13c`（迁移锁按 SQLite 实际打开的文件路径）、`568e01f`（文档）、`fc520b7`（失败续跑测试改在第二批中途注入；升级测试核对原有字段不变）。第九轮结论：PASS / no blocker，余项见下方残余风险。
- 相关 tag / release：N/A
- 状态：`validated`（本地验证；未推送、未开 PR）

## 工作摘要

按 [工程审计报告（2026-10）](../../../engineering-audit-2026-10.md) 逐项修复已确认的缺陷，每项单独提交并配红→绿测试；审计时提交的 8 个 `#[ignore]` 红测试已全部解除忽略并通过。每批修复都送 `codex-reviewer`（gpt-6-astra，max）独立评审，意见逐条核实后修复，共六轮。进度总表见报告第 10 节。

## 影响范围

- 模块：
  - `rssr-domain`：`AppStateRepository` 端口（`save` 换成原子 `update`）；`Feed.source_title`、`NewFeedSubscription.source_title`、`OpmlFeed`（B6）
  - 迁移：`migrations/0008_entry_title_search.sql`（D6）、`migrations/0009_feed_custom_title.sql`（B6）
  - `rssr-application`：`refresh_service.rs`（按订阅串行、抓取序号）、`subscription_workflow.rs`、`app_state_service.rs`
  - `rssr-infra`：`feed_body.rs`、`feed_normalization.rs`、`xml_declaration.rs`、`fetch/client/feed_http.rs`、`subscription_probe.rs`、`composition.rs`、`config_sync/webdav.rs`、`html/reader.rs`、`html/live_display.rs`、`db/app_state_repository.rs`、`db/entry_repository.rs`、`fetch/client/body_asset_localizer.rs`、`opml.rs`、`db/feed_repository.rs`、`application_adapters/{mutations.rs,non_refresh.rs,browser/adapters/app_state.rs,browser/adapters/{config,feed,mutations}.rs,browser/config.rs,browser/query.rs,browser/state/{entries,models,storage}.rs}`
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

### 数据库迁移

- G1：迁移校验和核对不计换行差异。sqlx 按编译期嵌入的文件字节算校验和，Windows 检出是 CRLF、其余平台是 LF，原先 Windows 版建的库在 Linux/macOS 版上（或反过来）会以“迁移被改过”拒绝启动。现在库里记录的若是同一份 SQL 另一种换行（整体 LF 或整体 CRLF）的校验和，就按记录核对；内容真正不同的照旧拒绝；已有记录不改写，新应用的迁移按本次构建的换行记录。没有加 `.gitattributes`（保守取舍；换行由 Git 配置与属性共同决定，不只取决于操作系统）。

### 搜索

- D6：标题搜索两端都按 Unicode 小写比较（规则在 `rssr_domain::search`，两端共用）。原生端新增迁移 0008 的 `entries.title_search` 列，写入条目时一起写折叠后的标题，启动迁移后把旧条目分批回填（折叠在 Rust 里算，SQLite 的 `lower()` 只处理 ASCII）；搜索用 `instr` 在 `title_search` 里做字面查找，尚未回填的行退回 `instr(lower(title), …)`（只按 ASCII 不区分大小写）；回填查询显式走部分索引；旧版本改标题由触发器清空过期值，当前版本写入后当场补回。迁移 0008 在发布前原地改过：已应用旧 0008 的本地测试库需要重建。不做完整大小写折叠（`ß` 不等于 `ss`）。

### 订阅名称（B6）

- 两个名称分开保存：用户或导入指定的名称（原生新列 `feeds.custom_title`，迁移 0009；浏览器 `PersistedFeed.custom_title`）与订阅源自己声明的标题（沿用原来的 `title` 列 / 字段）。刷新、添加时的探测只写源标题；CLI `--title`、OPML 与配置包导入只写指定名称。
- 领域：`Feed.title` 是显示名称（指定名称优先，否则源标题，两者都没有时为 `None`；URL 只在订阅列表、文章列表、阅读页等显示边界兜底）；新增 `Feed.source_title`。`NewFeedSubscription.title` 是指定名称（`None` 保留、空白清除、其余设置，原生端也先去首尾空白，与浏览器一致）；新增 `NewFeedSubscription.source_title`（只补不清）。两条原生 upsert SQL 合并为一个函数。
- 读取：原生订阅列表排序、订阅摘要、文章列表的订阅名统一 `COALESCE(custom_title, title, url)`；浏览器用 `PersistedFeed::display_title` 做同样的兜底。
- 添加订阅：`add_prepared_subscription` 不再把探测到的标题填进用户指定的名称，改为作为源标题随激活一起写入（同一事务 / 同一次 `store.update`）。
- 导入：OPML 与配置包里每个订阅的名称和分组总是“设置或清除”，不再按导入前的订阅快照区分（已删除的订阅不在快照里，重新激活时会留下删除前的名称和分组）。分组的行为因此也变了：OPML 里没有分组的订阅，重新激活时清除旧分组。
- 迁移 0009 只加列，不把已有标题复制进 `custom_title`（你的决定：升级前的标题一律当作源标题，继续跟随订阅源改名）。
- Web 存储：提交头 `version` 升到 2，键名 `rssr-web-commit-v1` 与锁名不变。读到版本 1 时，在同一把锁内成功读出全部数据片后，只经唯一发布点把提交头换成 2（纪元、各数据片版本不变，数据片不重写；只读打开也会升级）；新建的存储直接是版本 2；未知版本拒绝读写且不改动任何键。只认版本 1 的旧标签页此后在任何读写之前被拒绝，不会把 `custom_title` 写丢。
- CLI `add-feed --title` 帮助、`docs/user-guide.md`、`specs/001-minimal-rss-reader/data-model.md` 说明了两种名称、导入的“设置或清除”、本应用导出再导入会把显示名称变成指定名称、升级前的名称不受保护。
- 第二单元（OPML，`8a64904`，第十一轮评审后由 `f48ef80` 修订）：按 OPML 2.0，`text` 是显示的名称、`title` 来自订阅源。导入取 `text`，空白时退回 `title`（分组同样），任何非空名称都按指定名称导入，不与订阅地址比较。导出 `text` 写显示名称，没有名称时写空串（OPML 要求有 `text`）；`title` 只在知道源标题时写；文档带 `<head/>`。属性值里的制表、换行、回车写成字符引用，XML 1.0 不允许的字符去掉（不让一个远端标题里的控制字符使整份导出失败）。`OpmlCodecPort::encode` 改收新的 `OpmlFeed`（地址、显示名称、源标题、分组）；配置包格式不变，所以 OPML 导出直接从订阅仓储取列表，不再经配置包导出。
- D5（随第二单元完成）：Web 端 OPML 委托共用的 `OpmlCodec`，删除了 Web 端那份更宽松的副本。两端在写入前整体拒绝：被截断或标签未闭合、根元素前后有多余内容、`<body>` 之外的 `outline`（原先两端都静默跳过，报告成功）、任何元素上 quick-xml 能识别的属性语法错误（缺等号或引号、重复属性；原先只检查 `opml` 与 `outline` 的属性）。不做完整的 XML 良构性校验：属性值里的裸 `<`、未定义的实体（只出现在不读取取值的元素上时）、两个属性之间缺空白仍会通过（第十二轮评审）。

### SQLite 引擎与 sqlx 0.9（新发现，你已同意先做）

- 讨论跨进程刷新协调时查到：SQLite 官方确认的 WAL-reset 缺陷影响 3.7.0–3.51.2，在 3.51.3 修复（另有 3.44.6、3.50.7 回移）。触发条件是同一个 WAL 库文件上两个以上连接恰好同时写入或做 checkpoint，可能损坏数据库，官方称概率很低（https://sqlite.org/wal.html#walreset）。原来捆绑的是 3.46.0（sqlx 0.8.6 只能用 libsqlite3-sys 0.30.x）；应用的连接池与并发刷新本来就满足触发条件，GUI 与 CLI 共用数据目录后会更常见。
- 处理：sqlx 0.8.6 → 0.9.0；`rssr-infra` 直接声明 `libsqlite3-sys = 0.37`（捆绑 SQLite 3.51.3，已在 rusqlite 仓库 `v0.39.0` 标签核对），按 sqlx 0.9 CHANGELOG 的建议用依赖声明固定版本。只靠锁文件会被意外回退：升级过程中锁文件就被同时运行的 IDE 改回过 0.30.1，是新加的版本守卫测试发现的。
- 代码改动：`runtime-tokio-rustls` 组合特性已删除，改为 `runtime-tokio`（SQLite 不需要 TLS）；`query*()` 只接受静态或显式审计的 SQL：文章已读/收藏标记原来在运行时拼列名，改成封闭枚举 `EntryFlag`，每种标记一条固定 SQL；两处测试里的动态 SQL 用 `AssertSqlSafe` 标注；`QueryBuilder` 不再带生命周期参数；`Migration::sql` 变成 `SqlStr`；G1 的换行容错改用 sqlx 0.9 的 `Migrator::with_migrations`，去掉了自写的 `MigrationSource` 包装。校验和算法未变（仍是 SQL 原文的 SHA-384，已在 sqlx-core 0.9.0 源码核对）。
- 新测试 `db::tests::bundled_sqlite_has_the_wal_reset_fix`：实际链接的 SQLite 低于 3.51.3 时失败（锁文件回退到 0.30.1 时它确实报出了 3.46.0）。
- 独立评审（PASS / no blocker）后的跟进：守卫测试的措辞改为“项目要求的 SQLite 基线是 3.51.3”（官方另有 3.44.6、3.50.7 回移修复）；发布产物的实际版本已直接核对：release 版候选 CLI 内嵌的 SQLite 源码标识为 `2026-03-13 10:38:09 737ae4a3…`（3.51.3），v0.1.23 官方 CLI 为 `2024-05-23 … 96c92aba…`（3.46.0）。评审还把捆绑的 `sqlite3.c` 与官方 3.51.3 发布的 SHA3-256 逐字节核对一致。
- Rust 工具链：经你同意把本机 stable 从 1.98.1 升到 1.99.0；Dockerfile 的构建镜像从 `rust:1.88-bookworm` 改为 `rust:1.99-bookworm`（Docker Hub 标签已确认存在；sqlx 0.9 要求 1.94）。Rust 1.99 下全量复验通过（见验证一节）；本机 Docker 守护进程未启动，镜像未实际构建。
- 未采用：sqlx 0.9 新增的“计算迁移校验和时忽略指定字符”配置会改变校验和，已有数据库里的记录对不上，不能替代 G1 的容错层。

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
- 第七轮处理后（`fbd54e8`，实测）：`cargo test --workspace --locked -j 2` 423 passed / 0 failed / 3 ignored；全工作区与 wasm32 clippy、fmt 通过；四项变异（去掉当场补回、去掉触发器、换回 LIKE、换回手写 URL 解析）各自让对应测试失败。浏览器端代码这一轮未改。
- G1 与 D6 之后的边界（`0f2b2cf` 的内容，提交前在工作区实测）：`cargo test --workspace --locked -j 2` 420 passed / 0 failed / 3 ignored；三个浏览器 harness 30/30、21/21、3/3；`rssr-infra` wasm lib 测试 14/14；全工作区与 wasm32 clippy、`cargo fmt --all --check` 通过。G1 的测试把索引库与正文库的每条记录改成另一种换行的校验和，去掉这层核对就以原始错误失败；D6 的测试把原生搜索结果与共用规则逐条对照，改回比较原标题就失败。
- B6 第一单元（提交前在工作区实测）：`cargo test --workspace --locked -j 2` 430 passed / 0 failed / 3 ignored；三个浏览器 harness 30/30、24/24（新增 3 个）、3/3；`rssr-infra` wasm lib 测试 14/14；全工作区与 wasm32 clippy、`cargo fmt --all --check` 通过。新增测试：两端共用的名称契约（`support/feed_title_cases.rs`，原生与浏览器各跑一遍：指定名称与源标题分开、刷新不覆盖、`None` 保留、空白清除、无名称时 URL 只在列表兜底、重新激活时清除旧名称、配置包设置或清除）、0008→0009 真实旧库升级（已有标题留作源标题并继续跟随刷新）、添加时探测标题只进源标题、OPML 导入不看快照、浏览器提交头 v1 原地升级与未知版本拒绝。预期改变的旧测试 4 个：`test_application_refresh_store_adapter` 两处（添加时带名称、刷新后原断言为源标题，改为名称保留、源标题单独核对）、`test_persistence_atomicity::deleted_feed_rejects_late_metadata_update`（名称改存 `custom_title`，源标题列保持为空）、`wasm_subscription_contract_harness` 的去重测试（持久化字段改为 `custom_title`）。变异（实测，均恢复）：原生显示名偏向源标题、探测标题填回指定名称、OPML 缺名称传 `None`、配置导入不写 `custom_title`、0009 复制旧标题，浏览器显示名偏向源标题、只读打开不升级提交头、接受未知版本，各自让对应测试失败。
- 第十轮独立评审（第一单元）的处理与第二单元（`d872fdc`、`8a64904` 的内容，提交前在工作区实测）：`cargo test --workspace --locked -j 2` 433 passed / 0 failed / 3 ignored；三个浏览器 harness 30/30、26/26、4/4；`rssr-infra` wasm lib 测试 14/14；全工作区与 wasm32 clippy、`cargo fmt --all --check` 通过。评审处理：迟到刷新的测试补查源标题（去掉元数据更新的 generation 条件后，迟到刷新确实写入了 "Feed stale"，现在被抓到）；浏览器提交头升级补两条失败路径（提交头写失败时一切不变、同一句柄下次访问重试升级；数据片损坏时不升级）；workflow 测试的桩按真实契约返回显示名称、改为核对激活端口收到的参数；CLI 帮助说明省略 `--title` 时保留已有名称。旧版本标签页的拒绝只能读旧版代码确认（它在读数据片之前就拒绝版本 1 以外的提交头），当前构建无法实际运行旧版。第二单元新增测试：`text` 优先与空白退回、与地址相同的名称、导出属性与往返、Web 端 OPML（此前没有任何 harness 覆盖：按 `text` 导入、导出两个名称、残缺文档在写入前拒绝）。现有 OPML 夹具的 `text` 与 `title` 都相同，改优先级没有翻转旧断言；预期改变的只有“OPML 导出取自配置包导出”这一个应用层测试（改为核对两个名称都传给编码器）。变异（实测，均恢复）：去掉 generation 条件、升级时重写数据片、读数据片之前先发布新提交头、探测标题填回指定名称、`title` 优先于 `text`、保留与地址相同的名称、导出 `title` 写显示名称、服务层丢掉源标题，各自让对应测试失败。
- 第十一轮独立评审的处理（`f48ef80`，实测）：`test_opml_interop` 7/7、`test_config_exchange_contract_harness` 9/9、`test_config_package_io`、`test_regression_smoke`、`rssr-application` 60/60、浏览器 config harness 4/4、原生与 wasm32 clippy、fmt 通过。上一条里“与地址相同的名称不算名称”的规则及其测试已在这次删除（改为无名称时导出空 `text`）。变异：去掉 `<body>` 外订阅的检查、去掉逐元素的属性检查、去掉换行的字符引用，各自让对应测试失败。
- sqlx 0.9 / SQLite 3.51.3（提交前在工作区实测）：`cargo test --workspace --locked -j 2` 436 passed / 0 failed / 3 ignored（新增版本守卫 1 个）；三个浏览器 harness 30/30、26/26、4/4；`rssr-infra` wasm lib 14/14（Chrome 已自动升级到 155，改用 Scoop 的 ChromeDriver 155.0.8059.39；临时目录里的 wasm-bindgen 0.2.126 测试运行器被删过一次，已重新安装到同一位置）；原生与 wasm32 clippy、`cargo fmt --all --check` 通过；`cargo check -p rssr-infra -p rssr-cli --target aarch64-linux-android`（本机 NDK 28.2，CI 用 27.3）通过，含为 Android 编译的捆绑 SQLite。release 版 CLI（SHA-256 `d09fd228…51e4f89a`）重跑升级验收：v0.1.23 → 候选 8/8、v0.1.21 → 候选 10/10。
- Rust 1.99.0（守卫措辞与 Dockerfile 改动后，实测）：`cargo test --workspace --locked -j 2` 436 passed / 0 failed / 3 ignored；三个浏览器 harness 30/30、26/26、4/4（ChromeDriver 155.0.8059.39）；`rssr-infra` wasm lib 14/14；原生与 wasm32 clippy、`cargo fmt --all --check` 通过。
- 变异验证（实测，每次恢复后复跑通过）：C2 计数（把无图文章也计数 → 0 次请求）；F1/F2 三个测试（名额留在调用方、表满不记录、设备共用名额）；C1 三个测试（去掉正文条件、哈希忽略地址、仅规范化也写回）；A1（只在成功后登记）、A2（普通 `BEGIN`）、A3（忽略缺席条目）；第四轮的断开记账（在处理函数里校验与记账）、A1 部分写入（只在成功后登记）、NULL 哈希（`=` 比较）、A3 归档计数。第五轮的闸门测试（在请求里记账、提前释放名额，各自失败）；第六轮的满名额测试（在任务里取名额 → 第一次 poll 为 `Pending`，失败）。每个测试都在对应修复撤掉后失败。会话先验签名的顺序无法用行为测试观察，改由类型保证（`VerifiedPayload`），未做时序测量。
- 红→绿（实测）：8 个审计红测试解除忽略后通过；A2 在 SQLite（改成延迟事务即失败）与浏览器（改成先读后写即丢更新）两端各有一个红测试；E1 卸载测试在任务归页面作用域时失败；srcset、`data:` 媒体类型、声明固定等用例在修复前的实现上失败。

### 手工验收

- 界面：未执行（无布局、焦点或导航改动；界面行为由 VirtualDom 会话测试覆盖）。
- 原生旧库升级（2026-10-12，你同意下载官方包、桌面数据库迁移到 0009）：从 v0.1.21 Release 下载 `rssr-cli-windows-x86_64.zip`，SHA-256 `a87071bf…a40a875` 与 Release API 的 `digest` 一致，解压出的 `rssr-cli.exe` 为 `80ac9182…6fb7c0`。候选版本在 `de62375`（工作区干净）用 `cargo build --locked --release -p rssr-cli` 构建（rustc 1.98.1，`Cargo.lock` SHA-256 `74667298…8da6fca`，可执行文件 `d55dda6b…84a50c9`，未打包，直接用构建产物）。`python -X utf8 scripts/verify_release_upgrade.py --legacy-cli <v0.1.21> --candidate-cli <候选> --output-dir target/audit-20261009/upgrade/run-1`：10 项全部 PASS——旧版建库（迁移 1–5、3 个订阅、144 篇正文、48 已读、20 收藏、设置与 CSS）；升到 1–9 后所有旧字段与正文逐字节一致，0008 回填折叠标题，0009 的 `custom_title` 全为空；三次重启不变；0006–0009 各自注入失败都随历史一起回滚、重试成功；旧版持写锁时强制终止候选进程、再启动可完成升级；旧版拒绝已升级的库；恢复原始库后旧版可用、原始库哈希不变。输出留在 `target/audit-20261009/upgrade/run-1/`（`summary.json`、`commands.json`）。
- 浏览器新旧版本交错（2026-10-12，你同意补测）：把最新发布 v0.1.23（`e4c4a82`，只认提交头版本 1）和候选版本（`f15f221` 的导出，生产代码同 `de62375`）分别编译成 wasm 模块，放进同一页面——两者共用 `localStorage` 与 `rssr-browser-state-v1` 锁，各自持有一个长期的 `BrowserStore`，等同两个不同构建的标签页。数据由 v0.1.23 采纳旧键后写出真正的版本 1 提交头。在内置浏览器里运行 4 个场景，全部通过：A 旧版持锁时提交、新版首次访问排在其后（锁队列里确有 2 个请求）——旧版写入成功，新版随后把提交头升到 2，纪元不变，修订号带着旧版的这次提交，新版读到这条已读；A2 新版先排队升级、旧版写入排在后面——旧版被拒（“无法读取此版本的浏览器数据”），存储不变；B 旧版刷新等网络期间新版升级——旧版提交被拒，所有键逐字节不变，旧版之后连读取也被拒；C 新版写入指定名称后，旧版标签页的刷新提交（会重写订阅片）、标记已读、重新打开都被拒，`custom_title` 仍是“我的名称”。对照组：把候选版本的提交头版本改回 1 重新构建，同样 4 个场景全部失败，C 中旧版刷新把 `custom_title` 写成了空——证明测试确实能区分这道版本门槛。证据与驱动在 `target/audit-20261009/mixed-browser/`（`results-candidate.json`、`results-control.json`、`hashes.txt`、`site/index.html`、`driver/`，构建脚本 `build.sh`）；未入库。
- 原生旧库升级（最新发布 v0.1.23，7→9）：`rssr-cli-windows-x86_64.zip` 的 SHA-256 `2efa6b64…0bdec2cb8` 与 Release API 一致，可执行文件 `ad47fae0…d4c414fb`；候选同上。8 项全部 PASS：旧版建库为迁移 1–7（数据同上）；升到 1–9 后旧字段与正文逐字节一致、0008 回填、0009 的 `custom_title` 全为空；三次重启不变；0008、0009 各自注入失败都随历史回滚、重试成功；强制终止后可恢复；旧版拒绝已升级的库；恢复原始库后旧版可用。输出在 `target/audit-20261009/upgrade/run-3-v0.1.23/`。用 v0.1.23 重跑下一条的 B6 对照，结果与 v0.1.21 相同（`upgrade/b6-acceptance-v0.1.23/`）。
- B6 真实二进制对照（同一合成订阅源，先添加，再把源标题改名后 `refresh --all`）：v0.1.21 上 `add-feed --title 我的名称` 添加完成时就显示为源标题 `Chosen`（首刷当场覆盖），改名后跟着变；候选版本添加后与刷新后都显示 `我的名称`，未指定名称的订阅跟随上游改名。v0.1.21 建的库交给候选版本刷新：两个订阅都跟随新的源标题，`custom_title` 为空（不回填的决定，旧名称在旧版里本来就已丢失）。证据在 `target/audit-20261009/upgrade/b6-acceptance/`（`results.json`、`commands.json`）；脚本未入库。

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
- 已定：后台图片本地化不拦截内网与本机地址（你的决定，维持现状，不改代码）。
- 迁移锁：按 SQLite 实际打开的绝对路径加锁，覆盖百分号转义、`file:` URI、相对路径等写法；不覆盖同一文件的其它名字（Windows 8.3 短文件名、文件级符号链接），SQLite 本身也不支持这样打开同一个库。索引库与正文库各自加锁，与本分支更早的构建（正文库也用索引库的锁）不能混用：更新前请退出 GUI 与 CLI，所有进程使用同一个构建。
- 升级验收：已用官方 v0.1.21 Windows CLI 运行，结果见“手工验收”。最新发布是 v0.1.23，v0.1.22 起的库已有迁移 1–7，用户实际走的是 7→9：脚本已改为从旧库读出它已应用的迁移、只对缺的迁移注入失败（对 v0.1.21 复跑仍 10/10，`upgrade/run-2-param/`），经你同意下载 v0.1.23 官方 CLI 后，7→9 也已验过（见“手工验收”）。只覆盖 Windows CLI：GUI、Android 实机、签名与其他平台包未验；官方包与本地构建的迁移文件都是 CRLF，所以 G1 的换行容错没有被真实二进制走到（仍只有集成测试）。Unicode 搜索样本只覆盖原生列表与共用规则，Web 端与原生计数/批量已读未加同类样本。
- 迁移：`sqlx::migrate!` 在编译期读取迁移目录，只新增 `.sql` 而不改任何 Rust 文件时，增量编译可能不会重新嵌入；加迁移时顺手改动 `rssr-infra` 或清理其构建缓存。
- D6：混用新旧版本写同一个库时，旧版本新增的条目没有搜索用标题；旧版本改了标题的，由迁移 0008 的触发器清空过期值。这两种行在下次新版本启动回填前只按 ASCII 不区分大小写匹配原标题。折叠结果按写入时的 Rust Unicode 版本保存，Rust 升级改变大小写表时旧值不会自动重算（浏览器每次现算）；目前没有版本标记。阅读页的上一篇/下一篇导航不按搜索词限定范围，两端一致，属既有行为。
- G1：只有新版程序带这层核对；旧版程序遇到另一种换行建的库仍会拒绝（旧版本身无法修复）。迁移到 0008 之后，旧版程序一律拒绝打开（sqlx 在核对校验和之前先检查未知迁移），降级需要恢复升级前的完整备份。混合换行的文件只在一个方向上被接受（内嵌为混合、记录为整体 LF 时接受，反之拒绝）；跨行字符串字面量的换行变化也会被当成同一份迁移接受（现有迁移里没有这种写法）。
- B6（自定义订阅名不被刷新覆盖）：第一、二单元已完成（上文“订阅名称”）。第三单元是发布说明，发布不在本分支的授权范围内，要点留在这里，发布时直接采用：
  - 订阅名称不再被刷新覆盖：添加时指定的名称（CLI `--title`）、OPML 或配置包导入的名称会一直保留；没有指定名称的订阅显示订阅源自己的标题，并跟随它改名。
  - 升级前保存的名称无法区分是谁起的，一律按订阅源的标题处理，下一次带标题的刷新可能把它换掉。需要保留的名称，升级后重新导入原来的 OPML 或配置包即可。
  - 本应用导出的配置包写的是当前显示的名称；再导入（包括 WebDAV 下载）后这些名称都成为指定名称，之后不再跟随订阅源改名。
  - OPML 导入按规范取 `text`（其他阅读器里改过的名称），导出时 `text` 写显示名称、`title` 写订阅源的标题。Web 端改用与桌面相同的 OPML 解析；两端都更严格了：被截断、订阅不在 `<body>` 里、属性缺等号或引号的文件整体拒绝（以前 `<body>` 外的订阅会被静默跳过）。
  - Web：升级后请重新加载所有旧版标签页；旧版标签页会提示“无法读取此版本的浏览器数据”，不会改动数据。
  - 桌面 / Android：数据库升级到迁移 0009 之后，旧版本程序无法打开，降级需要恢复升级前的完整备份；更新时请先退出所有使用同一数据库的 GUI 与 CLI 进程。
  OPML 已接受的后果：B6 之前本应用导出的 OPML 给无标题订阅写的是 `text=title=订阅地址`，重新导入时地址会成为指定名称（只影响从未取到标题的订阅）；其他阅读器导入本应用 `text=""` 的条目可能显示空名称；优先读 `title` 的阅读器（例如 Miniflux 的实现）会取到源标题而不是指定名称。第十一轮评审另提的两项只记录不改：新旧两个构建的浏览器标签页交错已用 v0.1.23 与候选版本实测（见“手工验收”），不再只是静态分析。测试里临时替换 `Storage.prototype.setItem` 的恢复不在 `finally` 里，与该文件已有测试的写法一致，测试中途崩溃、被取消或超时时可能影响同一页面后续的用例。
  已接受的后果（第一单元起）：同名订阅在文章页“按来源”分组时合并，指定名称会影响分组与目录定位（既有语义）；已在运行的旧版本原生进程仍可能按旧语义写 `title`；配置包 v2 格式固定（版本精确匹配、`deny_unknown_fields`），无法区分名称来源。
- 你确认的后续顺序（2026-10-13）：
  1. sqlx 0.9 / SQLite 3.51.3：已完成（上文）。
  2. 跨进程刷新协调：采用 Codex 设计的“持久化抓取序号 + 条件原子发布”——每个订阅持久化 I（已发出的最大序号）、P（已发布的最大序号）、D（已记录诊断的最大序号），始终 0 ≤ P ≤ D ≤ I；200 只有序号大于 P 才发布，304 还要求它校验的版本正是当前已发布的版本，失败只更新诊断；添加订阅时预抓取的结果在激活事务里领序号；任何锁都不跨网络请求。原生端按你的决定把正文库并回索引库（同一个 SQLite 文件、表仍分开），让发布成为单个事务；迁移 0010 一次性复制正文，旧正文库保留为备份。设计原文在会话临时目录 `review/design-refresh-review.md`，实施前先出实施方案并送评审。
  3. GUI 与 CLI：不合并，见 [GUI 与 CLI 的分发形式](../../../design/cli-gui-distribution.md)；Scoop 包与 macOS 数据目录（现在会写进 `.app` 包内）待实施。
  4. D1：Claude 与 Codex 独立得出同一建议——浏览器端全部状态迁入同一个 IndexedDB、按记录存储，localStorage 只留阻止旧版本写入的版本栅栏；待你确认。若采用，浏览器端的刷新序号直接放在 IndexedDB 的订阅记录里。
  5. “Web 界面读写服务器上的数据库”属于产品边界问题，未决定。
- 待办：D2、D4、D7、E2、E3、G2–G6、H2、H3、B7、F2 其余部分（`/logout` 用 GET、CSP）未处理；D1 是否允许缩短 Web 正文保留期限、Scoop 分发时 GUI 与 CLI 是否共用数据库，两项产品决定仍待你确认。

## 给下一位 Agent 的备注

- 入口：报告第 10 节进度表；每项修复的提交信息写明了机制、取舍与测试。
- 在本机复跑浏览器验证：CI 用的 wasm-bindgen 是 0.2.126，全局装的是 0.2.128；Scoop 的 `chromedriver` 默认是 155，而本机 Chrome 是 154（版本不配时 harness 报 `http status: 404`）。复跑时不要改全局安装，按本次的方式临时指定：
  - `RSSR_WASM_BINDGEN_TEST_RUNNER=<0.2.126 的 wasm-bindgen-test-runner.exe 绝对路径>`（本次用 `cargo binstall --root <临时目录> --version 0.2.126 wasm-bindgen-cli` 装在会话临时目录，会话结束后不保留，需要重装）；
  - `PATH` 前置 `~/scoop/apps/chromedriver/154.0.8037.92`；`CARGO_TARGET_DIR` 指向独立的 wasm 目标目录；然后 `bash scripts/run_wasm_contract_harness.sh <harness>`；
  - `rssr-infra` 的 wasm lib 测试：`CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=<同一个 runner> cargo test -p rssr-infra --lib --target wasm32-unknown-unknown --locked`。
- 独立评审原文在会话 scratchpad 的 `review/batch1-review.md`、`review/round2-review.md`、`review/round3-review.md`、`review/round4-review.md`、`review/round5-review.md`、`review/round6-review.md`（不入库）；处理结论记在本记录与报告第 10 节。

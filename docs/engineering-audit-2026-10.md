# 工程审计报告（2026-10）

> 历史快照：本文结论、路径与行号属于下列基线，不代表之后的主线。当前入口见 [文档索引](./README.md)。

| 项 | 值 |
| --- | --- |
| 审计日期 | 2026-10-09 |
| 审计基线 | `origin/main` = `e4c4a82`（即 v0.1.23 发布源码快照 `e4c4a8260d5890a4da99713deb69185cad4cd2bf`） |
| 审计分支 | `audit/2026-10-09`（独立 worktree，未推送） |
| 执行者 | Claude（Opus 5.5），审计输入含 GPT 2026-10-09 交接意见 |
| 本地工具链 | rustc / cargo 1.98.1（CI 使用未固定的 `stable`） |

## 0. 结论先行

总体判断：分层边界清楚，测试基础扎实（356 个测试、原生与 wasm 契约 harness、真实浏览器 UI 断言），`rssr-web` 的代抓代理（DNS 钉死、逐跳校验、CSP sandbox、流式限长）和 `BrowserStore` 的提交协议都做得扎实。主要风险不在“缺测试”，而集中在五类**跨步骤不变量没有被守住**的地方：

1. **多步写入缺少协调**：同一订阅的两次刷新、AppState 的窄字段更新、列表查询与已读写入之间都没有统一的序号或锁；单个步骤各自正确，组合起来会丢更新或回退状态。
2. **派生数据与源数据共用身份**：图片本地化后的正文覆盖了 `content_hash`，源未变的刷新因此把派生缓存当作“变化”冲掉。
3. **Web 存储架构的容量与写放大天花板**：全部正文放在一片 `localStorage`，无保留/淘汰策略；双缓冲发布使有效容量约减半；每个订阅提交都重写整片索引。
4. **订阅抓取的稳健性缺口**：不发 User-Agent；传输层预先转码而解析器仍按 XML 声明再解码，带编码声明的非 UTF-8 源（GBK 等）无论 HTTP 是否声明 charset 都乱码；Atom 链接按位置选取；无正文条目整条丢弃。
5. **构建可复现性**：迁移校验和随换行符变化（Windows CRLF / 其余 LF），CI 工具链未固定，工作区 crate 的测试只在 Linux 运行。

本轮在审计分支上做了 3 项小而安全的修复（均为红→绿，304 修复同时覆盖原生与浏览器两条路径），并为 8 个需要设计决策的缺陷提交了确定性红测试（`#[ignore]`，可 `--ignored` 运行）。其余发现列为建议与待决策事项，未擅自改动 trait、schema 或平台语义。报告已经过一次跨模型独立评审，处置记录见第 9 节。

### 优先级最高的事项

| # | 发现 | 等级 | 证据 | 位置 |
| --- | --- | --- | --- | --- |
| 1 | 迁移校验和绑定换行符（索引库与正文库两套迁移）：跨平台打开同一数据库失败；若直接改 `.gitattributes`，下一个 Windows 版本会让记录了 CRLF 校验和的库启动失败 | P1 | 已验证（本地库为 CRLF 校验和）+ 推断（发布构建） | `crates/rssr-infra/src/db.rs:18-19`、`.gitattributes` |
| 2 | Web 正文全部存于一片 `localStorage`，无配额处理；发布峰值超过配额期间，含正文变化的刷新持续失败 | P1 | 推断 | `browser/state/storage.rs:160` |
| 3 | `rssr-web` 默认配置下登录限速键退化为全局 `direct\|用户名`，且锁定检查先于密码校验：知道用户名的任何人可反复把唯一用户锁在外面；键数无上界 | P1 | 推断 | `auth/rate_limit.rs:18`、`auth.rs:88` |
| 4 | 同一订阅的单源刷新 / 全量刷新 / 首刷互不协调，旧响应可覆盖新响应 | P2 | 已验证（进程内红测试）+ 推断（跨进程、提交步骤交错） | `bootstrap/native.rs:269`、`application_adapters/refresh.rs:125` |
| 5 | 在途列表查询覆盖成功的已读写入；渲染时采样时钟使查询每秒重发，放大该竞态 | P2 | 已验证（红测试）+ 推断 | `entries_page/session.rs:131`、`entries_page/mod.rs:227` |
| 6 | 源未变的刷新冲掉已本地化正文；后台本地化按成功数而非尝试数限额、任务无界 | P2 | 已验证（红测试）+ 推断 | `db/entry_repository.rs:937`、`bootstrap/native.rs:369` |
| 7 | 抓取稳健性：无 User-Agent；带编码声明的非 UTF-8 源乱码（有无 HTTP charset 皆然）；Atom 链接选错；无正文条目被丢弃（YouTube 频道 0 篇） | P2 | 已验证（红测试 / 探针） | `feed_body.rs:11`、`feed_normalization.rs:66,82,106,111` |
| 8 | AppState 窄字段更新是整份读改写，并发丢更新 | P2 | 已验证（红测试） | `app_state_service.rs:27,40` |
| 9 | Web 端 WebDAV 未兑现文档的凭据转换、无同源/大小/时限约束；Web OPML 解码器为弱化重复实现 | P2 | 推断 | `browser/config.rs:12,66`、`browser/adapters/config.rs:59` |
| 10 | CI 工具链未固定；工作区 crate 测试只在 ubuntu 跑；浏览器纯逻辑单测不在任何 CI 目标上执行 | P2 | 已验证（配置原文）+ 推断（后果） | `.github/workflows/ci.yml:31,135` |

## 1. 范围、方法与证据分级

**阅读范围**：六个 crate 的全部生产源码（约 4.2 万行 Rust，重点精读 domain / application / infra / rssr-web，rssr-app 精读 bootstrap、runtime、entries/reader/settings 会话与 reducer），`migrations/`、`migrations_content/`，`.github/workflows/ci.yml` 与 `release.yml` 的关键 job，`docs/design/functional-design-philosophy.md`、部署与使用文档，以及全部嵌套 `AGENTS.md`。`scripts/`、`specs/`、Android/Gradle 工程、`pages.yml`/`docker.yml` 只做了定位级浏览。

**执行范围**：基线测试、clippy、fmt、wasm 目标检查、探针、3 项修复的红→绿（304 修复含本地 headless Chrome 运行的 wasm 契约 harness）、8 个红测试的失败确认。命令与结果见第 7 节。

**证据分级**（全文统一）：

- **已验证（V）**：有可复现的命令、测试或编译输出直接支持。
- **推断（I）**：读码得出，机制明确，但本轮没有动态复现（常因需要真实浏览器、设备、外部服务或较大夹具）。
- **未验证（U）**：需要特定环境才能判断，本轮只给出假设与验证方法。

**严重度**：P1 = 现实条件下可造成数据错误、启动失败或可利用的可用性问题；P2 = 可观察的错误行为或明显性能退化，有规避方式或触发面较窄；P3 = 卫生、可维护性与边缘情况。

**环境说明**：首次全量编译出现过一次 rustc ICE 报文（`no type-dependent def for method call`），重试时报 `os error 1455`（页面文件不足）。改用 `CARGO_INCREMENTAL=0 -j 4` 后未再出现。归因于本机环境（16 GB 内存、20 线程默认并行，同时 IDE flycheck 也在编译），不是仓库缺陷；但未证明 ICE 一定由内存压力引起。

## 2. 本轮改动（`audit/2026-10-09`）

| 提交 | 内容 | 验证 |
| --- | --- | --- |
| `1456b2b` | 原生标题搜索对 `%` `_` `\` 转义（`ESCAPE '\'`），列表、计数、批量已读预览与写入一致按字面匹配，恢复与 Web 端字面 `contains` 的一致性 | 新测试 `search_title_treats_like_wildcards_as_literals`：未修复时失败（`"100%"` 命中 `"1000 done"`），修复后通过 |
| `28c0441` | 304 只更新响应里出现的校验器（原生 `record_not_modified_for_generation` 用 `COALESCE`；浏览器适配器同语义），200 与失败路径不变 | 原生新测试 `refresh_contract_not_modified_keeps_validators_the_server_did_not_resend`：未修复时 `last_modified` 被清空为 `None`，修复后通过；浏览器端见 `8ef3a36` |
| `64bdbed` | 浏览器查询测试夹具补 `feed_generations`；`test_persistence_atomicity.rs` 补原生门控 | 修复前 `cargo check -p rssr-infra --target wasm32-unknown-unknown --lib --tests` 报 `E0063 missing field feed_generations`；修复后 exit 0，clippy `-D warnings` 通过 |
| `0ac5ec7` | 7 个确定性红测试（`#[ignore = "audit-2026-10-09: …"]`），见附录 A | 每个均以 `--ignored` 运行并确认因所述原因失败 |
| `f814528` | 按独立评审加固红测试：同源刷新测试改为策略中立并全程设看门狗；AppState 夹具交错未建立即失败；新增“HTTP 与 XML 都声明 GBK”红测试；搜索测试补实际批量写入；修正 304 注释 | 新/改红测试均按所述原因失败；模拟“按 feed 串行化”的修复后，同源测试 2.03 秒通过而非挂起 |
| `8ef3a36` | wasm 契约 harness 新增“304 省略校验器”用例 | 本地 headless Chrome：修复后 29 passed / 0 failed；把浏览器适配器还原到修复前时该用例失败（`left: None`，`right: Some("etag-1")`） |
| （文档提交） | 本报告、交接记录、使用指南补充“标题搜索按字面匹配” | — |

这三项之所以直接修：改动局部、平台中立、不改 trait/schema；搜索修复是恢复 Web 已有语义；304 修复依据 RFC 9110 §15.4.5（304 不要求重发 `Last-Modified`）与 RFC 9111 §4.3.4 的更新语义（只用 304 中出现的字段更新已存元数据）；第三项只涉及测试。其余发现涉及端口契约、内容库 schema、存储架构或产品语义，按仓库护栏只给分析与建议。

## 3. 发现详述

### 3.1 数据一致性与并发

**A1 同一订阅的刷新互不协调（P2，V/I）**

- 位置：`crates/rssr-app/src/bootstrap/refresh_flight.rs` 只合并 `refresh_all`；`bootstrap/native.rs:269` 的 `refresh_feed` 与添加订阅的首刷直接调用 `RefreshService`；`RefreshService` 没有按 feed 的互斥；`application_adapters/refresh.rs:125-214` 的 `commit` 由 4 个独立提交组成（元数据 UPDATE、索引事务、正文写入〔另一个数据库〕、抓取状态 UPDATE）。generation 栅栏只拒绝“删除后重新订阅”，不拒绝同一 generation 的并发刷新。
- 已验证（进程内）：红测试 `the_fetch_that_started_last_owns_the_stored_result`——先发起的刷新在源上挂起，后发起的全量刷新提交新版本，旧刷新随后提交，最终 ETag 与正文都回到旧版本。该测试的不变量是策略中立的“最后开始的抓取拥有存储结果”：single-flight（源只被调用一次）与提交序号守卫（拒绝过期提交）都能通过；所有等待都有看门狗，任何实现都不会让它挂起。
- 推断：① 两次提交的 4 个步骤交错时，可出现“正文为旧版、ETag 为新版”，此后条件请求得到 304，旧正文持续到下次源变化；② CLI 与 GUI 两个进程共用同一数据库时没有任何跨进程协调，问题同样存在。
- 建议：① 在 `RefreshService`（或 host 能力层）按 `feed_id` 做 single-flight 或串行化，覆盖单源刷新、全量刷新、首刷与自动刷新；② 跨进程的兜底：每个 feed 记录单调的 `refresh_seq`（开始抓取时分配），每个写入步骤都在**同一道写栅栏内**比较序号再写——索引侧在同一个 IMMEDIATE 事务里检查并写入，正文写入像现有 generation 栅栏一样在持有索引库写锁期间检查序号；只在第一步检查、后续步骤独立提交，仍会出现“新版索引与 ETag 已提交、旧版正文最后覆盖”；③ 把索引侧三步并入一个 IMMEDIATE 事务。跨进程与步骤交错的保证不能只靠上述服务层测试验收，需要真实分库夹具逐步控制交错。

**A2 AppState 窄字段更新是整份读改写（P2，V）**

- 位置：`crates/rssr-application/src/app_state_service.rs:27,40`；原生 `SqliteAppStateRepository` 的 load/save 是两条独立语句；浏览器 `BrowserAppStateAdapter` 的 load/save 是两次独立的 Web Lock 获取。
- 已验证：红测试 `concurrent_narrow_field_updates_do_not_lose_each_other`（仓储在读取后等待另一方也读完，确定性构造交错），`save_entries_workspace` 的修改丢失。
- 触发面：订阅文章页上刷新完成触发的 bootstrap（写 `last_opened_feed_id`）与筛选变更触发的工作区保存并发；Web 多标签页。
- 建议：端口提供原子窄字段操作（原生：同一 IMMEDIATE 事务内读改写或 `json_set`；浏览器：一次 `store.update` 闭包内完成）。这是 domain/application 端口契约变更，需确认后实施。

**A3 在途列表查询覆盖成功的已读/收藏写入（P2，V）**

- 位置：`crates/rssr-app/src/pages/entries_page/session.rs:131-159`（查询代际只防“旧查询覆盖新查询”）与 `189-226`（写入成功后只修补列表并 bootstrap，不使在途查询失效）；`reducer.rs:88` 的 `SetEntries` 整体替换列表。
- 已验证：红测试 `in_flight_query_cannot_resurrect_flags_overwritten_by_a_successful_write`（UnreadOnly 下标记已读后，旧快照把条目恢复为未读）。All / StarredOnly 与不同完成顺序是同一机制（推断）。
- 建议：引入 `mutation_epoch`：每次成功写入递增；查询启动时记录 epoch，完成时若 epoch 已变则丢弃并重查，或把启动后发生的修补按 `entry_id` 重放到结果上。

**A4 渲染时采样时钟使 `EntryQuery` 每秒变化（P2，I）**

- 位置：`entries_page/mod.rs:227` 每次渲染调用 `current_time_utc()` 构造查询；归档分界截断到整秒（`crates/rssr-domain/src/entry.rs:208`）；`use_reactive_task` 的依赖元组直接包含 `entry_query.clone()`（`mod.rs:263`），中间没有 memo；`EntryQuery` 派生的 `PartialEq` 覆盖 `ArchiveFilter::ExcludeArchived { cutoff }`，因此分界每跨一秒，依赖比较就判定为变化。
- 机制：任何一次跨秒的重渲染都会产生新的 `EntryQuery` 并重发列表与计数查询；`SetEntries` 本身又触发重渲染。若查询耗时 ≥ 1 秒（Web 大数据量），会形成持续自触发的重查循环；即便查询很快，也会显著扩大 A3 的竞态窗口。
- 建议：在显式触发点（挂载、刷新完成、筛选变化，以及可选的分钟级定时器）采样一次时钟并存入页面状态，渲染不再读时钟；不改变归档分界的精确语义。

**A5 UI 便利状态的解析失败会阻断核心写操作（P3，I）**

- 位置：`application_adapters/mutations.rs:388-422` 在删除订阅、配置替换的事务内用 `?` 解析 app state；`AppStateSnapshot` 为 `deny_unknown_fields`（`rssr-domain/src/app_state.rs:14,36`）。
- 影响：降级到旧版本或该 JSON 损坏后，删除订阅与配置导入整体失败；`save_entries_workspace` 也会因为先读失败而永远无法自愈。
- 建议：便利状态解析失败时记录并按默认值处理（或跳过清理），不让它阻断订阅/配置这类核心写入。

### 3.2 订阅抓取与解析

**B1 304 清空服务器未重发的校验器（P2，V，已修复）**——见第 2 节。

**B2 两处解码：带编码声明的非 UTF-8 源一律乱码（P2，V；经独立评审后范围扩大）**

- 位置：传输层 `crates/rssr-infra/src/feed_body.rs:11-40` 按 Content-Type 的 charset 转成 UTF-8（无 charset 时按 UTF-8 有损解码）；解析器 `feed_normalization.rs:66` 再把这段 UTF-8 文本的字节交给 feed-rs，而 feed-rs 会遵从文本里原样保留的 `encoding="…"` 声明再解码一次。
- 已验证（探针矩阵 + 两个红测试，正文均为 GBK 编码）：

  | HTTP charset | XML 声明 | 解析出的标题 |
  | --- | --- | --- |
  | 无 | GBK | “锟斤拷…”（红测试 `prolog_declared_gbk_feed_keeps_its_text`） |
  | GBK | GBK | “涓枃鏍囬…”（红测试 `http_and_prolog_both_gbk_feed_keeps_its_text`） |
  | GBK | 无 | “中文标题”（正确） |

  第二行是 GBK 源最常见的正确标注方式，因此影响面大于初稿所写的“只在 XML 声明里给编码”。
- 建议：在 infra 里只保留**一个**解码边界并补齐两类回归。HTTP 未声明 charset 时把原始字节交给 XML 解析器（按 BOM / XML 声明解码）；HTTP 声明了 charset 时以它为准（RFC 7303 §3.2：BOM 存在时以 BOM 为准，否则 charset 参数优先于 XML 声明），转码为 UTF-8 后必须同步改写或去掉 XML 声明中的 encoding，不能留下过期声明。字节上限与转码后长度上限都要保留。

**B3 原生抓取不发送 User-Agent（P2，V）**

- 位置：`fetch/client/feed_http.rs:48` 的客户端未设置 UA；订阅探测用的是 `reqwest::Client::new()`（`composition.rs:66`）。图片本地化已单独设置 UA。
- 已验证：红测试 `feed_fetch_identifies_the_client`，请求头中没有 `user-agent`。RFC 9110 §10.1.5 规定客户端 SHOULD 发送 UA（允许显式配置关闭）。“部分站点或 CDN 拒绝无 UA 请求”是常见经验，本轮未对具体站点验证（U）。
- 建议：统一的 HTTP 客户端构造（UA、超时、重定向策略）供刷新、探测、WebDAV、图片共享（见 5.4）。

**B4 Atom 链接按位置而非 `rel` 选择（P2，V）**

- 位置：`feed_normalization.rs:82`（站点地址）与 `:106`（条目地址）均取 `links.first()`。
- 已验证：红测试 `atom_links_are_selected_by_relation_not_position`：站点地址取到 `rel="self"` 的 feed 地址；条目地址取到 `rel="replies"`。探针中 YouTube 频道 feed 的 `site_url` 也是 feed 自身地址。
- 建议：优先 `rel="alternate"` 或无 `rel` 的链接，再回退到第一个。

**B5 无 summary / content 的条目整条丢弃（P2，V，需产品决策）**

- 位置：`feed_normalization.rs:111-116`。
- 已验证（探针）：YouTube 频道形态的 Atom（正文在 `media:group`）解析结果 0 篇；只有标题与链接的 RSS 条目 0 篇。用户能成功订阅，但列表永远为空，只在日志里有一条 warn。
- 建议：保留无正文条目（只建索引，`has_content = false`，阅读页给出原文链接），或把 `media:description` 作为摘要。设计哲学写的是“feed 提供多少正文就缓存多少”，并未要求丢弃条目，因此建议按保留处理，但需你确认。

**B6 用户或导入的订阅名会被下一次刷新覆盖（P3，V）**

- 位置：`db/feed_repository.rs:184,291` 用 `COALESCE(源标题, title)`；首刷紧随订阅之后执行。
- 已验证（探针）：存入“自定义名”后执行元数据更新，标题变为源标题。因此执行刷新且源提供非空标题时，CLI `add-feed --title` 指定的名称会被覆盖（`--skip-refresh` 只把覆盖推迟到下一次刷新；源标题为空时 `COALESCE` 保留原值）；OPML / 配置包往返同样会丢失其他阅读器里的自定义名称。
- 建议：区分 `custom_title` 与源标题（新列），或约定“已有非空标题时不被源标题覆盖”。属于配置交换保真问题，需决策。

**B7 Web 刷新从不发条件请求，且直连附加时间戳参数（P3，I）**

- 位置：`browser/feed.rs:36-94` 只发送 URL；`browser/feed_request.rs:61` 追加 `_rssr_fetch=<毫秒>`，同时绕过浏览器 HTTP 缓存；`/feed-proxy` 也不转发条件头。
- 影响：Web 每次刷新都是全量下载与全量解析，存储的 ETag / Last-Modified 在 Web 上从未被使用。与原生（按“有缓存且上次成功”才发送校验器）语义不一致。
- 建议：由适配器带上校验器，代理转发 `If-None-Match` / `If-Modified-Since` 并透传 304；去掉时间戳参数或以显式缓存模式替代。

### 3.3 缓存与资源边界（原生）

**C1 源未变的刷新冲掉已本地化正文（P2，V）**

- 位置：本地化写回把 `content_hash` 改成本地化 HTML 的哈希（`bootstrap/native.rs:422-441`）；刷新写正文时以 `content_hash IS NOT excluded.content_hash` 判定变化（`db/entry_repository.rs:937`）。
- 已验证：红测试 `unchanged_source_refresh_keeps_localized_body`：同一源正文再次刷新后，正文回到远端图片地址。
- 影响：每轮刷新都把已本地化文章恢复为远端图片，再由后台任务重新下载前 5 篇的图片；阅读页按需本地化的结果同样会被冲掉，离线阅读退化。
- 建议：区分源版本与派生缓存——正文库新增 `source_hash`（内容库迁移），刷新比较源哈希：未变则保留派生 HTML，变化才失效。

**C2 后台图片本地化缺少尝试、队列与总量边界（P2，I）**

- 位置：`bootstrap/native.rs:369-466`：`MAX_BACKGROUND_LOCALIZED_ENTRIES = 5` 只在写回成功时计数（`:446`），失败、超时、无变化的条目不计数；每个“已更新”的订阅都 `tokio::spawn` 一个任务（`:375`），任务持有该订阅全部条目正文的副本（`refresh_service.rs:558` 克隆所有 `content_html`）。
- 影响：图片全部失败（防盗链、离线）时，一个 50 篇的订阅单轮最多发起 50 × 16 次请求，单篇超时上限 133 秒；自动刷新叠加时任务与正文副本可无限堆积。
- 建议：按尝试计数并设置每轮、每订阅的时间与字节预算；只把新增或正文变化的条目送去本地化；改为单个后台 worker + 有界队列，按 `(feed_id, dedup_key)` 去重并丢弃过期 generation 的任务。

**C3 本地化图片以 data URL 内嵌正文（P3，I）**

- 影响：存储膨胀约 33%，同一张图在多篇文章里重复存储；每次打开阅读页都要对数 MB 的 HTML 做 ammonia 消毒与自定义标签解析，并通过 IPC 整体传给 WebView；也无法利用 WebView 的图片缓存。
- 建议（中长期）：内容寻址的本地资源文件 + 自定义协议 / 资源处理器，正文只引用哈希。

**C4 刷新的写放大（P3，I）**

- 每次刷新对每个条目执行 `ON CONFLICT … DO UPDATE`（`updated_at = now`，即使内容未变），并维护 `entries` 上 10 个索引；其中 `published_at` 系列旧索引在引入排序键表达式索引后可能已无查询使用，`idx_entries_title` 对 `LIKE '%x%'` 无效。
- 建议：为条目 upsert 加“无变化不更新”的 `WHERE` 条件；用真实 SELECT 列表做 `EXPLAIN QUERY PLAN` 审计后新增迁移删除无用索引（迁移 0004/0005 的教训说明必须用生产查询评估）。

**C5 写正文库期间持有索引库写锁（P3，I）**

- `upsert_contents_for_generation` 在写另一个数据库期间持有索引库 `BEGIN IMMEDIATE`；大批正文写入时，前台“标记已读”等写入要排队（busy_timeout 30 秒）。这是 generation 栅栏的有意代价，建议先测量，必要时分块写并逐块复验 generation。

### 3.4 Web 平台（浏览器存储与配置交换）

**D1 `localStorage` 容量天花板且无配额处理（P1，I；U：各浏览器配额与计量方式）**

- 位置：全部正文在 `entry_content` 一片（`browser/state/models.rs`），代码中没有任何配额处理；`publish`（`browser/state/storage.rs:160-182`）先写新槽、再切换提交头、最后清理旧槽，因此一片数据在发布瞬间需要新旧两份同时在配额内。
- 影响：WHATWG Storage Standard 给出的 `localStorage` 建议配额是 5 MiB，实际计量方式与上限随浏览器而异，本轮未实测可容纳的篇数。正文只增不减（归档只是视图过滤），一旦正文片的发布峰值超过剩余配额，所有带正文变化的刷新都会以 QuotaExceeded 失败；只有在释放空间或正文变小后才会恢复。
- 建议：分成两类处理，不能混为一谈。
  - **保留全部正文的存储优化**（infra 内部，application 语义不变）：捕获配额错误并给出明确提示、在设置页显示用量；正文（及索引）迁到 IndexedDB 按记录写入，保留现有提交头协议于小数据片；压缩或去重。工作量大，需要按护栏先做方案评审。
  - **改变保留期限的策略**（例如只保留未归档或每订阅最近 N 篇的正文）属于产品决策：设计哲学规定“归档后的文章仍然保留在本地库中，可再次查看和阅读”，而条目退出上游 feed 后正文无法再取回。列入第 6 节待决策事项，不能当作无语义变化的 infra 优化实施。

**D2 写放大：每个订阅提交与每次设置保存都重写整片索引（P2，I）**

- 位置：`browser/adapters/refresh.rs:174` 每次提交都标记 `Changes::CORE`（全部订阅 + 全部条目索引）；设置也存放在 core 片（`browser/adapters/settings.rs:24`）。全量刷新 F 个订阅即序列化 F 次整片索引。
- 文档漂移：`rssr-application/src/refresh_service.rs:280-306` 仍把 `begin_batch`/`end_batch` 描述为“把每个订阅重写一次全库压成整轮一次”的 localStorage 机制，但 `BrowserRefreshStore` 已改为逐订阅持久化（`browser/adapters/refresh.rs:105`）。需要确认当前意图并更新注释。
- 建议：设置独立成片；索引按订阅分片或迁入 IndexedDB；若坚持逐订阅持久化，应同步修正文档。

**D3 浏览器批量 upsert 为二次复杂度（P2，I）**

- 位置：`browser/state/entries.rs:76-149`：每条新条目在全部条目中线性查找（`:97`），正文片也线性查找，旧 GUID 提升再做两次全表扫描。刷新 k 条进 N 条的库为 O(k·N)。
- 建议：每批构建 `HashMap<(feed_id, dedup_key), index>` 与 `HashMap<entry_id, index>`，保持 GUID 提升、重复顺序与原子发布语义；额外临时内存 O(N)。

**D4 Web 端 WebDAV 与原生不一致（P2，I；U：真实浏览器 + CORS 夹具）**

- 位置：`browser/config.rs:12-18` 的 `remote_url` 直接 `join`，没有原生端的“必须相对、保持同源、拒绝反斜杠与控制字符”校验；`browser/adapters/config.rs:59` 以 `.text()` 读取任意大小响应、无应用级时限；端点里的 userinfo 原样放进请求 URL。
- 文档 `docs/user-guide.md:65` 写明“客户端会把凭据从请求 URL 中剥离，改用 HTTP Basic”，未区分平台。按 WHATWG Fetch，URL 含凭据时构造请求会抛 TypeError，因此 Web 端带凭据的端点预计必然失败。这一点需要真实浏览器与本地 CORS 夹具确认，不能直接归因于 CORS。
- 建议：把原生端 `remote_url` 校验与凭据剥离移到不受平台门控的纯函数模块，两端共用；Web 下载加字节上限与 AbortController 时限；失败不得改变本地数据（现有导入流程是先下载后整体替换，满足该要求）。

**D5 Web OPML 解码器是弱化的重复实现（P2，I）**

- 位置：`browser/config.rs:66` 的 `decode_opml` 没有根元素、`<body>`、截断与孤立 outline 校验；而通用 `crate::opml::OpmlCodec` 本身就能在 wasm 上编译（`lib.rs` 未对 `opml` 门控）。截断或根外内容的文档在原生被拒绝、在 Web 被部分导入。编码函数也是逐行复制。
- 建议：`BrowserOpmlCodec` 直接委托 `OpmlCodec`，删除重复实现。

**D6 搜索的 Unicode 大小写契约未定义（P3，I，需决策）**

- 通配符差异已在本轮修复；剩余差异是原生 `NOCASE` 只折叠 ASCII，Web 用 Unicode `to_lowercase`。需要明确契约后再统一（例如两端都只做 ASCII 折叠，或原生用应用层折叠列）。

**D7 浏览器纯逻辑在 CI 中零执行覆盖（P2，V/I）**

- 已验证：基线上 `rssr-infra` 的 wasm lib 测试目标无法编译（`E0063`，本轮已修复）。推断：这些模块里的 `#[test]` 在 wasm32 上不会执行（需要 wasm-bindgen-test 运行器）；按 `ci.yml` 原文，wasm 相关步骤只有 `rssr-app` 的 clippy（不含测试，`ci.yml:213`）与三个 `wasm_*` 契约 harness，没有步骤编译或执行 `rssr-infra` 的 lib 测试。本轮未查看 CI 运行记录（`gh run list` 超时）。`query.rs`、`state/entries.rs`、`state/models.rs` 都是纯 Rust 逻辑。
- 建议：把这些纯逻辑移出 `#[cfg(target_arch = "wasm32")]` 门控（只把 `web_sys` / Web Locks 留在门控内），让单测在原生 CI 中运行；顺带可对 D3 做复杂度回归。

### 3.5 UI 状态与交互

**E1 JSON 配置导入后主题、CSS、密度、字号不立即生效（P2，I）**

- 位置：`ui/runtime/feeds.rs:100-125` 只返回订阅页 intent；对照 `ui/runtime/settings.rs:48-62` 的 WebDAV 拉取会返回 `SettingsLoaded` 并经 `settings_page/session.rs:66-70` 更新全局 `ThemeController`。
- 影响：导入后要访问设置页或重启才应用新外观。
- 建议：配置替换成功后由 shell 层统一重新加载设置（例如一个全局的 `SettingsReplaced` intent），而不是依赖设置页存在。

**E2 目录锚点身份不稳定（P3，I）**

- 位置：`entries_page/groups.rs:346-360` 的 slug 把大小写与 `- _ / .` 归一，不同标题（如 `A/B` 与 `a-b`）得到相同 DOM id；日期与来源月份锚点拼接了切片首条的 id（`:376,403,438,451`），新文章到来或换页时锚点漂移。
- 与 PR #24 的“十月目录不溢出”现象无直接因果；修复时保留按标题聚合的语义，用稳定分组键 + 冲突后缀生成 id。

**E3 其他低优先级**：阅读位置缓存无上限且键含搜索词（`ui/reading_position.rs:22`）；修改自动刷新间隔要等当前休眠结束才生效（`bootstrap/native.rs:206`）；Web 刷新期间离开页面时依赖 `RefreshBatchGuard` 的同步收尾，当前浏览器适配器无批次，故无风险但注释过时（见 D2）。

### 3.6 `rssr-web` 部署服务

**F1 登录限速在默认配置下可被用于远程锁定（P1，I；经独立评审由 P2 上调）**

- 位置：`auth/rate_limit.rs:18-21`：不信任代理头时（默认，也包括文档推荐的反代部署）客户端标识固定为 `"direct"`，键为 `direct|用户名`；`main.rs:96` 未启用 `ConnectInfo`，拿不到对端地址；`auth.rs:88` 在校验密码**之前**判断是否已锁定，因此锁定期间正确密码同样被拒绝。
- 前提：部署可被攻击者访问（公网或共享网络），且攻击者知道或猜中唯一用户名（用户名必填、无默认值，但部署文档的本地示例使用 `admin`，见 `docs/deployment/web.md:18`）。
- 影响：满足前提时，连续 5 次错误（默认值）即可把所有者锁在外面 15 分钟，并可周期性维持；这是报告 P1 定义中的“可利用的可用性问题”。变换用户名可无限新增键（内存无上界），每次登录尝试对整表 `retain`（`:68`），键数增长后为 O(n)。开启信任时取最左 `X-Forwarded-For`（`:86`），只有反代覆盖该头时才可信（部署文档已写明前提）。
- 建议：不信任代理时用对端地址；信任代理时按配置的跳数取最右侧不可信地址；键表设容量上限并惰性/定期清理；未知用户名不分配键；对真实用户名采用递增延迟而非硬锁定；Argon2 校验放入 `spawn_blocking` 并用信号量限并发。

**F2 其他（P3，I）**：Argon2 校验直接在异步 worker 上执行；错误用户名立即返回、正确用户名执行 Argon2，形成可测的时序差（单用户场景影响小）。`/logout` 是 GET（`main.rs:84`），可被跨站触发登出。应用壳没有下发 CSP，feed HTML 的防线只有 ammonia 一层；建议在验证 Dioxus Web 需求后补上 `script-src 'self' 'wasm-unsafe-eval'` 等策略。

**F3 消毒白名单把 `data:` 开放给所有 URL 属性（P3，V/U）**

- 位置：`html/reader.rs:63` 的 `add_url_schemes(&["data"])` 是全局的。
- 已验证（探针）：`<a href="data:text/html;base64,…">` 原样保留（仅追加 `rel`）。本意只是保留本地化图片。
- 未验证：点击后会发生什么取决于平台——桌面端经系统处理器打开、Web 端在新上下文中导航，而主流浏览器对顶层 `data:` 导航有限制（例如 Firefox 59 起的拦截策略）。是否执行其内容需按目标平台实测；无论结果如何，这都是不必要的放宽。
- 建议：用 `attribute_filter` 只允许 `img`/`source` 的 `src`/`srcset` 使用 `data:image/*`。

### 3.7 构建、发布与工程体系

**G1 迁移校验和绑定换行符（P1，已验证 + 推断，需决策）**

- 事实：`.gitattributes` 只约束 `*.sh`；Windows 工作区中 `migrations/*.sql` 与 `migrations_content/*.sql` 均为 CRLF（`git ls-files --eol` 显示 `i/lf w/crlf`）；索引库 `MIGRATOR` 与正文库 `CONTENT_MIGRATOR` 都由 `sqlx::migrate!`（`db.rs:18-19`）在编译期嵌入字节，锁定版本 sqlx-core 0.8.6 以 `Sha384::digest(sql.as_bytes())` 计算校验和、不归一化换行（`migrate/migration.rs:25`），已应用版本校验和不等时返回 `VersionMismatch`（`migrate/migrator.rs:173-176`）；启动流程要求两库迁移都成功（`bootstrap/native.rs:94-100`）。
- 已验证：仓库根目录本地 Windows 构建生成的 `rss-reader.db`（2026-03-29，`_sqlx_migrations` 中仅含 version 1），其 version 1 的校验和等于 CRLF 字节的哈希（只读副本比对）；全部 8 个迁移（7 个索引库 + 1 个正文库）的 LF 与 CRLF 哈希两两不同。
- 推断：Windows 发布在 `windows-latest` 上构建（`release.yml:297`）。runner 镜像的 `Install-Git.ps1` 未显式设置 `core.autocrlf`，沿用 Git for Windows 安装器默认的 `CRLFAlways`（`core.autocrlf=true`），因此发布构建为 CRLF 检出的可能性很高；其余平台为 LF。发布产物实际嵌入的字节本轮未验证。
- 后果（双向）：① 同一个数据库在 Windows 与 Linux/macOS 版本之间迁移（便携目录、同步盘、WSL）会因校验和不一致拒绝启动；② 若直接加 `*.sql text eol=lf`，下一个 Windows 版本会让记录了 CRLF 校验和的库启动失败（本地样本只证明了 version 1 的来源，不能代表所有发布库）。
- 兼容方案（需确认后实施）：(a) 增加 `*.sql text eol=lf`；(b) 在**两套** migrator 运行之前，对已应用迁移逐条比较：若库中校验和等于“内嵌 LF 源的 CRLF 变体”哈希，在事务内改写为内嵌校验和并记录日志；(c) 其他任何不一致仍然失败，真正被改写的迁移依旧被拒绝；(d) 测试对索引库与正文库分别覆盖 LF 库、CRLF 库与被篡改库三种情况。

**G2 CI 工具链未固定（P2；配置原文 V，后果 I）**：`ci.yml` 的 10 处工具链安装全部是 `dtolnay/rust-toolchain@stable`（`ci.yml:31` 等），代码中已有针对“Rust 1.99 Clippy 误报”的全局 `allow`（`rssr-infra/src/lib.rs:1`）；本地为 1.98.1。推断：新 stable 的 lint 或回归会直接打断主线。建议 `rust-toolchain.toml` 固定版本，并用定期升级 PR 显式迁移。

**G3 平台覆盖缺口（P2；配置原文 V，后果 I）**：工作区 crate 的 clippy 与测试 job 只在 `ubuntu-latest` 运行（`ci.yml:135`）；CI 里唯一的 Windows runner 只测 `scripts/release-ui` 这个独立 CLI（`ci.yml:101-107`）。Windows 是主力开发与发布平台（路径、换行、文件锁、MSVC 差异），macOS 同样没有测试；浏览器纯逻辑见 D7。建议至少为 `rssr-infra`、`rssr-application`、`rssr-cli` 增加 `windows-latest` 测试 job。

**G4 依赖与 schema 卫生（P3，静态检索）**：`rssr-infra` 的 `regex`、`thiserror` 在源码与测试中均无引用；`rssr-cli` 的 `async-trait`、`rssr-app` 的 `gloo-file` / `uuid`（后者可能用于 wasm 的 getrandom 特性统一）疑似未用，需 `cargo-machete` / `cargo-udeps` 确认；迁移 0001 的 `sync_sources` 表没有任何代码引用。

**G5 重复实现（P3，I）**：订阅 upsert SQL 有三份且语义不同（`feed_repository.rs` 两份、`mutations.rs::upsert_config_feed` 一份）；OPML 编解码两份（D5）；`ReaderService` 与 `EntriesListService` 的 toggle 重复。重复本身已经造成过规则分叉（`validation.rs` 顶部注释记录了一次），建议收敛。

**G6 文档漂移（P3；原文对照 V）**：见 D2（批次注释，`refresh_service.rs:280-285` 对照 `browser/adapters/refresh.rs:105-106`）与 D4（WebDAV 凭据说明未区分平台，`docs/user-guide.md:65`）。

### 3.8 CLI 与其他小问题

- **H1 `remove-feed --purge-entries` 无法设为 false（P3，V）**：`rssr-cli/src/main.rs:100` 为布尔 flag 配 `default_value_t = true`；实测 `--purge-entries=false` 被 clap 拒绝。软删除在 CLI 不可达，选项形同虚设。建议改为 `--keep-entries`，或 `ArgAction::Set` 接受显式布尔值。
- **H2 属性实体解码循环最多 8 次（P3，I）**：`html/live_display.rs:748-758` 会把 `&amp;lt;` 解成 `<`，对合法包含 `&amp;` 的 URL 会多解一层。属于对“双重转义 feed”的宽容，建议改为一次解码，或仅在检测到双重转义时再解一层。输出最终经 ammonia，不构成注入。
- **H3 同秒内时间字符串排序（P3，I）**：排序键 `COALESCE(published_at, created_at)` 比较 RFC3339 字符串；`published_at` 为整秒 `…Z`，`created_at` 含可变长小数秒，同一秒内 `"…00Z"` 字典序大于 `"…00.5Z"`，原生与 Web（比较时间值）在同秒内顺序可能不同。可在写入时统一为固定宽度格式。

## 4. GPT 交接意见逐项核对

| GPT 条目 | 本轮结论 | 证据 |
| --- | --- | --- |
| §一 PR #19/#23/#25/#26 已完成 | 确认已合并；#25/#26 随 v0.1.23 发布，当前 `main` 即发布快照 | `git log origin/main` |
| §二.1 同 feed 单源/全量刷新重叠 | 仍存在（A1） | V：进程内红测试（旧响应胜出）；跨版本混合与跨进程为 I |
| §二.1 AppState 读改写覆盖 | 仍存在（A2） | V：服务层红测试 |
| §二.1 在途旧查询覆盖已读/收藏 | 仍存在（A3） | V：会话层红测试（UnreadOnly）；其余筛选同机制 I |
| §二.2 Web WebDAV userinfo→Basic | 仍存在（D4） | I（读码 + Fetch 规范）；真实浏览器未验证 |
| §二.2 Web WebDAV 下载无预算 | 仍存在（D4） | I |
| §二.2 Web OPML 弱解析器 | 仍存在（D5） | I |
| §二.2 JSON 导入设置不即时传播 | 仍存在（E1） | I |
| §二.2 LIKE 与字面 contains | 通配符部分**已修复**（`1456b2b`）；列表、计数、预览与实际批量写入一致；Unicode 契约待决策（D6） | V：红→绿 |
| §二.2 WASM lib-test 夹具缺 `feed_generations` | 确认并**已修复**（`64bdbed`）；性质是测试目标覆盖缺口，产品 CI 当时为绿是真实的 | V：编译错误 → exit 0 |
| §二.3 render 采样时钟 | 仍存在（A4），并补充“查询 ≥1 秒时持续重查” | I |
| §二.3 Web 批量 upsert 二次复杂度 | 仍存在（D3） | I |
| §二.3 图片本地化按成功计数、无界 | 仍存在（C2） | I |
| §二.3 不变源刷新覆盖本地化缓存 | 仍存在（C1） | V：红测试 |
| §二.3 登录失败键无上界、全表 retain | 仍存在（F1），并补充“默认配置下为全局键，锁定先于密码校验，可远程锁定”，等级为 P1 | I |
| §二.3 低优先级：阅读位置缓存、同秒 RFC3339 排序 | 均确认存在（E3、H3） | I |
| §三 目录滚动与 PR #24 | 本轮未复查滚动现象（缺原始窗口、DPI、数据）；identity 线索确认（E2），与十月现象无直接因果 | I |
| §四 工具链迁移后续阶段 | 不在本轮范围，未审 | — |
| §五 待决策事项 | 维持待决；CRLF 校验和补充了已验证证据与兼容机制（G1）；私网图片访问见下 | — |

关于“自动图片的私网访问”：`BodyAssetLocalizer` 的重定向策略不限制私网与回环地址（其测试显式允许 `http://[::1]:8080`），后台本地化会在用户未打开文章时就对正文里的任意地址发起 GET。与“用户主动订阅私网源”是两个问题：前者由第三方 feed 内容驱动、无交互，建议至少对**后台**本地化默认拒绝私网目标（阅读时按需本地化可保留现状），仍需你决策。

## 5. 优化建议（性能与工程）

1. **统一“写入序号”机制**：A1（每 feed 刷新序号）、A3（页面写入 epoch）本质上是同一个缺失的抽象——“结果是否仍然对应最新的意图”。建议在 application 层提供一个通用的单调序号工具，而不是各处临时补丁。
2. **派生数据显式建模**：正文本地化、未来的任何缓存都应有“源版本”字段（C1），让失效规则可测试，而不是复用内容哈希。
3. **Web 存储重构的最小路径**：先做 D1 中保留全部正文的配额处理（可在现有协议内完成），再评估 IndexedDB；D2/D3 的分片与索引化可独立先做。这些改动保持在 infra/adapter 层，application 语义不变。缩短正文保留期限则是产品决策，不属于这一类（见第 6 节）。
4. **HTTP 客户端单一出处**：刷新、探测、WebDAV、图片四处各自构造客户端（超时、UA、重定向不一致）。提供一个按用途配置的工厂，统一 UA、超时、重定向与私网策略（B3、B8、私网决策）。
5. **测试结构**：把浏览器纯逻辑移到可原生编译的模块（D7）；为 Windows 增加原生测试 job（G3）；固定工具链（G2）。这三项会把“只在本机发现”的问题提前到 CI。
6. **查询性能**：页面状态里固定查询时钟（A4）能直接减少一大类重复查询；列表与计数共用一次扫描（Web）或一次事务快照（原生）可再省一次往返。
7. **写放大**：条目 upsert 的无变化跳过、索引审计（C4）在大订阅量下收益明显，需配对基准（同数据、同产物）证明。

## 6. 建议执行顺序与需要你决策的事项

**第一批（小改动或高风险项，多数红测试已就绪）**：F1 登录限速重构（对端地址键、容量上限、递增延迟）、B3 UA、B2 单一解码边界（两个红测试）、B4 Atom `rel`、F3 `data:` 收窄、H1 CLI flag、E1 导入后应用设置、A4 查询时钟、A3 写入 epoch。

**第二批（需要小型设计，涉及端口或 schema）**：A1 每 feed 协调 + 刷新序号（序号检查与每个写入步骤同栅栏）；A2 AppState 原子窄字段端口；C1 正文 `source_hash`（内容库迁移）；C2 本地化 worker 与预算；A5 便利状态容错。

**第三批（Web 平台）**：D1 配额处理（保留全部正文）→ D2/D3 分片与索引化 → D4/D5 共用校验与编解码 → B7 条件请求。IndexedDB 迁移需单独方案评审。

**第四批（发布工程）**：G2 固定工具链、G3 Windows 测试 job、G1 换行符兼容迁移（两套 migrator，需决策后实施）。

**需要你决策**：

1. B5：无正文条目是保留（推荐）还是继续丢弃？
2. B6：用户/导入的订阅名是否应在刷新后保留？
3. D6：搜索大小写契约是 ASCII 还是 Unicode？
4. G1：是否接受“启动时把 CRLF 变体校验和改写为 LF”的兼容迁移方案及其发布节奏？
5. 后台图片本地化是否默认拒绝私网与回环地址？
6. （沿用 GPT）Scoop 分发时 GUI 与 CLI 是否共享数据库。
7. D1：Web 端是否允许缩短正文保留期限（例如淘汰已归档文章的正文）？这会改变“归档文章仍可本地阅读”的现有契约，且上游已下线的正文无法取回；不接受时只能依靠配额处理与 IndexedDB 迁移。

## 7. 验证记录

| 命令（审计 worktree，`CARGO_TARGET_DIR` 独立） | 结果 |
| --- | --- |
| `cargo test --workspace --locked -j 4`（基线 `e4c4a82`） | 通过：356 passed / 0 failed / 3 ignored，38 个测试二进制，1m37s |
| `cargo clippy --workspace --all-targets --locked -j 4 -- -D warnings`（基线） | 通过，2m45s |
| `cargo check -p rssr-infra --target wasm32-unknown-unknown --lib --tests --locked`（基线） | **失败**：`E0063 missing field feed_generations`（`browser/query.rs`）及 `test_persistence_atomicity.rs` 未门控 |
| 同上（`64bdbed` 后） | 通过 |
| `cargo clippy -p rssr-infra --target wasm32-unknown-unknown --lib --tests -- -D warnings` | 通过 |
| `cargo clippy -p rssr-app --target wasm32-unknown-unknown -- -D warnings` | 通过（首次报 `only metadata stub found for rlib dependency std` / `can't find crate for futures_core` 中断，当时 IDE flycheck 正并发编译，推测与内存压力有关但未证明；`-j 2` 重跑通过） |
| `cargo fmt --all --check` | 通过 |
| `cargo test --workspace --locked -j 3`（`7b69a56`） | 通过：358 passed / 0 failed / 10 ignored |
| `cargo clippy --workspace --all-targets --locked -j 3 -- -D warnings`（`7b69a56`） | 通过 |
| 红测试 `-- --ignored`（`0ac5ec7`：rssr-infra 5、rssr-application 1、rssr-app 1） | 全部按预期失败 |
| 迁移校验和比对（`rss-reader.db` 只读副本，仅含 version 1） | version 1 = CRLF 变体哈希；8 个迁移的 LF/CRLF 哈希两两不同 |
| `rssr-cli remove-feed 1 --purge-entries=false` | clap 报错：`unexpected value 'false'` |
| 探针矩阵：GBK 正文 × HTTP charset × XML 声明 | 见 B2 表 |
| 评审后：`test_entry_state_and_search`、`test_refresh_contract_harness` | 通过：各 5 passed（含实际批量写入断言） |
| 评审后：红测试 `-- --ignored`（rssr-infra 6、rssr-application 1） | 全部按所述原因失败；同源测试为确定性失败（`v1-older` 覆盖 `v2-newer`），无看门狗超时 |
| 评审后：同源测试的临时副本，模拟“按 feed 串行化”的修复（未提交） | 1 passed，2.03 秒，无挂起 |
| `bash scripts/run_wasm_contract_harness.sh wasm_refresh_contract_harness`（本地 headless Chrome 154，wasm-bindgen 0.2.126 隔离安装） | 修复后：29 passed / 0 failed；浏览器适配器还原到修复前：28 passed / 1 failed（新用例，`left: None`） |
| `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo clippy -p rssr-infra --target wasm32-unknown-unknown --lib --tests -- -D warnings`（`8ef3a36`） | 全部通过 |
| `cargo test --workspace --locked -j 3`（`8ef3a36`） | 通过：358 passed / 0 failed / 11 ignored（3 个原有 + 8 个红测试），2m57s |

## 8. 未覆盖范围与已知局限

- Web：只在本地 headless Chrome 中运行了 `wasm_refresh_contract_harness`；另外两个 wasm 契约 harness、UI 断言脚本与真实部署场景未运行（D1、D4、B7 的浏览器侧行为均为推断）。
- 未在 Android、macOS、Linux 实机或模拟器上运行。
- 未做性能基准；所有性能结论是复杂度与机制分析，不含测量数据。
- 未审查 `scripts/`、`specs/`、Android/Gradle 工程、`pages.yml`、`docker.yml` 的细节，未复查 PR #24 的目录滚动现象，未评审工具链迁移后续阶段。
- 报告经过一次跨模型独立评审（第 9 节）；按评审修订后的版本未再次送审。

## 9. 独立评审记录

- 评审方式：`codex-reviewer`（WSL 只读沙箱，`gpt-6-astra`，`model_reasoning_effort=max`，启用网页检索），范围 `e4c4a82..7b69a56`，耗时约 22 分钟，exit 0。评审方读了报告、6 个提交、原始日志与 GPT 交接原文，并用内存 SQLite 复核了 LIKE 转义与 304 SQL；它没有构建或运行 Rust 测试，也没有使用浏览器或发布产物。
- 结论：1 项 BLOCKER、6 项 SHOULD_FIX、3 项 NEEDS_VERIFICATION、2 项 NICE_TO_HAVE。逐条到仓库核实后全部采纳（其中第 4 项经探针证实比评审所述更广），未驳回任何一项。

| # | 评审意见 | 核实 | 处置 |
| --- | --- | --- | --- |
| 1 | BLOCKER：同源刷新红测试编码了某一种修复方式，按 single-flight 修复后会挂起 | 读码属实 | `f814528` 改为策略中立的不变量并全程看门狗；模拟串行化修复后 2.03 秒通过 |
| 2 | D1 的正文淘汰建议违反“归档文章仍保留”的契约；“永久失败”过度 | 对照设计哲学属实 | D1 拆为“保留全部正文的存储优化”与“保留期限（产品决策）”，后者列为决策 7；措辞收窄 |
| 3 | G1 遗漏正文库 `CONTENT_MIGRATOR`；“所有 Windows 库”过度 | 读码属实；8 个迁移哈希本地复核 | G1 覆盖两套 migrator，措辞收窄 |
| 4 | B2 建议在有 HTTP charset 时仍会重复解码 | 探针证实，且当前代码在 HTTP 与 XML 都声明 GBK 时同样乱码 | B2 重写并扩大范围；新增红测试 `http_and_prolog_both_gbk_feed_keeps_its_text`；建议改为单一解码边界（RFC 7303 §3.2） |
| 5 | AppState 红测试的 barrier 超时会静默放行 | 读码属实 | 超时改为测试失败；注明夹具绑定当前端口 |
| 6 | F1 按报告自己的定义应为 P1 | `auth.rs:88` 锁定先于密码校验，属实 | 上调为 P1 并写明前提，移入第一批 |
| 7 | D7/G2/G3/G6 的 V 缺少证据；§7 把 app wasm clippy 的失败写成 1455；“明显高于同类项目”无依据 | 属实 | 补配置与原文证据，静态事实 V、后果 I；更正 §7；删除比较性措辞 |
| 8 | F3 把“链接被保留”写成了“内容会被执行” | 属实 | 保留行为 V，执行结果 U |
| 9 | A1 的序号方案需与正文写入共用栅栏，否则旧正文仍可后写 | 读码属实 | A1 建议改为“每个写入步骤在同一写栅栏内检查序号”，并要求分库夹具验收 |
| 10 | 搜索修复缺实际批量写入断言；浏览器 304 路径无动态验收 | 属实 | 搜索测试补写入与剩余未读集合；新增 wasm 用例并在本地 headless Chrome 跑出红→绿 |
| 11 | “`--title` 实际无效”过于绝对 | 属实 | B6 改为条件性表述 |
| 12 | 304 注释把丢失 Last-Modified 等同于无条件请求 | 属实（ETag 仍会发送 `If-None-Match`） | 修正代码与测试注释；`28c0441` 的提交说明保留原文，未改写历史 |

评审对外部事实的核对结论：RFC 9110/9111 引用成立（并指出 200 带 ETag 时 304 必须带 ETag，保留缺省 ETag 属兼容处理）；RFC 9110 §10.1.5 只能支持“SHOULD 发送 UA”，不能证明特定站点拒绝无 UA 请求（B3 已改为 U）；WHATWG Fetch 对含凭据 URL 抛 `TypeError` 成立；sqlx 0.8.6 按字节 SHA-384 且不归一化换行成立（本报告另在本地 cargo 注册表中核对了锁定版本源码，并与本地 CRLF 样本一致）；`windows-latest` 沿用 Git for Windows 的 CRLF 默认（本报告另行核对了安装器默认值 `CRLFAlways`）；`localStorage` 5 MiB 为标准建议值；clap、ammonia、feed-rs 三项判断成立。

## 10. 修复进度

审计之后在同一分支上逐项修复，每项单独提交、配红→绿测试，每批送 `codex-reviewer`（gpt-6-astra，max）独立评审，意见逐条核实后修复。验证命令与结果见 [交接记录](handoffs/2026-10-09-audit-fixes.md)。“与建议的差异”只列实现偏离第 3 节建议之处。

| 发现 | 状态 | 提交 | 与建议的差异 / 备注 |
| --- | --- | --- | --- |
| A1 | 已修复（进程内） | `83e2136`、`e93b968`、`92eca6c` | 在 `RefreshService` 内按订阅加锁，抓取与提交同在锁内；`refresh_all` 在锁内重读目标；添加订阅的预抓取结果按抓取序号判断是否已被取代；落库途中被取消按已提交登记。未做跨进程的 `refresh_seq` 列 |
| A2 | 已修复 | `abc2f8f`、`5648b32` | 端口 `save` 换成原子 `update(mutation)`，不提供整份写入；测试改为不依赖等待时长 |
| A3 | 已修复（补充未提交） | `347890e`、`2418c14` | 写入成功时作废在途查询并重查（补丁回放补不回新满足筛选的条目，已弃用）；查询先于写入完成的顺序在工作区补上，待构建验证 |
| A4 | 已修复 | `e10b99e`、`69eedf2` | 采样触发器为手动刷新、批量已读与全部查询条件；未加分钟级定时器 |
| A5 | 已修复 | `2d1bef5` | 只改核心写入里的清理；读取失败时页面仍按原设计不覆盖无法解析的偏好 |
| B1 | 已修复 | `28c0441` | — |
| B2 | 已修复 | `2e382ea`、`fbe9c8f`、`77a5f1a`、`062f335`、`d1def2e` | 单一解码边界（BOM → HTTP charset → XML 声明 → UTF-8）；不改写原声明，而是在交给 feed-rs 的 XML 文本前补 UTF-8 声明锁定 quick-xml 的编码 |
| B3 | 已修复 | `5cf52f5` | 同时覆盖 WebDAV 与 `rssr-web` 代抓；图片本地化保留自身 UA |
| B4 | 已修复 | `2a4857b` | 站点地址没有 alternate 时留空而不是回退到第一条；无稳定 ID 的条目可能出现一次重复 |
| B5 | 已修复 | `6f00aaa` | 产品决策：保留为仅索引条目 |
| C1 | 已修复 | `e04484c`、`d1b8782` | 未加 `source_hash` 列（避免迁移）：`content_hash` 改为只标识源正文的无碰撞哈希（含文章地址），本地化写回保留它，并要求存储正文仍是本次读到的那份；没有图片换成本地副本时不写回；后台本地化跳过已本地化条目 |
| C2 | 已修复 | `0313d20` | 按尝试计数（发过图片请求才算），每订阅每轮 5 次、180 秒，最多 2 个订阅并发，同一订阅不叠加；以每订阅去重 + 信号量代替单 worker 有界队列；未做“只送新增或正文变化的条目” |
| D1 | 部分 | `364df0d` | 只做了配额错误的明确提示；用量显示、IndexedDB 迁移未做 |
| D3 | 已修复 | `f1592e7` | 每批一次查找表，O(N + k)；差分测试以旧线性实现为对照 |
| D7 | 部分 | `307e31f` | 浏览器模块单测改为 `#[wasm_bindgen_test]`，可在 wasm 上执行；CI 尚未接入 |
| E1 | 已修复 | `bbf09b3`、`bb9e66a` | 由订阅页会话写入全局主题；导入任务归 App 作用域，离开页面不取消 |
| F1 | 已修复 | `0847630`、`2b0b552`、`af3ae73` | 另加已知设备标记（OWASP device cookies），未采用递增延迟；计数不看用户名；表满后新客户端共用溢出计数；F2 的 Argon2 阻塞执行与用户名时序一并处理，名额随阻塞任务持有，已知设备另有名额 |
| F3 | 已修复 | `28f880d`、`861585a`、`ee1daa0`、`5286d30` | `srcset` 逐候选检查；媒体类型按 Fetch 规则取；另修 srcset 解析 `dc8ec81`、`1f197bc` |
| H1 | 已修复 | `5a0c402` | 采用 `--keep-entries`，旧开关隐藏保留 |
| 标题搜索通配符 | 已修复 | `1456b2b` | 审计期间修复（第 2 节） |
| 其余 | 待处理 | — | B6 单独一轮；G1、“内网背景图”与 D6 待确认；D2、D4、D5、E2、E3、G2–G6、H2、H3、B7 与 F2 其余部分未处理 |

## 附录 A：确定性红测试

运行方式（任选其一）：

```bash
cargo test -p rssr-infra --test test_audit_20261009_refresh_invariants --locked -- --ignored
cargo test -p rssr-application --lib --locked -- --ignored concurrent_narrow_field_updates
cargo test -p rssr-app --locked -- --ignored in_flight_query_cannot_resurrect
```

| 测试 | 不变量 | 当前失败表现 |
| --- | --- | --- |
| `unchanged_source_refresh_keeps_localized_body` | 源未变的刷新不丢弃派生的本地化正文 | 正文回到远端图片地址 |
| `the_fetch_that_started_last_owns_the_stored_result` | 同一订阅的存储结果属于最后开始的那次抓取（与修复策略无关） | ETag 为 `v1-older`，正文为旧版 |
| `feed_fetch_identifies_the_client` | 订阅抓取携带 User-Agent | 请求头无 `user-agent` |
| `prolog_declared_gbk_feed_keeps_its_text` | 仅在 XML 声明里给出编码的 feed 正确解码 | 标题为“锟斤拷…” |
| `http_and_prolog_both_gbk_feed_keeps_its_text` | HTTP 与 XML 都声明同一非 UTF-8 编码时只解码一次 | 标题为“涓枃鏍囬…” |
| `atom_links_are_selected_by_relation_not_position` | Atom 站点与文章链接按 `rel` 选择 | 站点地址为 feed 自身地址 |
| `concurrent_narrow_field_updates_do_not_lose_each_other` | 两个窄字段更新并发时都被保留 | 工作区更新丢失 |
| `in_flight_query_cannot_resurrect_flags_overwritten_by_a_successful_write` | 写入成功后，较早开始的查询不能回退已读状态 | 已读条目重新出现在 UnreadOnly 列表 |

修复对应缺陷后，多数测试去掉 `#[ignore]` 即成为常规回归测试。两个例外：`concurrent_narrow_field_updates_do_not_lose_each_other` 的夹具绑定当前 load/save 端口，端口改为原子操作后要换成新端口的夹具；同源刷新测试对 single-flight、串行化与提交序号三种策略都成立，但跨进程与步骤交错的保证需另写分库夹具（见 A1）。

## 附录 B：探针结果（未提交）

以下为一次性探针的观察结果，代码未进入仓库：

- YouTube 频道形态 Atom：`entries = 0`，`site_url = https://www.youtube.com/feeds/videos.xml?channel_id=…`。
- 仅含标题与链接的 RSS 条目：`entries = 0`。
- Atom 条目首个链接为 `rel="replies"` 时，文章地址为评论页。
- 订阅名“自定义名”在元数据更新后变为源标题。
- `sanitize_reader_html` 保留 `<a href="data:text/html;base64,…">`。

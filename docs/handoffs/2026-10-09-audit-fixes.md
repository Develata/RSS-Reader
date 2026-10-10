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
  - 第三轮独立评审的处理：`af3ae73`（F1/F2）、`d1b8782`（C1 派生正文）、`92eca6c`（A1 取消窗口）、`5648b32`（A2 测试）；A3 的第二种完成顺序已改未提交（见“结果”）
- 相关 tag / release：N/A
- 状态：`in_progress`（本地验证；未推送、未开 PR。最终边界验证因本机内存与 C 盘空间耗尽暂停，见“结果”）

## 工作摘要

按 [工程审计报告（2026-10）](../engineering-audit-2026-10.md) 逐项修复已确认的缺陷，每项单独提交并配红→绿测试；审计时提交的 8 个 `#[ignore]` 红测试已全部解除忽略并通过。每批修复都送 `codex-reviewer`（gpt-6-astra，max）独立评审，意见逐条核实后修复，共三轮。进度总表见报告第 10 节。

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
- C1：`content_hash` 只标识源正文（标题、纯文本、HTML、文章地址各带长度前缀，带版本；地址变了，按旧地址解析的相对图片就不再对应）；本地化写回只替换正文、保留源哈希，并要求存储正文仍是本次本地化读到的那份（防止同一份源的两个本地化任务互相覆盖）；一张图都没换成本地副本时不写回；刷新在源哈希未变时保留存储的正文，后台本地化跳过已本地化的条目。
- C2：后台本地化按尝试计数：为一篇文章发过图片请求（不论成败、超时）计一次，正文已派生或根本没发请求（无图、超预算）的不计；每个订阅每轮最多 5 次尝试、180 秒；同时最多 2 个订阅，同一订阅已有排队或进行中的一轮时不再追加。
- A1（补充）：刷新在落库途中被取消时按已提交登记抓取序号，更早的预抓取结果不能再覆盖它；显式失败不登记。

### 应用状态与列表

- A2：`AppStateRepository::update` 在一次原子读改写里应用修改（SQLite 单个 IMMEDIATE 事务，Web 一次 `store.update`），端口不再提供整份写入。
- A5：删除订阅、导入配置不再因应用状态无法解析而整体失败。
- A3：已读/收藏写入成功时若有在途列表查询，作废它并递增 `requery_revision` 重查；查询先于写入完成时，写入的条目不在列表里且写入后可能符合筛选也重查（这一补充尚未提交）。
- A4：归档分界的“现在”只在挂载、手动刷新、批量已读与查询条件变化时采样。

### 正文消毒

- F3：`data:` 只在 `img`/`source` 的 `src`/`srcset` 且媒体类型为完整 `image/<subtype>` 时保留；`srcset` 逐候选检查；媒体类型按 Fetch 规则从含 query 的序列化 URL 中取。
- srcset 按 HTML 标准的描述符状态机切分并校验描述符，无效候选既不被选中也不保留。

### Web 部署服务

- F1：登录失败按 TCP 对端地址计数（信任代理时取 `X-Forwarded-For` 最右项）；成功登录过的浏览器持签名的已知设备标记，单独计数。计数不看用户名（否则只有真实用户名会被限速，泄露用户名是否存在）；计数表封顶 4096 项，表满后新的客户端地址共用一份溢出计数，已知设备照常单独计数。
- F2（部分）：Argon2 在阻塞线程池里执行，对任何用户名都执行一次；并发名额随闭包移入阻塞线程，客户端断开不会提前归还名额；已知设备另有一个名额，其他来源占满时仍能校验。

### Web 本地存储

- D3：浏览器批量 upsert 每批建一次查找表（去重键、外部 ID 计数、旧 URL 身份、正文位置），O(k·N) 降为 O(N + k)；差分测试用旧的线性实现做对照。
- D1（部分）：`localStorage` 配额超限时给出明确提示（`QuotaExceededError` / `NS_ERROR_DOM_QUOTA_REACHED`），不再只显示 `JsValue` 调试串。用量显示与 IndexedDB 迁移未做。
- 浏览器模块的单元测试改为 `#[wasm_bindgen_test]`：此前的 `#[test]` 在 wasm32 上从未执行。

### CLI 与界面

- H1：`remove-feed --keep-entries`。
- E1：JSON 配置导入替换设置后，由 App 作用域的任务把新设置写入全局主题；离开订阅页不再取消导入。

## 验证与验收

### 自动化验证

- 第一批边界（`861585a`）：`cargo test --workspace --locked` 378 passed / 0 failed / 7 ignored；三个浏览器契约 harness 29/29、20/20、3/3。
- 第一轮评审处理后的边界（`062f335`）：`cargo test --workspace --locked -j 2` 393 passed / 0 failed / 5 ignored；三个浏览器 harness 29/29、20/20、3/3；`cargo clippy --workspace --all-targets -- -D warnings`、`cargo clippy -p rssr-app --target wasm32-unknown-unknown -- -D warnings`、`cargo fmt --all --check` 通过。
- 第二批与第二轮处理：逐提交运行受影响 crate 的测试与 clippy（原生与 wasm），浏览器端改动跑对应 harness。
- 第三批与第三轮处理（逐提交，实测）：`rssr-web` 28 passed / 0 failed / 1 ignored；`rssr-application` 58 passed；`rssr-infra` lib 65 passed、`test_audit_20261009_refresh_invariants` 9/9、`test_feed_refresh_flow` 14/14、`test_native_outbound_requests` 3/3、`test_app_state_persistence` 1/1；`rssr-app` 的 bootstrap 测试 16/16；`cargo clippy -p rssr-infra -p rssr-app --all-targets -- -D warnings` 原生与 wasm32 通过，`rssr-web` clippy 通过，`cargo fmt --all --check` 通过。浏览器模块测试在 `307e31f` 时于 wasm 上 14 passed（node），之后的哈希改动尚未在 wasm 上复跑。
- 变异验证（实测，恢复后复跑通过）：C2 计数（把无图文章也计数 → 0 次请求，失败）；F1/F2 三个新测试（名额留在调用方、表满不记录、设备共用名额 → 各自失败）；C1 三个新测试（去掉正文条件、哈希忽略地址、仅规范化也写回 → 各自失败）。A1、A2、A3 新测试的变异验证尚未运行（构建被内存不足阻断）。
- 最终边界（全工作区测试、三个浏览器 harness、全工作区 clippy、wasm lib 测试）：尚未运行，原因同上。
- 红→绿（实测）：8 个审计红测试解除忽略后通过；A2 在 SQLite（改成延迟事务即失败）与浏览器（改成先读后写即丢更新）两端各有一个红测试；E1 卸载测试在任务归页面作用域时失败；srcset、`data:` 媒体类型、声明固定等用例在修复前的实现上失败。

### 手工验收

- 未执行（无布局、焦点或导航改动；界面行为由 VirtualDom 会话测试覆盖）。

## 结果

- 三批修复已提交；第三轮评审意见除 A3 外均已提交（A3 改动已在工作区，`rssr-app` 编译在 os error 1455 上失败：C 盘只剩不到 1 GB、页面文件 40 GB、可用提交内存约 1.4 GB）。恢复后依次：A3 测试与提交、A1/A2/A3 变异验证、最终边界验证，再送第四轮独立评审。
- 未合并：第三轮评审的处理与第三批尚未经独立复审。

## 风险与后续事项

- B4：没有稳定 ID 的条目若所选链接因新规则改变，去重键变化，下一次刷新出现一次重复。
- B2：只识别 ASCII 兼容字节流中的 XML 声明；无 BOM 的 UTF-16/UTF-32 不做探测。
- A1：只协调同一进程内的刷新；CLI 与图形界面同时刷新同一个数据库、或 Web 多个标签页仍可能交错。两端的刷新都逐订阅落盘（浏览器端用默认的空批次钩子，代价见 D2）。刷新在落库途中被取消时按已提交处理：若那次落库其实没有发生，添加订阅时预抓取的首刷结果会被丢弃，下一次刷新重新抓取。
- C1：升级后每个订阅下一次刷新会因哈希算法变化重写一次当前 feed 中条目的正文（本地化正文此前每次刷新都会被冲掉，这里只是最后一次）。条目地址在某次刷新中缺失时（存储沿用旧地址），源哈希也会变化，派生正文被重算一次。
- C2：尝试窗口按 feed 顺序：图片持续失败的文章每轮都会占用尝试次数，直到源正文变化，排在后面的文章可能一直轮不到后台本地化（阅读时的按需本地化不受影响）。部分图片成功的文章算作已本地化，后台不再补其余图片，打开阅读页时按需补。排队中的旧条目列表不会被新一轮替换，新增条目留到下一次刷新。“只把新增或正文变化的条目送去本地化”未做。
- D1：配额满时只给出提示；正文仍在 `localStorage` 一片里，接近上限时每次带正文变化的刷新都会失败，删除订阅才能释放空间。
- E1：导入读回设置与设置页保存极端交错时（读回完成到 App 任务被轮询之间有保存落地），全局主题可能停在导入时的值，直到之后重新加载设置（打开设置页）或重启；未加版本校验。
- F1：没有已知设备标记的新浏览器在攻击期间仍会被限速；反向代理后未开启信任代理头时，所有新客户端共用一份计数；计数表满时所有未单独计数的新客户端共用溢出计数，可能被一起限速。被盗的设备标记可以重放并耗尽该设备自己的计数。
- 待办：B6 单独一轮；G1（迁移校验和）、“内网背景图”与 D6（Unicode 折叠，依赖 G1 的迁移决定）需要你确认；D2、D4、D5、D7、E2、E3、G2–G6、H2、H3、B7、F2 其余部分（`/logout` 用 GET、CSP）未处理。

## 给下一位 Agent 的备注

- 入口：报告第 10 节进度表；每项修复的提交信息写明了机制、取舍与测试。
- 独立评审原文在会话 scratchpad 的 `review/batch1-review.md`、`review/round2-review.md`、`review/round3-review.md`（不入库）；处理结论记在本记录与报告第 10 节。

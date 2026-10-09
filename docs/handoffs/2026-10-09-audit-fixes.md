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
- 相关 tag / release：N/A
- 状态：`validated`（本地验证；未推送、未开 PR）

## 工作摘要

按 [工程审计报告（2026-10）](../engineering-audit-2026-10.md) 逐项修复已确认的缺陷，每项单独提交并配红→绿测试；审计时提交的 8 个 `#[ignore]` 红测试已全部解除忽略并通过。每批修复都送 `codex-reviewer`（gpt-6-astra，max）独立评审，意见逐条核实后修复。进度总表见报告第 10 节。

## 影响范围

- 模块：
  - `rssr-domain`：`AppStateRepository` 端口（`save` 换成原子 `update`）
  - `rssr-application`：`refresh_service.rs`（按订阅串行、抓取序号）、`subscription_workflow.rs`、`app_state_service.rs`
  - `rssr-infra`：`feed_body.rs`、`feed_normalization.rs`、`xml_declaration.rs`、`fetch/client/feed_http.rs`、`subscription_probe.rs`、`composition.rs`、`config_sync/webdav.rs`、`html/reader.rs`、`html/live_display.rs`、`db/app_state_repository.rs`、`db/entry_repository.rs`、`application_adapters/{mutations.rs,non_refresh.rs,browser/adapters/app_state.rs}`
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
- C1：`content_hash` 只标识源正文（标题、纯文本、HTML 各带长度前缀，带版本）；本地化写回只替换正文、保留源哈希；刷新在源哈希未变时保留存储的正文，后台本地化跳过已本地化的条目。

### 应用状态与列表

- A2：`AppStateRepository::update` 在一次原子读改写里应用修改（SQLite 单个 IMMEDIATE 事务，Web 一次 `store.update`），端口不再提供整份写入。
- A5：删除订阅、导入配置不再因应用状态无法解析而整体失败。
- A3：已读/收藏写入成功时若有在途列表查询，作废它并递增 `requery_revision` 重查。
- A4：归档分界的“现在”只在挂载、手动刷新、批量已读与查询条件变化时采样。

### 正文消毒

- F3：`data:` 只在 `img`/`source` 的 `src`/`srcset` 且媒体类型为完整 `image/<subtype>` 时保留；`srcset` 逐候选检查；媒体类型按 Fetch 规则从含 query 的序列化 URL 中取。
- srcset 按 HTML 标准的描述符状态机切分并校验描述符，无效候选既不被选中也不保留。

### Web 部署服务

- F1：登录失败按 TCP 对端地址计数（信任代理时取 `X-Forwarded-For` 最右项）；成功登录过的浏览器持签名的已知设备标记，单独计数，远端攻击者锁不住它；未知用户名不分配计数键，计数表封顶。
- F2（部分）：Argon2 在阻塞线程池里执行（两个并发许可），对任何用户名都执行一次，消除用户名时序差。

### CLI 与界面

- H1：`remove-feed --keep-entries`。
- E1：JSON 配置导入替换设置后，由 App 作用域的任务把新设置写入全局主题；离开订阅页不再取消导入。

## 验证与验收

### 自动化验证

- 第一批边界（`861585a`）：`cargo test --workspace --locked` 378 passed / 0 failed / 7 ignored；三个浏览器契约 harness 29/29、20/20、3/3。
- 第一轮评审处理后的边界（`062f335`）：`cargo test --workspace --locked -j 2` 393 passed / 0 failed / 5 ignored；三个浏览器 harness 29/29、20/20、3/3；`cargo clippy --workspace --all-targets -- -D warnings`、`cargo clippy -p rssr-app --target wasm32-unknown-unknown -- -D warnings`、`cargo fmt --all --check` 通过。
- 第二批与第二轮处理：逐提交运行受影响 crate 的测试与 clippy（原生与 wasm），浏览器端改动跑对应 harness。最终边界数字见下一次更新。
- 红→绿（实测）：8 个审计红测试解除忽略后通过；A2 在 SQLite（改成延迟事务即失败）与浏览器（改成先读后写即丢更新）两端各有一个红测试；E1 卸载测试在任务归页面作用域时失败；srcset、`data:` 媒体类型、声明固定等用例在修复前的实现上失败。

### 手工验收

- 未执行（无布局、焦点或导航改动；界面行为由 VirtualDom 会话测试覆盖）。

## 结果

- 两批修复与两轮评审意见均已处理，可合并；最终独立评审结论见报告第 10 节。

## 风险与后续事项

- B4：没有稳定 ID 的条目若所选链接因新规则改变，去重键变化，下一次刷新出现一次重复。
- B2：只识别 ASCII 兼容字节流中的 XML 声明；无 BOM 的 UTF-16/UTF-32 不做探测。
- A1：只协调同一进程内的刷新；CLI 与图形界面同时刷新同一个数据库、或 Web 多个标签页仍可能交错。
- C1：升级后每个订阅下一次刷新会因哈希算法变化重写一次当前 feed 中条目的正文（本地化正文此前每次刷新都会被冲掉，这里只是最后一次）。
- E1：导入读回设置与设置页保存极端交错时（读回完成到 App 任务被轮询之间有保存落地），全局主题可能短暂停在导入时的值，打开设置页或重启即恢复；未加版本校验。
- F1：没有已知设备标记的新浏览器在攻击期间仍会被限速；反向代理后未开启信任代理头时，所有新客户端共用一份计数。
- 待办：C2、D1、D3、D5、D6 等剩余项；B6 单独一轮；G1（迁移校验和）与“内网背景图”两项需要你确认。

## 给下一位 Agent 的备注

- 入口：报告第 10 节进度表；每项修复的提交信息写明了机制、取舍与测试。
- 独立评审原文在会话 scratchpad 的 `review/batch1-review.md`、`review/round2-review.md`（不入库）；处理结论记在本记录与报告第 10 节。

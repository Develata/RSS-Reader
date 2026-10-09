# 2026-10 工程审计发现的修复

- 日期：2026-10-09
- 作者 / Agent：Claude（Opus 5.5）
- 分支：`audit/2026-10-09`（worktree `target/audit-20261009/worktree`）
- 当前 HEAD：见“相关 commit”的最后一个提交
- 相关 commit：第一批 `6f00aaa`、`5cf52f5`、`2e382ea`、`2a4857b`、`28f880d`、`5a0c402`、`bbf09b3`、`dc8ec81`、`861585a`
- 相关 tag / release：N/A
- 状态：`validated`（本地验证；未推送、未开 PR）

## 工作摘要

按 [工程审计报告（2026-10）](../engineering-audit-2026-10.md) 逐项修复已确认的缺陷，每项单独提交并配红→绿测试；审计时提交的 `#[ignore]` 红测试随修复解除忽略。进度总表见报告第 10 节。本记录按批次更新。

## 影响范围

- 模块：
  - `crates/rssr-infra`：`feed_body.rs`、`feed_normalization.rs`、`fetch/client/feed_http.rs`、`subscription_probe.rs`、`composition.rs`、`config_sync/webdav.rs`、`html/reader.rs`、`html/live_display.rs`
  - `crates/rssr-web/src/proxy.rs`、`crates/rssr-cli/src/main.rs`
  - `crates/rssr-app`：`pages/feeds_page/*`、`ui/runtime/feeds.rs`
- 平台：Windows / macOS / Linux / Android / Web（解码、链接选择、正文消毒与 srcset 两端共用）
- 额外影响：`docs/user-guide.md`、`docs/design/frontend-command-reference.md`

## 关键变更（第一批）

### 订阅解析与抓取

- B5 `6f00aaa`：既无摘要也无正文的条目保留为仅索引条目（产品决策），列表显示标题、阅读页提供原文链接，不再整条丢弃（YouTube 频道等）。
- B3 `5cf52f5`：订阅刷新、原生订阅发现、WebDAV 与 `rssr-web` 代抓统一发送 `RSS-Reader/<版本> (+仓库地址)`；版本取发布构建的 `RELEASE_TAG`。原生订阅发现改用刷新同款客户端配置（UA、30 秒总时限、10 秒连接时限）。
- B2 `2e382ea`：`read_feed_text` 作为唯一解码边界，按 RFC 7303 §3.2 依次采用 BOM、HTTP charset、XML 声明（XML 1.0 §4.3.3）、UTF-8；解码后把非 UTF-8 声明改写为 UTF-8，解析器不再二次解码。
- B4 `2a4857b`：站点地址只取 alternate（或 RSS 无 rel 的 `<link>`），没有时留空；条目地址优先 alternate，否则沿用第一条链接。

### 正文消毒

- F3 `28f880d` + `861585a`：`data:` 只在 `img`/`source` 的 `src`/`srcset` 且媒体类型为 `image/*` 时保留；只检查承载 URL 的属性，scheme 用 `Url::parse` 判定。
- `dc8ec81`（修 F3 时发现的既有缺陷）：srcset 按 HTML 标准切分候选，本地化后 `<source srcset="data:…">` 不再在 data URL 的逗号处被切断而显示裂图。

### CLI 与界面

- H1 `5a0c402`：`remove-feed` 默认连同文章删除，`--keep-entries` 保留文章；旧 `--purge-entries` 隐藏保留为兼容空操作。
- E1 `bbf09b3`：JSON 配置导入更新了设置时，读回设置并写入全局主题，主题、自定义 CSS、密度与字号立即生效。

## 验证与验收

### 自动化验证（第一批）

- 逐提交：`cargo test -p rssr-infra --locked -j 3`（861585a：167 passed / 0 failed / 3 ignored，均为剩余审计红测试）、`cargo test -p rssr-web`（21 passed / 1 ignored，既有浏览器驱动用例）、`cargo test -p rssr-cli`（2 + 4 passed）、`cargo test -p rssr-app`（111 passed / 2 ignored）；改动 crate 的 `cargo clippy --all-targets -- -D warnings`；`cargo fmt --all --check`；`cargo check`/`clippy -p rssr-app --target wasm32-unknown-unknown`：全部通过
- 红→绿：`feed_fetch_identifies_the_client`、`prolog_declared_gbk_feed_keeps_its_text`、`http_and_prolog_both_gbk_feed_keeps_its_text`、`atom_links_are_selected_by_relation_not_position` 解除忽略后通过；srcset 缺陷修复前实测输出 `srcset="data:image/png;base64, iVBORw0KGgo= 1x"`
- 边界（`861585a`）：`cargo test --workspace --locked -j 3` 378 passed / 0 failed / 7 ignored（4 个剩余审计红测试 + 3 个既有手动/浏览器用例）
- 浏览器契约（headless Chrome 154）：wasm_refresh_contract_harness 29/29、wasm_subscription_contract_harness 20/20、wasm_config_exchange_contract_harness 3/3

### 手工验收

- 未执行（本批无布局、焦点或导航改动；E1 的界面效果由 VirtualDom 会话测试覆盖）

## 结果

- 第一批可合并；待独立评审结论（见报告第 10 节）。

## 风险与后续事项

- B4 已知效果：没有稳定 ID 的条目（缺 `<id>`/`<guid>`），若所选链接因本次规则改变（首条链接不是 alternate），其去重键会变化，下一次刷新会出现一次重复。只影响不合规 Atom 或无 guid 且条目内有多条链接的 RSS。
- B2：只识别 ASCII 兼容字节流中的 XML 声明；无 BOM 的 UTF-16/UTF-32 按规范本应带 BOM，不做探测。
- 待办：第二批（A1–A4、C1、C2、D1、D3、D5、D6、F1）、B6 一轮、G1 与“内网背景图”两项待你确认。

## 给下一位 Agent 的备注

- 入口：报告第 10 节进度表；每项修复的提交信息写明了机制与测试。
- 剩余审计红测试：`rg -n 'audit-2026-10-09' crates`。

# 审计修复后的反向复核与修补

- 日期：2026-10-01
- 作者 / Agent：ChatGPT
- 分支：fix/post-audit-repair
- 当前 HEAD：544ec976（handoff 创建前）
- 相关 commit：PR #10；最终 squash commit pending
- 相关 tag / release：N/A
- 状态：`draft`

## 工作摘要

对已合并的 #2–#6 进行多轮反向 review，不再只确认原 bug 是否消失，而是专门检查修复本身引入的事务边界、失败语义、历史数据升级、Web 存储放大和跨平台语义分叉；在问题集合收敛后集中形成 PR #10。

## 影响范围

- 模块：
  - `crates/rssr-application`：删除与配置替换用例边界
  - `crates/rssr-infra`：SQLite / BrowserStore mutation、entry identity、OPML、refresh metadata
  - `crates/rssr-app`：native 启动清理
  - `crates/rssr-cli`：CLI 启动清理
  - native / wasm contract regression
- 平台：
  - Windows / macOS / Linux
  - Android
  - Web
  - CLI
- 额外影响：
  - 无 schema migration
  - 无产品功能扩张
  - 无新依赖

## 关键变更

### 删除事务职责收敛

- `SubscriptionRemovalPort` 成为删除持久化副作用的唯一所有者。
- `FeedService` 不再保留语义不完整的 repository fallback 构造路径。
- `SubscriptionWorkflow` 不再在成功提交后重复清理 last-opened app state，避免“删除实际成功但第二次 cleanup 失败，对外返回失败”。
- Native / Web subscription contract harness 使用与生产装配相同类型的 removal adapter。

### 正文 purge 的可重试收敛

- index/content 仍保持两个 SQLite 文件，不伪造跨库事务。
- immediate content cleanup 失败时保留 tombstone。
- native GUI / Android 与 CLI 启动后，重试清理“`is_deleted=1` 且 index entries 已为空”的正文。
- 用“entries 已空”区分 purge 删除与 `purge_entries=false`，避免后者在重启后被误清。

### 配置替换的并发边界

- `ConfigReplacementPort` 从接收锁外预计算 delta 改为接收 desired state。
- SQLite 在 `BEGIN IMMEDIATE` 内读取当前 feeds/settings、计算 removals 并提交 replacement。
- Browser 在 BrowserStore Web Lock transaction 内计算同样的 diff。
- replacement 的 title/folder 是精确 desired state；Native/Web 对空白值归一语义保持一致。
- Web 只在 flags/content/app-state slice 实际变化时发布相应 slice，避免配置导入无条件重写全文缓存。

### GUID 修复的旧库过渡

- #5 已修 future parser provenance，但旧数据库可能已有 `external_id == dedup_key == current URL` 的受影响条目。
- Native/Web upsert 在 incoming 为 32/40/64 位 hex source GUID 时，仅识别这一种旧 bug 的精确遗留形态并原地 promotion。
- entry id 不变，因此 read/starred/first-seen/content 状态自然保留。
- 不对旧链接已经变化后的历史重复做标题/日期模糊合并，避免误合并不同文章。

### refresh / OPML 边界

- native `update_feed_metadata` 与已有 fetch-state/index fence 一致，要求 feed 仍 `is_deleted=0`。
- OPML 根元素关闭后只允许 XML whitespace、comment、PI；其它 material 直接报错且 import 零写入。

## 验证与验收

### 自动化验证

- PR #10 CI #125（早期 head）：部分通过；发现 rustfmt 差异与一处 Clippy `bool_assert_comparison`，均已修复；该 run 后续 job 被新提交 supersede/cancel。
- PR #10 CI #134（最新 repair head）：运行中。
- PR #10 Pages #55：运行中。
- 新增/更新回归：
  - native GUID legacy promotion 保留 read/star/content
  - Browser GUID legacy promotion
  - content purge 失败后 retry
  - `purge_entries=false` 不被 startup cleanup 误清
  - config replacement 在 backend lock 内决定 removals
  - deleted feed 拒绝 late metadata
  - OPML trailing material 零写入
  - removal exact-once delegation

### 手工验收

- Android 实机：未执行；依赖 PR CI 的 Android build smoke，交互层本 PR 无新增 UI 行为。
- Desktop GUI：未单独手工执行；等待完整 CI/UI acceptance。
- Web：等待 PR CI 中 wasm contract / Web UI acceptance。

## 结果

- 当前 PR #10 尚不可视为完成交付，必须等待最新 CI / Pages 全绿。
- 全绿后可 squash merge；合并前更新本 handoff 为 `validated`。

## 风险与后续事项

- `EntryQuery.feed_ids` 极大时仍可能形成超过 SQLite bind limit 的 `IN (...)`；这是独立 P3 批查询设计问题。
- refresh 存在 delete → 同 URL re-add 的 ABA generation 边界；正确修复应引入 generation/token fence，不能在本修补 PR 中用启发式替代。
- GUID promotion 只处理可证明的“旧 row 仍使用当前 URL 作为双 identity”情形；旧版本中 link 已发生变化而形成的历史 duplicate 无法安全自动判定，不做模糊合并。
- content cleanup retry 基于 soft-delete + index entries 已空；如果 cleanup 失败后该 feed 在下一次 retry 前被重新激活，旧 content 不会被该 tombstone GC 路径删除，应在未来 cache hygiene 设计中继续评估。
- `FeedRepository::upsert_subscriptions` trait 仍有逐条默认实现；生产 SQLite/Browser override 为原子 batch，但 contract 本身没有类型层强制 atomicity，属于低优先级架构债务。

## 给下一位 Agent 的备注

- 删除/配置事务入口：`crates/rssr-application/src/persistence_mutation.rs`
- Native mutation：`crates/rssr-infra/src/application_adapters/mutations.rs`
- Browser mutation：`crates/rssr-infra/src/application_adapters/browser/adapters/mutations.rs`
- GUID transition：`crates/rssr-infra/src/db/entry_repository.rs` 与 `browser/state/entries.rs`
- 合并前先确认 PR #10 最新 head 的 CI、Pages 均为 success，并更新本文件验证状态。

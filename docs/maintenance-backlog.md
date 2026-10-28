# 当前维护待办

核对日期：2026-10-28；代码基线：main `da8521e23b16079294dd43f4cfe9a1f7d5024cdc`。本页记录经当前源码核对后仍值得跟进的事项。发布和 CI 状态见[维护状态](./current-status.md)，逐次执行结果见[本轮交接](./handoffs/2026-10-28-historical-followup-audit.md)。

本轮扫描了 147 份历史 handoff，优先核对架构、数据迁移、资源边界与发布。以下是这一轮的处置结果，不是对所有历史段落、所有用户路径的完整结案。源码支持、历史实测、本轮实测必须分开；缺少当前复现时不把推断升级成新缺陷。

## 当前待办

| ID | 顺序 / 类型 | 当前事实与边界 | 下一步与完成标准 |
| --- | --- | --- | --- |
| M01 | 优先复现 / HTML 解析性能 | 当前 [parse_budget](../crates/rssr-infra/src/html/parse_budget.rs) 仍明确记录 Marker 残留、单标签属性去重以及块内拒绝延迟的盲区；树构建预算不构成全链路复杂度证明。历史证据见[正文预算](./handoffs/archive/2026-10/2026-10-20-reader-parse-budget.md)。本轮未重跑攻击形状，旧耗时不当作当前测量。 | 用隔离进程、确定输入和明确超时复现剩余形状，记录时间、内存和降级结果；先比较上游修复或局部防护方案，再决定实现，保留正常正文与 MathML 回归。不能仅扩大超时或关闭消毒。 |
| M02 | 优先测量 / 后台图片队列 | [native_host](../crates/rssr-infra/src/local_backend/native_host.rs) 已限制每轮 5 次尝试、180 秒、2 个订阅并发，且同订阅去重；仍按候选顺序处理，失败的前几篇可反复占预算。不同订阅等待时持有各自正文列表；[refresh_service](../crates/rssr-application/src/refresh_service.rs) 尚未只生成新增或变更条目的候选。 | 用“前 5 篇图片失败、后续可成功”和多订阅排队夹具，测请求次数、后续文章是否得到处理及保留字节；先论证轮转、候选收敛或总队列预算。保持 generation / 源正文校验，不另建业务状态镜像。 |
| M03 | 发布前 / 正式产物身份 | Windows / Android 测试宿主验收已通过，但 Android 为 debug 外壳；不能替代正式签名 APK/AAB 的安装和覆盖升级。v0.2.1 尚未发布。 | 按[发布草稿](./releases/v0.2.1-draft.md)核对最终源码、版本、证书、公开附件哈希与升级。四平台 Web 服务包通过也不等于 GUI 或 Android 正式包通过。 |
| M04 | 专项验收 / 旧库导入 | 已有 Windows 正式旧版本导入及 v0.2.0 升级证据；本轮手机新建测试库，没有验证 Android 正式旧双库导入。早期 v0.1.0–v0.1.6 的真实官方库、macOS 数据目录搬迁、超大库资源占用仍不能由合成夹具推定。 | 在隔离副本上逐一记录旧包与数据库身份、导入报告、双向数据对照、源文件保留、磁盘占用和中断结果。未发布 schema 0008/0009 不加入支持范围。依据[升级验收](./testing/native-upgrade-verification.md)与[架构决策](./design/v0.2-architecture.md)。 |
| M05 | 测量后决定 / SQLite 写入成本 | [刷新 upsert](../crates/rssr-infra/src/store/refresh.rs) 对现有索引行仍更新字段与 `updated_at`；[基线索引](../crates/rssr-infra/src/store/migrations/0001_baseline.sql)保留多组历史索引。不能仅凭名称认定某个索引无用。 | 用生产 SELECT 的 `EXPLAIN QUERY PLAN` 和真实规模夹具测查询、WAL 与前台写等待。只有收益和行为保持得到证明后，才讨论无变化写入优化或新增迁移删索引；不改已发布迁移。 |
| M06 | 持续维护 / 上游与本地补丁 | [vendor/url/PATCH.md](../vendor/url/PATCH.md) 的本地修复仍接入。2026-10-28 读取到 [rust-url #1163](https://github.com/servo/rust-url/issues/1163) 为 open，上游标签列表首项仍为 v2.5.8；[feed-rs #301](https://github.com/feed-rs/feed-rs/issues/301) 也为 open。此状态会变化。 | 更新依赖时重新核对修复是否进入发布版；移除 url 补丁前按 PATCH.md 跑上游/WPT 与仓库 canary。feed-rs、quick-xml 或 Rust 升级需重核 [feed_structure](../crates/rssr-infra/src/feed_structure.rs) 的估算前提。不能因“上游 issue 关闭”就直接删本地防护。 |
| M07 | 按变更补验 / 真实交互 | 当前 Windows 与一台 Android 的专项通过，不覆盖系统 IME、屏幕阅读器、其他设备、macOS/Linux GUI、所有 SAF/WebDAV/图片手势。Penpot 本轮为代表抽查。 | 依[覆盖矩阵](./testing/release-ui-coverage-matrix.md)选择本次变更涉及的路径，以实际设备和候选包留证；不把编译、模拟视口或旧截图写成真机验收。 |

## 已关闭或已被当前架构替代

下面的“关闭”针对所列旧问题，不扩大为整个模块无缺陷。原始 handoff 保留当时语义，不回写历史测试结论。

| 旧事项 / 来源 | 当前处置与证据 |
| --- | --- |
| 大量 `feed_ids` 超过 SQLite 参数上限；[10-01 修补](./handoffs/archive/2026-10/2026-10-01-post-audit-repair.md) | [entries.rs](../crates/rssr-infra/src/store/entries.rs) 用一个 JSON 参数及 `json_each`；[契约测试](../crates/rssr-infra/tests/test_library_contracts.rs)覆盖 40000 个来源 id。旧的逐项 bind 风险已移除。 |
| 刷新乱序、删除后同 URL 重加的 ABA、清理失败遗留正文；同上 | [refresh.rs](../crates/rssr-infra/src/store/refresh.rs) 使用持久票据与 generation；[票据测试](../crates/rssr-infra/tests/test_library_refresh_tickets.rs)、[原子性测试](../crates/rssr-infra/tests/test_library_persistence_atomicity.rs)覆盖多连接、重新激活和陈旧提交。单库删除事务消除了旧双库的孤儿清理窗口。 |
| `FeedRepository` 批量写入有非原子的默认后备；同上 | [repository.rs](../crates/rssr-domain/src/repository.rs) 将批量端口作为必需实现；[配置交换契约](../crates/rssr-infra/tests/test_library_config_exchange_contract_harness.rs)验证批量写入失败整体回滚。 |
| BrowserStore 正文配额、全片写放大、Web Locks、浏览器 WebDAV 副本；[10-09 审计](./engineering-audit-2026-10.md) | 正式 Web 经服务器 `HttpBackend`；Pages 为共享 SQL 的内存 SQLite、每次加载重建。旧 BrowserStore 已删除，不为旧架构继续设计 IndexedDB 迁移；浏览器设备偏好与设置草稿仍有各自职责。见[架构](./design/v0.2-architecture.md)。 |
| `/feed-proxy` 的真实 304 路径；[10-07 整合](./handoffs/archive/2026-10/2026-10-07-feed-proxy-main-integration.md) | 旧代理入口已退役；现行服务端抓取与 API 契约见[阶段 C 实施计划](./design/v0.2-phase-c-plan.md)。不把旧代理的未验收项转成修复已不存在端点的任务。 |
| SQL 校验和依赖 CRLF/LF；[10-09 修复](./handoffs/archive/2026-10/2026-10-09-audit-fixes.md) | [migrate.rs](../crates/rssr-infra/src/store/migrate.rs) 对当前迁移规范化换行；[import.rs](../crates/rssr-infra/src/store/import.rs)核对已发布旧前缀的换行形式。设备和真实旧库覆盖仍列 M04，不混为一个“全部升级通过”。 |
| `url` 根路径点段反扫、JSON Feed 作者与 Media RSS 复制放大；[10-21 防护](./handoffs/archive/2026-10/2026-10-21-untrusted-url-guard.md) | url 已有本地补丁；当前 [feed_structure](../crates/rssr-infra/src/feed_structure.rs) 已覆盖 JSON 作者继承/比较与 rating 传播，[响应边界测试](../crates/rssr-infra/tests/test_feed_response_limits.rs)有相应反例。旧“JSON 不预检”结论已过时；依赖模型仍需维护，见 M06。 |
| `<picture>` 重复写 data URL 导致正文放大；[C4 记录](./handoffs/archive/2026-10/2026-10-19-v0.2-phase-c4.md) | [live_display.rs](../crates/rssr-infra/src/html/live_display.rs)改写前计算 allowance，逐项累计，超限保留原文；`localized_rewrites_cannot_multiply_one_image_without_bound` 已有回归。data URL 的常规存储成本仍存在，不等于此放大缺陷未修。 |
| C4 Ubuntu CI / 独立复审、C6 四平台包、v0.2.0 发布尚未完成；[10-22 整合](./handoffs/archive/2026-10/2026-10-22-branch-integration.md) | 后续[10-23 整合](./handoffs/2026-10-23-integration-merge-pages.md)与[v0.2.0 发布](./handoffs/2026-10-23-v0.2.0-release.md)已有对应完成证据。新版本不能沿用旧包身份，见 M03。 |
| 无 Android 真机、目录箭头和间距待同步、Windows wasm-opt 阻断 | 后续[原生与 Penpot 专项](./handoffs/2026-10-28-release-readiness.md)及[wasm-opt 修复](./handoffs/2026-10-27-wasm-opt-release-build.md)已补齐所列范围；其余设备与全量设计检查仍按 M07 判断。 |

## 保留的契约与暂缓方向

- 多浏览器 / 多进程并不共享一套 UI 操作序列；返回“结果未知”后重新读取，不自动重放写入。设置补丁、配置指纹、刷新票据各自解决明确的一致性问题，不承诺任意操作全局串行化。现行依据是[架构](./design/v0.2-architecture.md)与[命令契约](./design/frontend-command-reference.md)。
- 旧库导入保留源文件、拒绝未知开发 schema；回退须恢复配套备份。SQLite 单库的写锁争用、创建锁等待上限、强杀后的恢复边界是已记录的设计代价，不作为“待恢复双库”的理由。
- 自写有界 feed 解析器、图片改为内容寻址文件等方案需要独立设计与成本验证；本轮只整理证据，没有批准新增 crate、存储迁移或职责重分配。
- 历史交接里的“未 push / 未发布 / 待评审”先查后续交接和提交，不机械复制到本页；旧绝对路径和旧工具版本不作为当前运行指令。

## 维护方式

新增或关闭事项须补当前源码与可回读证据，注明实际测试平台。优先更新对应长期契约，再更新本页和 handoff。关闭一个已复现输入不代表关闭整个风险类别；暂缓也不等于完成。

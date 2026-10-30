# 当前维护待办

核对日期：2026-10-30（返回策略和 App 设置读取已合并，主题选择宿主评估完成；M01–M07 的其他实测仍以各自记录日期为准）。本页记录经源码核对后仍值得跟进的事项。main、本地候选、发布和 CI 状态统一见[维护状态](./current-status.md)，历史核实见[10 月 28 日记录](./handoffs/2026-10-28-historical-followup-audit.md)。

10 月 28 日扫描了 147 份历史 handoff，优先核对架构、数据迁移、资源边界与发布。以下保留当轮处置结果，不是对所有历史段落、所有用户路径的完整结案。源码支持、历史实测、本轮实测必须分开；缺少当前复现时不把推断升级成新缺陷。

## 当前批次的收口

- 返回策略 `62113c4f` 及后续验收已通过 PR #46 合入 main `6a75fb83`；[实际宿主回执](./handoffs/2026-10-30-native-back-validation.md)保留其 Windows / Android 场景和包身份，不再列为待合并或缺设备。
- App 设置读取已通过 PR #47 合入 `663ef4ee`，原批普通 Rust、Dioxus、正式 Web 与实际 Windows 结果见[实现交接](./handoffs/2026-10-30-shell-settings-lifecycle.md)。随后完成 Windows / Android 主题选择专项，见[宿主评估](./design/native-theme-selector-assessment.md)；不扩大为所有 Android 生命周期通过。前置提取在此收口，不继续扩大 Shell 重构。
- 检查全部 20 个不同分支名后完成旧引用清理、历史资料保全和 5 个旧工作树 / 3 个分支退役，见[分支审计](./handoffs/2026-10-30-branch-consolidation.md)。本批复用现有工作树，当前数量统一见[维护状态](./current-status.md)；主 checkout 的既有改动继续保留。
- PR #38、#40、#46–#48 已合并，不再作为待实现事项。PR #48 收录列表密度退役、主题选择评估与 `.agents` 清理；其 head / main CI 均为 36/36。
- 本批已完成 Android 主题文件导入和实际系统选择器验收，复用现有 Settings 规则；发现的 CSS 开头 BOM 呈现问题已先在旧包复现，再独立修复，见[接续交接](./handoffs/2026-10-30-android-theme-import.md)。独立原生设置面板仍只是下一设计候选，须明确窗口、session、CSS 和回退边界；未批准整页替换、渲染器、数据库或新 crate 迁移。

## 0.3.x 移除旧列表密度参数

用户已批准在 0.3.x 做破坏性更新。当前批次先移除设置控件、预览状态和紧凑 CSS 分支，统一原默认间距；0.2.x 暂时保留 `list_density` 的序列化、存储、API 和 CLI 兼容，UI 保存不发送该字段，读取或保存其他字段也不改写其旧值。不是新的独立模块，也不为它继续提取策略。实现与验收见[交接](./handoffs/2026-10-30-density-retirement.md)。

0.3.x 完成标准：

- 删除 `ListDensity`、`UserSettings.list_density`、`SettingsPatch.list_density` 及补丁合并、序列化和导出入口；CLI 删除 `--list-density`。旧 API/CLI 参数不保留永久别名。
- 更新配置包版本 / schema、示例、契约测试与文档，明确旧配置格式的支持范围。`deny_unknown_fields` 仍有效，不通过全局放宽反序列化来掩盖契约变化。
- 旧资料库在升级边界一次性处理 JSON 中的旧键，保留其余偏好与所有订阅、文章；版本化草稿也须消费旧键或明确淘汰。相应备份、导入、回退验证在 0.3.x 实施时完成，本批没有数据库迁移。
- 删除固定 `data-density="comfortable"` 属性及只为 0.2.x 服务的 UI 草稿过滤 / 兼容测试；主题以 `.app-shell` 与布局选择器为入口。验证更新后的 API、CLI、配置导入导出及 Web/原生宿主。

当前没有升版本、发 tag 或发布。以上是获批的参数退役范围，不扩大为重做 Settings 或全局存储迁移。

## 持续维护事项

| ID | 顺序 / 类型 | 当前事实与边界 | 下一步与完成标准 |
| --- | --- | --- | --- |
| M01 | 具体慢点已修 / 持续防护 | [本轮实现](./handoffs/2026-10-28-parse-image-performance.md)通过本地 html5ever 小补丁修复大标签属性去重，将 Marker 纳入计量，并提前检查长格式化属性。固定进程基准、6 篇快照、MathML 与输出哈希见[当前实测](./testing/evidence/2026-10-28-parse-image/README.md)。单 token 内部仍不可中断，不构成全链路复杂度证明。 | 继续观察正常正文误拒与未覆盖形状；更强取消必须有独立设计和成本证据。新不可信任意语料用有硬超时的隔离工具；当前固定 probe 不承担 fuzz runner。升级依赖时核对本地补丁，不能扩大超时或关闭消毒。 |
| M02 | 本轮调度与载荷修复 / 规模观察 | [图片模块](../crates/rssr-infra/src/local_backend/image_localization.rs)已从宿主组合中拆出，排队只保存指纹等元数据，执行时逐篇读正文；失败/取消也推进轮转。保留 5 次/180 秒/2 并发，增加 64 个订阅/8 MiB 准入上限；本地 HTTP + SQL 测试覆盖前五篇失败、第六篇成功及陈旧写回。[完整边界](./design/reader-processing-budgets.md)。 | 观察大规模订阅的队列饱和、重启和游标淘汰；304 不产生新候选，本轮没有持久续排。仍为含 HTML 的刷新文章计算紧凑候选，以保留失败重试；若收敛为仅变更条目，须先解决重试语义。保持身份检查，不另建业务状态镜像。 |
| M04 | 专项验收 / 旧库导入 | 已有 Windows 正式旧版本导入及 v0.2.0 升级证据；`67e1c2c2` 真机 debug 测试使用新库，v0.2.1 模拟器使用预置 Windows 单库夹具。正式包真机覆盖升级、真实旧 Android 数据目录搬迁和旧双库导入仍未验证。早期 v0.1.0–v0.1.6 的真实官方库、macOS 数据目录搬迁、超大库资源占用也不能由合成夹具推定。 | 在隔离副本上逐一记录旧包与数据库身份、导入报告、双向数据对照、源文件保留、磁盘占用和中断结果。未发布 schema 0008/0009 不加入支持范围。依据[升级验收](./testing/native-upgrade-verification.md)与[架构决策](./design/v0.2-architecture.md)。 |
| M05 | 测量后决定 / SQLite 写入成本 | [刷新 upsert](../crates/rssr-infra/src/store/refresh.rs) 对现有索引行仍更新字段与 `updated_at`；[基线索引](../crates/rssr-infra/src/store/migrations/0001_baseline.sql)保留多组历史索引。不能仅凭名称认定某个索引无用。 | 用生产 SELECT 的 `EXPLAIN QUERY PLAN` 和真实规模夹具测查询、WAL 与前台写等待。只有收益和行为保持得到证明后，才讨论无变化写入优化或新增迁移删索引；不改已发布迁移。 |
| M06 | 持续维护 / 上游与本地补丁 | [url 补丁](../vendor/url/PATCH.md)仍接入；本轮新增 [html5ever 补丁](../vendor/html5ever/RSSR-PATCH.md)。2026-10-28 读取到 rust-url #1163、feed-rs #301、html5ever #796/#801 仍 open，html5ever 0.40.1 发布源仍保留所修热点；这些状态会变化。 | 更新依赖时重新核对修复是否进入发布版；按各补丁说明跑上游与仓库 canary，html5ever 还需 wasm/MathML 回归。feed-rs、quick-xml 或 Rust 升级需重核 [feed_structure](../crates/rssr-infra/src/feed_structure.rs) 的估算前提。不能因“上游 issue 关闭”就直接删本地防护。 |
| M07 | 按变更补验 / 真实交互 | `67e1c2c2` 的 Windows 与一台 Android debug 测试包专项通过，不覆盖系统 IME、屏幕阅读器、其他设备、macOS/Linux GUI、所有 SAF/WebDAV/图片手势。v0.2.1 正式包完整交互未全部重跑；Penpot 本轮为代表抽查。 | 依[覆盖矩阵](./testing/release-ui-coverage-matrix.md)选择本次变更涉及的路径，以实际设备和候选包留证；不把编译、模拟视口或旧截图写成真机验收。 |

## 已关闭或已被当前架构替代

下面的“关闭”针对所列旧问题，不扩大为整个模块无缺陷。原始 handoff 保留当时语义，不回写历史测试结论。

| 旧事项 / 来源 | 当前处置与证据 |
| --- | --- |
| M03：v0.2.1 正式产物身份与发布 | [正式发布记录](./handoffs/2026-10-28-v0.2.1-release.md)固定 tag/source、14 个附件 SHA256、APK/AAB 版本与证书；正式 Windows 单库升级、Android 模拟器正式包覆盖安装及初始化通过。Android 真机签名升级、真实旧库搬迁与更广交互仍留 M04 / M07；不扩大关闭范围。 |
| 大量 `feed_ids` 超过 SQLite 参数上限；[10-01 修补](./handoffs/archive/2026-10/2026-10-01-post-audit-repair.md) | [entries.rs](../crates/rssr-infra/src/store/entries.rs) 用一个 JSON 参数及 `json_each`；[契约测试](../crates/rssr-infra/tests/test_library_contracts.rs)覆盖 40000 个来源 id。旧的逐项 bind 风险已移除。 |
| 刷新乱序、删除后同 URL 重加的 ABA、清理失败遗留正文；同上 | [refresh.rs](../crates/rssr-infra/src/store/refresh.rs) 使用持久票据与 generation；[票据测试](../crates/rssr-infra/tests/test_library_refresh_tickets.rs)、[原子性测试](../crates/rssr-infra/tests/test_library_persistence_atomicity.rs)覆盖多连接、重新激活和陈旧提交。单库删除事务消除了旧双库的孤儿清理窗口。 |
| `FeedRepository` 批量写入有非原子的默认后备；同上 | [repository.rs](../crates/rssr-domain/src/repository.rs) 将批量端口作为必需实现；[配置交换契约](../crates/rssr-infra/tests/test_library_config_exchange_contract_harness.rs)验证批量写入失败整体回滚。 |
| BrowserStore 正文配额、全片写放大、Web Locks、浏览器 WebDAV 副本；[10-09 审计](./engineering-audit-2026-10.md) | 正式 Web 经服务器 `HttpBackend`；Pages 为共享 SQL 的内存 SQLite、每次加载重建。旧 BrowserStore 已删除，不为旧架构继续设计 IndexedDB 迁移；浏览器设备偏好与设置草稿仍有各自职责。见[架构](./design/v0.2-architecture.md)。 |
| `/feed-proxy` 的真实 304 路径；[10-07 整合](./handoffs/archive/2026-10/2026-10-07-feed-proxy-main-integration.md) | 旧代理入口已退役；现行服务端抓取与 API 契约见[阶段 C 实施计划](./design/v0.2-phase-c-plan.md)。不把旧代理的未验收项转成修复已不存在端点的任务。 |
| SQL 校验和依赖 CRLF/LF；[10-09 修复](./handoffs/archive/2026-10/2026-10-09-audit-fixes.md) | [migrate.rs](../crates/rssr-infra/src/store/migrate.rs) 对当前迁移规范化换行；[import.rs](../crates/rssr-infra/src/store/import.rs)核对已发布旧前缀的换行形式。设备和真实旧库覆盖仍列 M04，不混为一个“全部升级通过”。 |
| `url` 根路径点段反扫、JSON Feed 作者与 Media RSS 复制放大；[10-21 防护](./handoffs/archive/2026-10/2026-10-21-untrusted-url-guard.md) | url 已有本地补丁；当前 [feed_structure](../crates/rssr-infra/src/feed_structure.rs) 已覆盖 JSON 作者继承/比较与 rating 传播，[响应边界测试](../crates/rssr-infra/tests/test_feed_response_limits.rs)有相应反例。旧“JSON 不预检”结论已过时；依赖模型仍需维护，见 M06。 |
| `<picture>` 重复写 data URL 导致正文放大；[C4 记录](./handoffs/archive/2026-10/2026-10-19-v0.2-phase-c4.md) | [live_display.rs](../crates/rssr-infra/src/html/live_display.rs)改写前计算 allowance，逐项累计，超限保留原文；`localized_rewrites_cannot_multiply_one_image_without_bound` 已有回归。data URL 的常规存储成本仍存在，不等于此放大缺陷未修。 |
| C4 Ubuntu CI / 独立复审、C6 四平台包、v0.2.0 发布尚未完成；[10-22 整合](./handoffs/archive/2026-10/2026-10-22-branch-integration.md) | 后续[10-23 整合](./handoffs/archive/2026-10/2026-10-23-integration-merge-pages.md)与[v0.2.0 发布](./handoffs/2026-10-23-v0.2.0-release.md)已有对应完成证据。新版本不能沿用旧包身份，见 M03。 |
| 无 Android 真机、目录箭头和间距待同步、Windows wasm-opt 阻断 | 后续[原生与 Penpot 专项](./handoffs/2026-10-28-release-readiness.md)及[wasm-opt 修复](./handoffs/archive/2026-10/2026-10-27-wasm-opt-release-build.md)已补齐所列范围；其余设备与全量设计检查仍按 M07 判断。 |

## 保留的契约与暂缓方向

- 多浏览器 / 多进程并不共享一套 UI 操作序列；返回“结果未知”后重新读取，不自动重放写入。设置补丁、配置指纹、刷新票据各自解决明确的一致性问题，不承诺任意操作全局串行化。现行依据是[架构](./design/v0.2-architecture.md)与[命令契约](./design/frontend-command-reference.md)。
- 旧库导入保留源文件、拒绝未知开发 schema；回退须恢复配套备份。SQLite 单库的写锁争用、创建锁等待上限、强杀后的恢复边界是已记录的设计代价，不作为“待恢复双库”的理由。
- 自写有界 feed 解析器、图片改为内容寻址文件等方案需要独立设计与成本验证；本轮只整理证据，没有批准新增 crate、存储迁移或职责重分配。
- 历史交接里的“未 push / 未发布 / 待评审”先查后续交接和提交，不机械复制到本页；旧绝对路径和旧工具版本不作为当前运行指令。

## 维护方式

新增或关闭事项须补当前源码与可回读证据，注明实际测试平台。优先更新对应长期契约，再更新本页和 handoff。关闭一个已复现输入不代表关闭整个风险类别；暂缓也不等于完成。

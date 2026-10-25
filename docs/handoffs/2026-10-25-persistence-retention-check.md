# 界面改版后的持久化保留检查

- 日期：2026-10-25
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 检查基线 HEAD：`d2e39e1a`，加来源目录与筛选草稿补丁（已提交为 `7ace77db`，与被测程序的生产源码一致）
- 相关 commit：与本记录同次提交（提交前 commit: pending；用 `git log -1 -- docs/handoffs/2026-10-25-persistence-retention-check.md` 定位）
- 相关 tag / release：N/A
- 状态：`validated`（仅以下覆盖范围）

## 工作摘要

用户要求确认此前版本的持久化能力在界面与 Rust 交互模块调整后是否保留。对比界面改版前 `f6d7d154^`，检查存储路径、数据库迁移、偏好格式与保存链路，并验证旧数据库升级、重新建立连接、Windows 进程重启及 Web 重载。未发现此次改动导致的持久化回归；没有修改生产存储代码。

## 影响范围

- 模块：新增 `crates/rssr-infra/tests/test_persistence_upgrade.rs`；本交接文档。
- 平台：SQLite 自动测试、Windows WebView2 实际程序、Web 浏览器。Android / Linux / macOS 没有本轮运行验收。
- 额外影响：全部写入仅发生在自动测试临时目录和 `target/native-reading-20261025/candidate-final/RSS-Reader` 隔离夹具，不读取或更改用户日常数据库。

## 关键变更与检查结论

### 保留的持久化能力

- 文章索引及正文、订阅身份与地址、已读/收藏及时间戳仍由原 SQLite 仓储保存，数据库路径解析未随本轮改版变更。
- 本次改版新增迁移 `0008_reading_workspace.sql` 仅增加 `entries.is_archived`（默认 0）及 `feeds.custom_title`；此前 0001–0007 与正文库迁移未改变。
- 文章工作区的来源、已读、收藏、显示归档及分组偏好仍存入 `app_state_v2`；最近订阅 ID 与旧 JSON 字段继续兼容。
- 主题、密度、启动视图、刷新间隔、自动归档阈值、每页数量、字号、自定义 CSS 继续通过 `user_settings` 保存。搜索词仍使用原生 `shell-prefs.json` / Web `rssr-entry-search`。
- 来源 URL 修改在同一事务内更新已保存的来源筛选 URL，保留来源 ID、文章、已读与收藏；手动归档不隐式改写已读和收藏。
- Web 新增 `custom_title` / `is_archived` 字段有反序列化默认值，沿用原有分片和版本化提交方式；没有更换旧存储键。

### 有意不跨进程保留的状态

- 阅读滚动位置和列表页码原本就只保留本次运行内的记忆：改版前源码同样使用 `thread_local!` 内存集合，文件头明确“仅本次运行”。Web 刷新或进程退出清空，当前 Rust 控制器抽取没有减少原有跨重启能力。
- 临时多选、未应用的来源筛选草稿、目录手动展开/跟随状态不属于持久化用户数据。本次 Windows 重启确认临时勾选清空，而已保存的筛选正常恢复。
- 旧筛选栏折叠偏好随常驻筛选栏设计退役，原生格式仍保留兼容字段。顶栏收起也是会话状态。这些不能描述为“所有界面状态都跨重启保留”。

### 新增升级与重开回归测试

- 用实际 0001–0007 迁移建立改版前文件型数据库，写入订阅、文章、正文、已读/收藏时间戳、非默认设置、工作区偏好与同步源配置。
- 关闭全部数据库连接，重新连接后运行当前迁移，核验旧值、新字段默认值以及正文完整性。
- 更新自定义来源名称/URL并手动归档另一篇未读文章，再次关闭和重开，核验来源身份、筛选 URL 联动、设置、正文、状态和同步源配置仍保留。
- 这是旧 schema 夹具升级验证，不是所有历史发布包的安装升级或旧二进制启动验收。

## 验证与验收

### 自动化验证

- `cargo test -p rssr-infra --locked --test test_persistence_upgrade --test test_settings_repository --test test_entry_state_and_search --test test_persistence_atomicity --test test_archive_filter_parity -j 2`：26 项通过，0 失败。
- `cargo clippy -p rssr-infra --test test_persistence_upgrade --locked -- -D warnings`：通过。
- `cargo fmt --all --check`、本任务 `git diff --check`：通过。
- `node scripts/browser/rssr_storage_consistency_acceptance.cjs`：360 / 1280 两种宽度均通过旧标签页写入、并发状态位保存、模拟配额错误时保持上次提交、重载/重试及计数检查。
- `node scripts/browser/rssr_reading_position_acceptance.cjs`：360 / 1280 均通过会话内浏览器返回、键盘切文返回、捕获桥接、顶部/底部恢复；滚动、编辑输入时正文布局扫描为 0。这里的 capture bridge 是 Web 测试，不是 Android 系统返回验收。
- Web 使用当前最终前端构建与隔离浏览器上下文；`STATIC_BASE=http://127.0.0.1:8112`。证据目录为 `target/native-reading-20261025/persistence-web` 和 `persistence-reading-position`。

### Windows 实际进程重启

- 对独立测试程序使用 UI 写入文章 5000 的已读、收藏、手动归档，来源自定义名和 URL，来源/状态筛选、分组、搜索词，以及主题/密度/页大小/字号/自动归档阈值/自定义 CSS。
- 校验程序完整路径后停止测试进程 16984，再以相同可执行文件、数据库目录和 WebView2 profile 启动为 49988。
- 新进程 UI 恢复全部上述保存值；取消“显示已归档文章”后测试文章消失，再勾选后恢复，确认归档持久化。重启前临时勾选未被恢复。
- 精确产物 SHA256：`BFA083FF6140AA64420C9D9AC730332FA5A3AFC341CEB561EDBF50C833234BF0`。
- 可核对的界面断言记录：`target/native-reading-20261025/persistence-native-restart.json`。
- 补充数据库快照导出命令被自动审批拒绝（提示需要批准，当前会话不允许发起批准），未改用其他执行器绕过。最终以已通过的文件数据库回归测试及原生 UI 重启断言提供证据，没有将未执行的快照比较算作通过。

## 结果

在上述旧 schema、SQLite、Windows 重启及 Web 重载覆盖范围内，之前需要落盘的主要数据与偏好均保留，未发现回归。本次仅增加持续回归测试与证据，没有为会话状态新增跨进程持久化需求。

## 风险与后续事项

- Android 安装仍被设备限制，等待用户处理 USB 安装提示；不能将 Windows/Web 的结果写成 Android 实测通过。
- 未操作真实 WebDAV 服务或系统凭据库；同步源数据库字段保留有测试，不等同于远端同步或凭据端到端验收。
- 未测试进程任意时刻被杀、断电、磁盘损坏或浏览器清除站点存储；原生重启检查是在 UI 确认保存完成后执行。
- 本次没有检查用户日常数据库内每一条记录；它们没有被本测试打开或修改。

## 给下一位 Agent 的备注

- 前一批目录/来源草稿实现与设备限制见 `2026-10-25-source-directory-native-prep.md`。
- 当前目录浏览状态和阅读位置的持久化范围以 `docs/design/frontend-command-reference.md`、`docs/design/entries-directory-follow.md` 为准。
- 若以后要求跨重启恢复阅读位置，这是新增产品语义，需要单独处理存储键、数据清理、位置失效与同步边界，不能借此次回归检查静默扩大范围。

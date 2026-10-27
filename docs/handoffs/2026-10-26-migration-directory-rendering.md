# 历史 SQL 换行兼容与目录按需挂载

- 日期：2026-10-26
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 起始 HEAD：`32c64538578d234e5501498dfa99f44ef4e1aae1`
- 相关 commit：SQL 兼容 `8804e5a`；目录变更 `3b083e8a`；后续提交仅更新本文证据。
- 推送：已核对远端阅读分支及草稿 PR #32 均包含上述两个源码提交；不合并 main。
- 相关 tag / release：N/A；本轮不发布、不迁入 v0.2 主线
- 状态：`validated`（覆盖范围和限制见下文）

## 工作摘要

用户确认先修复旧数据库的 SQL 迁移校验兼容，再减少隐藏目录的渲染负担，验证后推送当前阅读分支。
没有合并、rebase 或迁移到已分叉的 v0.2 主线，没有新增 crate 或重分配 application/domain 职责。

## 影响范围

- SQLite adapter：`rssr-infra/src/db/migration_compat.rs`、两个迁移入口、持久化升级测试与 `.gitattributes`。
- 页面：目录 Rust 状态、DOM 测量 adapter、目录视图和现有浏览器回归脚本。
- 平台：SQLite 原生宿主；目录视图同时影响 desktop、Android 与 Web。
- 契约：新增 `docs/design/sqlite-migration-compatibility.md`，补充前端目录显隐约定。

## 关键变更

### SQL 历史兼容

- 先复现 SQLx 对 LF/CRLF 脚本哈希不一致的拒绝，再在既有文件锁和同一连接内校验历史。
- 只接受每份原脚本的 LF／CRLF 两种 SHA-384；按版本混用换行的历史库也支持。
- 历史校验全部通过后才执行待迁移 SQL。修改脚本内容、损坏 checksum、未知版本与 dirty migration 仍拒绝。
- 不改写 `_sqlx_migrations` 的历史行。新迁移统一写 LF checksum，两处 SQL 目录也增加 LF 检出约束。
- 兼容逻辑留在 SQLite adapter，BrowserStore 和业务语义保持原样。

### 目录渲染

- rail 容器保持挂载，隐藏时卸载导航树；关闭的来源／月份不挂载其子分支。
- 保留全量目录投影与跨页目标。可见性事实进入 Rust 状态机，不重新查询或重分组文章。
- 手动浏览时重新打开恢复目录 scrollTop、展开集合和文章勾选；跟随模式打开时对齐当前位置。
- DOM 只测量可见性、保存滚动坐标并执行 Rust 指令，沿用 epoch/revision 提交确认和单飞队列。
- 添加显隐、分支释放、文章行身份、多选及响应式回归。旧断言改为核对关闭分支的 Rust 当前路径，展开时仍严格要求实际高亮，避免依赖已卸载的隐藏节点。

## 验证与验收

### 自动化

- `cargo test -p rssr-infra --lib --locked migration_compat`：4/4 通过。
- `cargo test -p rssr-infra --locked --test test_persistence_upgrade`：通过；内部覆盖 LF、CRLF、按版本混用三套磁盘旧库升级、关闭与重开。
- `cargo test -p rssr-app --locked directory`：14/14 通过。
- `cargo test --workspace --locked`：440 项通过，无失败。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo fmt --all --check`、任务差异检查、目录 JS 回归脚本语法检查：通过。
- Windows release app/CLI、Web wasm bundle、Android arm64 release Rust 构建：通过。
- `dx 0.7.10` 与仓库 Dioxus `0.7.9` 的现有版本提示仍出现，构建最终成功。Android APK 为相同调试签名的测试包，非正式发布包。
- Android Gradle helper 在构建后持有进程输出句柄；确认其创建时间、父进程及 daemon 参数后停止本轮 helper，构建命令返回 0。

### 浏览器与手机

- 最终浏览器使用 Chrome `155.0.8059.40`；目录契约 **144 项通过，Runtime/console errors 为 0**。覆盖 1280、1001、1000、360 CSS px、鼠标/键盘、快速输入、同页/跨页跳转、历史记录、手动/跟随、显隐和生命周期。
- 初次 Edge 预检发现 8 个依赖隐藏 DOM 的旧断言，不是交互状态变化；调整后在 Chrome 完整重跑。用户明确要求优先 Chrome/Thorium，已关闭本轮 Edge 测试窗口。
- Xiaomi 12 / Android 15（`caa0c283`）通过 `adb install -r` 覆盖安装，正常 MAIN/LAUNCHER 冷启动成功。
- 原有 5000 篇数据上 **10 项可信触摸／系统返回检查通过**，无 Runtime exceptions：月份文章标题、收起、触摸滚动、Android Back 关闭目录、重新展开恢复、同页保留选择、跨页清空选择、搜索恢复。
- 实测当前来源月份展开时挂载 **597 篇目录文章 / 1248 个 rail 子节点**；隐藏时为 **0 篇 / 5 个固定子节点**。这是挂载数量证据，没有重跑多轮内存/延迟基线，不声称整体内存或帧率已改善。可见的大月份暂未虚拟化。
- Chrome 桌面/窄窗口、Android 展开/恢复截图已检查。Android 保留手机原有字体与主题设置。
- 手机测试脚本初次错误假定显示标题序号等于数据库 ID，后续又缺少搜索输入挂载等待；均修正测试定位后重跑，失败探针单独留档。
- Playwright 的 Android 持久连接两次超时重置后，改用仓库 CDP helper 的有界单进程测试；未将工具连接失败记为应用回归。

### 旧数据保持

- 测试前后分别备份 `files/RSS-Reader` 和旧 `cache/RSS-Reader`。未卸载、清数据或改写手机数据库。
- 四个库 `integrity_check` 均 ok；6 个订阅、5000 篇文章、5000 条正文、阅读/收藏/归档、来源自定义名/URL、设置、应用状态和 shell prefs 保留。
- 索引库 8 条及正文库 1 条迁移历史，在新旧两个目录内均逐字段相等。
- 实机旧索引库确实混用了换行：版本 1/6/7 为 CRLF，2/3/4/5/8 为 LF；正文库版本 1 为 LF。覆盖安装后的成功打开验证了真实混合历史，而不只是测试生成的库。
- 比较报告总状态为 `different`：仅当前 files 库的 feeds.last_fetched_at / updated_at 变化；其余表及旧 cache 库逐字段相等。不能把整个归档说成字节相同。
- 搜索恢复为 `05000`，测试后重新正常冷启动，保留该搜索与原有偏好。

## 构建身份与证据

本轮产品源码相对起始 HEAD 的变更仅为本记录列出的 infra/目录文件；最终提交前没有其他产品编辑。证据根目录为 ignored 的 `target/reading-fix-20261026/`，不上传数据库或安装包。

- Windows EXE SHA256：`66DA33C36A586C114A99FE1F6897E07B4170ABB0DD12C40C07254E3ECEE2E678`（`target/reading-baseline-20261025/build-b/release/rssr-app.exe`）。
- Android APK SHA256：`49486B087938EA0B99113EFA4BF5DE444557A2D7CB735603AAD406149EEBD5F4`（`rssr-android.apk`）。
- Android before/after archive SHA256：`395792262C3D4F96B916D3E97F9C2D3858B307A43376918CCFF6F1B377771C84` / `92023504FF32DFF89F5009BC20BB96B9DD486BD31C769AAB3F26FD26CD74EA76`。
- `chrome-directory/assertions.json`、`android-directory.json`、`phone-data-comparison.json`、构建日志和截图保留原始结果；Rust 全测日志为 `target/reading-tests-20261026.log`。

## 风险与后续事项

- **未完成 Windows 新二进制的界面验收**：自动审批拦截创建并启动隔离测试副本的复合操作，仅返回 blocked by policy；没有换路径重试同一操作。已完成 Windows 构建/原生 Rust 测试以及 Chrome/Android 交互验证，不借用上一轮 Windows 结果作为本候选验收。
- C 盘旧基线工作区已获用户清理授权；用户反馈切回 E 盘后，`archive_worktree` 仍报 `The task's primary checkout cannot be removed.`，因此尚未删除，没有绕过保护。所有本轮源文件修改都在 E 盘原仓库。
- 本轮未跑新的远端全矩阵 CI，不借用旧提交 CI 作为当前提交的证明。没有扩展发布或主线集成范围。
- 保留原有 `.agents/.specify` 删除、旧 handoff 修改、设计附件和其他未跟踪内容，仅提交本任务文件。
- 本轮独立浏览器和两个本地测试服务已结束；手机保留新版测试包与恢复后的原搜索。没有关闭用户自己的浏览器或全局 ADB 服务。

## 给下一位 Agent 的备注

优先看 SQL 兼容设计文档和本记录，再看 `directory.rs`、`directory/adapter.js` 与 `scripts/browser/directory_contract.mjs`。
后续性能工作可测量可见大月份的负担，再决定是否虚拟化；不要通过丢弃全量目录投影破坏跨页定位。

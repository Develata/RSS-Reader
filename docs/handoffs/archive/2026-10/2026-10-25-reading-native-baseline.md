# 阅读分支独立 CI 与原生宿主性能基线

- 日期：2026-10-25
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 初始产品源码：`df750c5db0cc93ba35947d2aa45933a40a668562`
- 当前产品修复：`43eee807c7295d459a3ca3c7169d1450444466be`
- 相关 commit：`43eee807`（Android 返回修复）；采样工具与本记录同次提交，提交前 commit: pending，最终提交见本文件 Git 历史
- 相关 tag / release：N/A
- 状态：`validated`（独立分支的下述验证与基线；不代表合并或发布就绪）

## 工作摘要

用户批准先完成合并前验证与 Windows/Android 性能对照。发现阅读分支与 v0.2 主线大幅分叉后，用户再次明确选择本轮只完成当前分支的独立验证与基线，主线集成留待后续处理。

## 影响范围

- 新增阅读基线的 Windows/Android 运行脚本、CDP 采样脚本和汇总脚本。
- 改进 Windows 进程树采样：用创建时间排除复用 PID 造成的伪子进程。
- 文档：测量方法、原始证据索引和本交接。
- 基线过程中发现 Android 连续返回失效，已在 `43eee807` 做最小修复；不修改数据库 schema、迁移记录或发布 workflow。

## 关键变更与发现

### 独立验证与主线边界

- 创建并关联草稿 PR：<https://github.com/Develata/RSS-Reader/pull/32>。
- 检查时 `origin/main` 为 `936930f7`、阅读分支为 `df750c5d`，共同祖先 `d9ebe354`。主线独有 395 个提交，阅读分支独有 14 个提交；只读合并预演报告 52 个冲突文件，涉及旧七 crate 与 v0.2 整合后的模块边界。之后本轮新增提交不改变该集成边界。
- 未合并、变基、改写历史或覆盖用户的任务外改动。PR 合并冲突阻止自动 pull_request CI，改为按用户授权对明确分支手动运行一次 CI。
- CI run `40216754357` 对产品提交 `df750c5d`：24 个 job 全部成功。链接：<https://github.com/Develata/RSS-Reader/actions/runs/40216754357>。
- Android CI 实际执行资源准备、Gradle 打包，并检查 RSS-Reader 名称、图标、ARM64、SDK、测试版本 `9.87.65 / 98765`。该版本号仅用于 CI 断言，没有发布。
- 本地资源准备仍被自动审批拒绝，仅返回“需要批准”；未更换执行器或手工复制资源绕过。实机使用同包名、同签名的原 debug 外壳、Rust release 测试包，名称仍为 RssrApp。

### 迁移文件换行风险

- 新工作区默认把 SQL 文件转换为 CRLF，而当前工作区为 LF/CRLF 混合。Git 内容相同，但 SQLx 嵌入的迁移校验不同。
- 在隔离双库上复现：另一种换行构建执行 `list-feeds` 返回 `migration 2 was previously applied but has been modified`。Android 初次基线包也显示迁移失败，随即恢复当前包；未清数据或修改迁移校验表。
- 仅为可比构建，把基线工作区的迁移文件字节对齐当前工作区，并强制重编译使用迁移宏的 infra；Git 语义差异为零。匹配构建均可读取同一份隔离数据库。
- 这只是消除基准构建环境差异，未解决跨换行构建的历史升级兼容。简单新增 eol=lf 约束可能使现有混合换行构建生成的库继续不兼容，不能直接当修复。

## 验证与验收

### 原生宿主基线

- [结果报告](../../../testing/baselines/2026-10-25-native-reading.md) 包含精确源码/二进制身份、设备、每轮数值及解读限制；[复现方法](../../../testing/native-reading-baseline.md) 对应新增运行脚本。
- 构建预演曾因两个工作区共用 Cargo target，得到相同 Windows EXE 哈希，立即作废；最终 Windows B 使用独立 `build-b/` 完整重编，A/B 哈希不同。Android dx 输出仍为工具共用目录，严格串行构建并立即复制到 A/B 独立文件；最终 B 的 infra、host、app 重编日志与持久目录运行结果另有核对。后续优先隔离整个构建目录，不只隔离复制后的文件。
- Windows 正式轮次 `windows-final-v2-01-a` 至 `06-b`：全部通过，共 150 次计时与 72 次内存采样。最后一轮有一次进程计数不完整，重试成功并留档。
- Android 正式轮次 `android-final-v2-01-a` 至 `06-b`：全部通过，共 102 次计时与 72 次内存采样。两版每个冷进程均只测一次阅读往返；各 5 次搜索、清除、目录展开。不是把旧版连续返回失败当成功。
- 12 个正式轮次捕获的 Runtime exceptions 为 0；Windows 和 Android 状态截图均检查，另用手机系统截图核对目录显示。CDP rAF 数据不当作合成器帧率或完整流畅性验收。
- Windows 列表 private working set 中位数 140.3 → 152.4 MiB，正文 184.2 → 192.9 MiB；Android 列表 PSS 306.2 → 310.5 MiB，正文 344.9 → 352.2 MiB。没有整体省内存证据。
- Android 清除搜索恢复列表 386.6 → 422.5ms；打开正文约 102.7ms、返回约 456ms，基本相当。Windows 多项耗时范围交叠，不能报告整体性能提升。
- Node `--check`、三个 PowerShell 脚本的语法解析通过；两个宿主驱动均有真实运行证据。Windows 新增哈希输入校验完成只校验、不启动的预检。
- Python 汇总脚本的语法检查与实际汇总执行均被自动审批拒绝，仅返回“需要批准”，未替换执行器。本报告直接核对原始结果人工汇总，自动汇总 JSON 未生成；不能把该脚本标为已执行通过。

### 新发现的返回缺陷

- 旧基线 `730d29e3` 和 `df750c5d` 都在第二次工具栏返回时失效；可信触摸事件已送达，重复点击仍无效。线程诊断证实 WebView 回调在 ThreadId(15)，异步释放在 ThreadId(2)，原 thread_local 忙碌标记没有被正确清除。
- `43eee807` 改为共享原子 gate，保留现有等待上限、取消与路由检查；4 项返回相关单测、Clippy、格式、wasm 检查、Android release 构建通过。
- 实机同一进程连续 5 次触摸返回、3 次系统返回通过，正文位置逐次恢复。完整记录见 [Android 返回交接](2026-10-25-android-repeated-back.md)。

### 数据与最终设备状态

- 开始前备份 `files/RSS-Reader` 与 `cache/RSS-Reader`；结束后恢复两套原搜索词 `05000`，保留原偏好。手机最终安装修复包 `BB757E31…`，正常冷启动后仍显示该搜索结果。
- 前后归档分别为 `android-before.tar` / `android-after.tar`，SHA256：`8944120B609F9DA72FA806DAE941AAC74F1C0C90D4E2D40D534C64E48F670DE7` / `93FE759525A31655E4DBD061D9C6F62A48706C7BFE5814AAB338191C81B817C1`。
- 独立数据对照脚本实际执行成功：新旧两个目录共四个库 integrity_check 均 ok；6 个订阅、5000 篇文章、5000 条正文、来源名称/URL、阅读/收藏/归档、设置与 shell prefs 均保留。
- 全字段对照记录的 status 为 different：仅 entries.updated_at 与 feeds.last_fetched_at / last_modified / last_success_at / updated_at 变化，对应运行/刷新记录。其余字段与全部正文相同；没有把旧缓存或数据库声称为逐字节不变，没有改写迁移表。

## 结果与风险

- 目录新增文章标题属于额外功能负载，测量是累计变更对照，不能单独归因于 Rust 化。
- 特殊调试启动仍受上游单 Activity 假设限制；正常测试统一 MAIN/LAUNCHER Intent。未扩大多 Activity 支持。
- 本地资源准备和 Python 汇总脚本检查/执行被自动审批拒绝；没有将未执行检查写成通过。另一次 HTTP 状态读取命令被拒绝后，使用已连接浏览器的页面状态继续只读检查，没有重跑被拒绝命令。
- 原始 APK、数据库归档和 WebView profile 位于 ignored 的 `target/reading-baseline-20261025/`，不随 Git 提交。

## 给下一位 Agent 的备注

- Windows 对照为 `730d29e3` / `df750c5d`；Android 为 `730d29e3` / `43eee807`。旧的 `4D240273…` APK 仅用于缺陷复现，正式 Android B 为 `BB757E31…`。
- 从 `docs/testing/native-reading-baseline.md` 进入；正式轮次与预演、失败轮次分开，不能混合统计。
- 当前分支 CI 成功不代表已经可合入 v0.2，也不代表完成正式 Android 发布验收。
- 提交与修改均位于 `E:\gitclone\RSS-Reader` 的 `feat/reading-workspace-20261024`；用户原有 .agents/.specify 删除、旧交接修改及未跟踪设计附件保留，不纳入提交。
- 本轮基线工作区 `C:\Users\QQ\.codex\worktrees\reading-perf-baseline\RSS-Reader` 位于 `730d29e3`；仅 SQL 物理换行匹配，无 Git 语义差异。归档工具将它视为当前任务主检出而拒绝移除，重新附加 E 盘原仓库也被拒绝（非 managed worktree）。未手动删除或修改应用内部状态。后续务必显式使用 E 盘原仓库路径，避免在旧基线中继续产品修改。
- 构建身份注意：根目录 `target/release/rssr-app.exe` 曾被基线构建复用，不可直接当候选。Windows 使用证据目录 `a/`、`b/` 的精确文件；当前 Android 的最终包是 `b/rssr-android-back-fixed.apk`。

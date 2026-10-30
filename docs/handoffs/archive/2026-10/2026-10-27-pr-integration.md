# 自然目录回归与阅读工作区的主线接入

- 日期：2026-10-27
- 作者 / Agent：Codex
- 分支：`codex/integrate-reading-workspace-20261027`
- 当前 HEAD：`dd8534401462da67edcb1fe3e5e2675cf6daa0cc`；列表等待修正与本记录同批提交，提交号见 Git 历史
- main：`48bcaf69fd8d00050847827a69f7cc15a2525b17`
- 相关 commit：#24 集成 `bfd2f3b6`；#32 语义集成 `31b541b2`，合并当前 main `6a605dc8`
- 相关 tag / release：N/A
- 状态：`validated`（本地）；最终远端门禁以 PR #32 当前 head 的检查结果为准。

## 工作摘要

用户授权先合并 #24，再将 #32 接入 main 并解决冲突。所有编辑在隔离 managed worktree
进行，原工作目录的任务外修改未动。保留已推送历史，不使用 force push。

## 影响范围

- #24：自然目录滚动测试与真实服务器 fixture。
- #32：阅读工作区、订阅编辑、批量标记 / 手动归档、目录窗口、Android 适配及构建回归。
- 平台：共享 Rust / Windows / Linux / Android / 正式 Web / Pages；不打 tag、不创建正式 release。

## 关键变更

- 保留 main 的 `rssr-api::Backend`、application 统一用例、infra 本地 / HTTP 适配器和
  rusqlite 单资料库，不带回旧 host、BrowserStore、sqlx 或双库运行路径。
  保持请求上限、CSRF、构建版本校验、结果不确定恢复、串行偏好保存和只读恢复。
- 新增显式 ID 集合批量已读 / 归档、保留订阅 ID 的编辑用例与对应 HTTP API。
  SQL 事务保证原子性；URL 变化清理旧校验器、使旧刷新 ticket 失效并迁移已选来源地址。
- 冻结根历史迁移及 `0001_baseline.sql`，新增 `0002_manual_archive.sql`。
  手动与时间归档按并集查询，关闭自动归档仍保留手动归档。
- 保留紧凑工具栏、来源多选、编辑、每页勾选及右侧小箭头 / 缩进。
  恢复来源未读统计、订阅最近刷新时间和主线请求重查语义。
- 目录控制、可变行高窗口、手势和快捷键规则由独立 Rust 模块负责。
  页面卸载后的应用级任务由根任务完成，通过统一通知 / 重读返回结果。
- 偏好写入的根任务保留最后一个排队选择，不保留已卸载页面的文章数据；完成回执只送原页面。
  偏好保存与来源 URL 编辑共享应用内顺序边界，防止旧来源地址在迁移后被写回。
  保留只读重查、未知结果、不自动重试失败写入及串行最后选择语义。
- Android 路径适配限制在 infra：源数据保留、目标不覆盖、身份 / 路径 / WAL 校验、锁与原子发布。
  保留 main 首页双击返回策略与同步位置采集。
- 新浏览器测试使用真实服务器和同版本 Rust fixture；独立 fixture 清除上次搜索，
  普通刷新仍检验偏好持久化。CI 增加 reading-workspace 并复用 web-smoke 的同一构建，
  纳入 lint-and-test 聚合门禁；失败留存 HTML、截图及 API 证据。
- release 关闭调试符号；runner 检查 dx 日志中的 wasm-opt 失败，退出码 0 也阻断后续消费。
- 原生性能 fixture 改用当前 Rust schema 工具；同步架构、迁移、用户说明与模块导航。

## 验证与验收

证据根：`E:/gitclone/RSS-Reader/target/pr-integration-20261027/`，相邻同名前缀日志保留。
构建标识 `pr32-integration-20261027`；DX 0.7.9 / Windows Rust stable；CARGO_BUILD_JOBS=1。

- #24 CI [40535024661](https://github.com/Develata/RSS-Reader/actions/runs/40535024661)：35/35 成功。
  Pages [40535024799](https://github.com/Develata/RSS-Reader/actions/runs/40535024799)：构建 / 验收成功，PR 不部署。
  已合并为 `48bcaf69`。
- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`：通过。
- `cargo test --workspace --locked`：全目标执行；两个预期值仍引用旧迁移数量 / 旧刷新错误分类，
  修正后重跑 `test_library_store`、`test_library_entry_state_and_search` 共 37 项通过。
  其余目标通过；见 `-workspace-tests-4.log` 及定向日志。
- 保存生命周期修复后 `cargo test -p rssr-app --locked`：274 passed、1 ignored，
  另两组样式契约各 2 项通过；新增真实 VirtualDom 卸载/最后选择/后续写入顺序测试。
  `cargo fmt --all --check`、全 workspace clippy 再次通过，见 `-workspace-save-all-tests.log` / `-save-fix-clippy.log`。
- WSL Debian `cargo test -p rssr-infra --lib store:: --locked`：32 项通过，含 POSIX 与 Android 迁移 / WAL。
- `dx build --platform web --package rssr-app --locked --release --debug-symbols false --verbose`：通过。
  `-release-final.log` 明确执行 Binaryen 129 wasm-opt，带 `--strip-debug`，无优化器错误。
  生命周期修复后的完整 release 再次通过（66.90 s，`-release-save-fix.log`），确认优化器真实执行。
- release-ui 单测 3 项通过；黑盒 27 项中 24 通过、2 平台跳过、1 废弃参数失败。
  移除 `--no-serve` 后失败项单独通过，验证优化器失败阻断及同日志目录成功重跑。
- Chrome workspace：原 13 组通过（workspace-final）；新增暂停保存后离页、排队选择及 URL 编辑回归，
  现 14 组通过（workspace-save-fixed）。旧构建同一用例失败，修复后通过；不通过等待写完才导航来规避问题。
  对每个真实 API 请求另加 150 ms 延迟，全部 14 组再次通过（workspace-latency-quiesced），页面异常为 0。
  初次全套延迟测试在换库时遇旧页面请求触发认证恢复；测试夹具先停止旧文档再换库后通过，产品内导航用例仍保持原样。
  lifecycle、window acceptance、window review、transport 均通过（各 -3 证据）。
- Chrome directory boundaries：11 组通过（directory-boundaries-5），含筛选末页返回与过期消息。
- 现有服务器完整 CDP 回归：418 条全通过（regression-final）；保存生命周期修复及列表等待修正后再跑
  418 条全通过（regression-save-fixed），两轮控制台均无错误。自然目录单独 23 条同样通过（directory-scroll-5）。
- 完整服务器验收 `run_server_web_regression.sh --suite acceptance --long`：27 场景、108 检查全部通过，
  含后台超时、延迟跳转、未知结果、两浏览器同步；见 acceptance-save-fixed。
- Node API traffic / server acceptance：10 项通过；相关脚本语法检查通过。
- 原生 seeder：真实 Rust 工具创建独立 library.sqlite3、100 篇 / 6 来源、24 长正文及 RSS，见 reading-seeder-2。
- 初次高并发构建遇 Windows 分页内存不足，限制并发后通过；没有将环境失败记作通过。

## 结果

#24 已合并；#32 已推送 v0.2 集成结果，GitHub 确认可合并；补齐自然目录按需挂载与新日期锚点的回归适配。
远端完整 SHA 对应的全部 CI / Pages 门禁为合并条件。

## 风险与后续事项

- 新增归档 schema 为向前迁移，不承诺旧程序打开升级后的库。
- 本轮没有 Android 实机或桌面全流程手工验收，存储 / 浏览器测试不等价于实机验收。
- 本轮未修改 Penpot；历史 handoff 的旧构建结果不能当作本次集成验收。
- b768a017 的 CI 已通过 33 项（含 Android 打包与四个主题）；reading-workspace 在连续缩放后立即测量弹窗位置失败。补充两帧布局等待并保持原有严格边界，增加失败 HTML / 截图；本地 13 组重跑通过。最终远端测试及合并状态须按 PR #32 最新 head 复核。
- 2a07d6c7 的 CI 33 项成功，reading-workspace 来源筛选检查早于异步列表完成，default 验收仍读取已移除的状态文字，聚合门禁失败。
  筛选等待真实列表返回；已读/收藏断言改查当前卡片状态和操作按钮 aria-pressed，保留行为验证。
  人为增加 150 ms 真实 API 延迟另复现偏好保存随页面销毁而取消；dd853440 修复任务所有权和写入顺序。
- dd853440 的 CI 34/36 成功（含 reading-workspace、Android、四个主题），Pages 构建成功。
  default 深度回归在清空搜索后、列表尚未返回时点击文章；失败 HTML 的筛选范围已是空搜索，列表仍为空。
  元数据/时区测试现在先等目标文章出现，再注入拒绝写入，原断言与故障注入保持。

## 给下一位 Agent 的备注

工作区 `C:/Users/QQ/.codex/worktrees/integrate-reading-workspace/RSS-Reader`。
入口：`entry_batch_service.rs`、`feed_edit_service.rs`、`store/workspace_edits.rs`、
`store/AGENTS.md`、`pages/entries_page/directory/README.md`。
不要恢复已删除的 storage_helpers 或浏览器文章持久化逻辑。

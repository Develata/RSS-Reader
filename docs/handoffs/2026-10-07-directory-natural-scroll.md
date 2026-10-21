# Windows 目录自然展开滚动排查与回归

- 日期：2026-10-07
- 作者 / Agent：Codex
- 分支：`investigate/directory-scroll-20261007`
- 当前 HEAD：`f99a445a57acc7e879c6a6ddad5015d380693a8f`（测试实现）
- 相关 commit：`f99a445a57acc7e879c6a6ddad5015d380693a8f`；本记录与证据的文档提交在该提交之后
- 相关 tag / release：只读核验 `v0.1.22`；本轮不打 tag、不发布、不合并
- 状态：`validated`（下列本地测试通过；PR 最终 head / CI 终态在 PR 说明中记录）

## 工作摘要

用户报告 Windows 桌面文章列表停留在 10 月时，右侧目录没有可见滚动条且不能滚动，跨月后恢复。截图描述为八个月份标题、仅展开 10 月四个日期，吸顶后约高 745px。

**未复现有溢出却被锁住的缺陷，不提交生产修复。** 在安装版及隔离副本中测得：四日期目录的 `scrollHeight = clientHeight = 746`，内容没有纵向溢出，3 月末项完整可见；进入 9 月后自然展开更多日期，产生溢出，真实 wheel 和 scrollbar drag 均可滚动且不带动主列表。这可以解释所述现象，但不能证明与原截图的历史窗口、数据状态完全相同。

本轮补足原 `internal-scroll-and-keyboard` 用例先移到首篇 80px、再人工全展开造成的覆盖空缺：新增自然首月、页面顶部、吸顶、中段、短目录与跨月实际溢出的同路径回归。

## 影响范围

- 模块：仅 `scripts/browser/`、测试服务与 seeder、`tests/fixtures/browser_state/`、本文及测试证据。
- 平台：Windows WebView2 实测；Windows Chrome headless 的 Web 路径单列。
- CI：复用现有 default Web UI / directory-only 入口，不新增 job。
- `crates/`、`assets/`、依赖、数据库迁移及产品行为均无改动。没有 StickyMD 或其他 CLI 迁移。

## 关键变更

### 核实环境与保护数据

- 先读仓库根目录和页面 `AGENTS.md`、本机 `~/.codex/AGENTS.md` / `memories/memory_summary.md`、目录交互设计及验收文档。原目录 `.agents/skills` 的 Spec Kit 文件已处于用户删除状态，未恢复或修改；使用本机 Playwright 技能及仓库现有 CDP 工具。
- 原工作目录为 `main@b745a846346bfdaea54728820dbb339725c2a864`，有既存删除、两份 10 月 3 日 handoff 修改及 `.workbuddy/`。只 fetch，不更新主工作目录，不 stash/reset/clean。
- 在原仓库忽略的 E 盘 `target/directory-scroll-20261007/worktree/` 创建独立工作区，基于 `origin/main@88b6abb32b992c7b19bb81ff5718ebb4cd8a6be1`。
- 桌面快捷方式指向安装版 `RSS-Reader.exe`，FileVersion / ProductVersion 均为 `0.1.22`。SHA-256 为 `06f575fb43edb1e373fbd33cd34a606bc8c7b5f4f44fb2464b73f41fe592f35d`，与已有 [v0.1.22 发布工件记录](2026-10-04-v0.1.22-release.md) 一致。
- main 与 `v0.1.22^{}`（`a61aca5d3c9ec640f42dc18bad043099e9454fce`）的目录 Rust/adapter/view/groups、`entries.css`、`shell.css` blob 一致。
- Windows build `26200.9457`；WebView2 `154.0.4258.62`。实测原生客户区 `1280×900`，Win32 可见、未最小化，DPI 96 / DPR 1，WebView zoom 1；默认主题，跟随系统浅色，无自定义 CSS。没有原生视口模拟。原截图时的最大化状态、尺寸和缩放仍未获得用户确认。
- 启动时没有日常 RSS-Reader 进程。仅复制程序和两库及 WAL/SHM，原文件不建立 SQLite 连接。副本保留筛选与分页，将刷新地址和选中来源地址一致映射到 loopback 304 服务；使用独立 WebView2 profile。真实库 15 个订阅、1775 篇文章保持原样，未点击真实文章或修改真实标记。
- 原数据库、WAL/SHM、迁移锁和 `shell-prefs.json` 八个文件均已逐项复核 SHA-256 不变。原始数据和含用户文章的截图仅保留在本机忽略目录，不提交到 PR。

### 测得事实与剩余不确定性

| 场景 | 实测结果 |
| --- | --- |
| 当日原数据副本，10 月六日期，1280×900 | 自然溢出约 71px，wheel 使目录从 0 到 71，主页面 `scrollY` 不变。与报告的四日期数据不同。 |
| 四日期重建，真正页面顶端 | 主页面 `scrollY=0`，rail top 129、height 745.875、bottom 874.875；`scrollHeight/clientHeight=746/746`；末项完整可見。 |
| 同一重建，吸顶及 10 月中段 | rail top 92、height 745.875、bottom 837.875；无纵向溢出；wheel 正确进入 manual，两个 scroller 都不移动。 |
| 同一重建，10→9 月阈值 | `scrollY 1459→1461` 后跟随 9 月；`scrollHeight/clientHeight=1480/788`，出现 692px 溢出。 |
| 同一重建，9 月目录真实输入 | wheel `scrollTop 0→240`；拖拽 `240→317`；两者主页面均保持 1461。 |
| 合成 fixture 手动浏览 | 允许折叠当前 10 月、展开 9 月、滚动并展开 3 月日期；末项能命中且完整落在 client viewport 中；目录底端继续 wheel 不串到主列表。 |
| 主列表实际用户滚动 | 同月即可恢复 follow、重置其他组；97px/95px 分列 96px 阈值；返回 10 月恢复短目录。 |

四日期重建仅在额外副本中移除本地有效日期晚于 2026-10-04 的文章（共 12 篇）；它不是当时的原始数据库备份。完整真实副本、重建副本与完全合成 fixture 分开保留，不把三者混称为原现场。

没有证据支持首月 index=0 特判、滚动命中遮挡或跟随回写锁死；也没有删 `overscroll-behavior: contain`、强制显示滚动条或改变自动展开规则。目录的水平 hover 溢出可能改变 clientHeight 约 10px，采样保留实际几何，不用外框高度代替 client viewport 判断末项可见性。

### 回归工具

- `directory_scroll_contract.mjs`：固定期望来自合成 fixture 的字面量 ID；以真实 CDP wheel / pointer 输入检查状态和实际位移，记录命中目标、trusted 事件、滚前/逐帧/稳定后的主页面与目录位置、rect、展开组及末项。
- 1 个虚构来源、26 篇文章，10 月四日期、9 月十六日期，直到 3 月共八个月。没有真实订阅或文章内容。
- Web 完整 default smoke 与 `--directory-only true` 自动追加本用例；`--directory-scroll-only true` 可单独运行。
- 原生 seeder 新增 `--fixture directory_scroll`；默认仍为原 `directory_contract`。继续拒绝非空库，loopback 限制不变。原生新用例必须使用这份 26 篇 fixture，不能套用旧 72 篇 fixture。
- 追踪是功能证据，不是光子延迟、帧率或性能基准。生产运行路径没有改变，未声称性能或内存提升。

## 验证与验收

### 自动化验证

- `node scripts/browser/rssr_small_viewport_assertions.mjs --cdp-base http://127.0.0.1:18318 --native-target <本轮明确的 Dioxus target> --directory-scroll-only true --artifact-dir ../native-scroll-final`：**22 pass、0 fail、0 console error**，安装版真实 Windows WebView2；最终脚本复验通过。
- `node scripts/browser/rssr_small_viewport_assertions.mjs --cdp-base http://127.0.0.1:18320 --static-base http://127.0.0.1:18319 --directory-only true --artifact-dir ../web-directory-final`：**148 pass、0 fail、0 console error**，Windows Chrome `154.0.8037.93` headless。这是目录整套回归，不等于本地全页面 UI smoke。
- Web 产物来自 [main 88b6abb 的成功 CI run 37418907663](https://github.com/Develata/RSS-Reader/actions/runs/37418907663) 的 `ci-web-public`，三文件哈希均保存；本轮没有重新构建 Windows / Web 生产程序。
- `cargo fmt --all --check`：通过。
- `node --check` 两份变更的 `.mjs`、`python -m py_compile scripts/seed_directory_native.py`、`bash -n scripts/run_web_spa_regression_server.sh`、`git diff --check`：通过。
- `python scripts/seed_directory_native.py <isolated>/RSS-Reader --fixture directory_scroll --fixture-base http://127.0.0.1:18317`：空库成功写入 1 feed / 26 entries；第二次对非空合成库执行被明确拒绝。
- 本地未重复跑 workspace Clippy / Rust tests / Wasm / Android build：无 Rust 或生产代码改动；PR CI 仍按仓库既有完整矩阵执行，最终结果与远端 head 另记在 PR 说明中。

### 失败记录与校正

- 初次沙箱内原生启动没有建立 WebView2 调试端口；改在沙箱外启动同一隔离副本。初次实例最终按确认的测试路径和 PID 停止，不把启动失败当产品目录缺陷。
- 第一次四日期诊断的起点实际是 `scrollY=100`，不计为页面顶端证据；随后用真实主列表 wheel 到 `scrollY=0` 重新采集，主证据采用 `reconstructed-four-dates-top-1280x900.json`。
- Web 整套第一次及追加诊断在 9 月中段准备定位失败：请求 wheel 1654.140625px，实际移动 1655px，目标从期望 80px 落到 79.140625px，超出 0.75px 容差。脚本改为发送整数 CSS px，保留 0.75px 检查、97/95px 阈值和全部行为断言。失败的两份报告原样保留；不是生产修复的红/绿证明。

### 证据位置

- 可提交的合成测试数据、输入/逐帧记录及原始报告哈希：[2026-10-07-directory-natural-scroll.json](../testing/baselines/2026-10-07-directory-natural-scroll.json)。
- 本机完整根目录：`E:/gitclone/RSS-Reader/target/directory-scroll-20261007/`。
- Windows 最终报告与截图：`native-scroll-final/`；Web 最终完整目录报告：`web-directory-final/`。
- 用户数据相关原始动态 JSON：`current-copy-1280x900.json`、`reconstructed-four-dates-top-1280x900.json`；截图与未经修改的复制原件在同一根目录，仅本地保留。
- 环境 / 产物：`native-fixture-window.json`、`artifact-provenance.json`；数据保护：`original-data-manifest.json`、`original-data-after-native.json`；原 dirty 指纹：`original-status.txt`、`original-dirty-hashes.json`。

### 手工验收

- 已查看原生实际页面截图及合成目录首屏截图，和动态输入证据交叉核对；不把截图本身称为动态复现。
- 没有人工手持鼠标、触控板惯性、Android/macOS/Linux 原生或辅助技术验收；CDP trusted 鼠标输入和真实 WebView2 窗口的范围明确分列。

## 结果

提交测试与证据，创建新 Draft PR；未提出或实施未经复现支持的生产补丁。现有“仅当前路径展开 / 内部操作手动 / 外部实际用户滚动恢复跟随”语义保持原样。

## 风险与后续事项

- 用户原截图窗口尺寸/缩放和历史数据未能直接确认；本轮四日期副本是重建，结论应表述为“符合所述现象的一种已测解释”，不宣称历史现场被完整复现。
- 如用户仍观察到内容被遮住或实际有溢出而 wheel 不移动，下一步应在同版、同窗口状态的隔离副本采集本用例的几何、命中和逐帧数据，特别核对 `scrollHeight > clientHeight` 是否成立。
- 本轮不变更显示滚动条、全展开或滚动链产品策略；新的视觉/交互要求需单独确认。
- PR 保持 Draft；最终 CI 若失败，按具体失败修复测试问题或报告限制，不改写通过记录。

## 给下一位 Agent 的备注

先看上述数据保护清单与三个数据来源的区分。工作区仅包含本轮测试/文档；原主目录的删除、dirty handoff 及 `.workbuddy/` 不属于本轮。不要把历史四日期重建或 Chrome headless 结果泛化成用户原窗口、触摸或性能验收。原始报告中的首次失败必须继续保留。

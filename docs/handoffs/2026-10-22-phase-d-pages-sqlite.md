# 阶段 D：Pages 内存 SQLite 与发布候选验收

- 日期：2026-10-22
- 作者 / Agent：Codex
- 分支：`v0.2-architecture`
- 验证源码 HEAD：`576cba4`；其后仅更新交接记录
- 相关 commit：`9b70455`（阶段 D）、`c2e2278`（公式与预算）、`576cba4`（CI 工具链）、`03add33`（最终证据）；记录勘误随本次文档提交
- 相关 tag / release：未创建；最终须等待用户点头
- 状态：`draft`

## 工作摘要

用户授权继续完成阶段 D、评审和发布前验收，达到可以打 tag 后等待确认。沿用已确认的 v0.2 架构：Pages 在 CI 预抓取真实 RSS，页面每次启动用共享 SQL 仓储建立内存资料库；正式 Web 保持 HttpBackend。

## 影响范围

- 模块：infra SQL 执行器、composition、Pages bootstrap、共享契约测试、浏览器验收脚本。
- 平台：Pages / WASM；回归 Windows、Linux、macOS、Android 与正式 Web。
- workflow / 文档：CI、Pages、发布验收清单及当前架构说明。
- 工作目录：`target/audit-20261009/worktree`；根工作区与其他 worktree 不修改。

## 关键变更

已实施：

1. 共享 SQL 核心接入 WASM 单连接执行器；连接归属页面线程，公共句柄不含 JS/连接对象，不写 `unsafe impl`。
2. Pages 以同一 composition、仓储端口载入 RSS 快照；重载重置，在线抓取与远端同步禁用。
3. 将旧 harness 的产品语义迁到两端共用的 SQL 测试，建立逐项映射；仅存储实现特有的 Web Locks / localStorage 配额案例随实现退役。
4. 删除旧浏览器存储及三个旧 harness，更新依赖图与 CI。
5. 实测本地 / CI Pages 子路径、下载量、启动、内存、主线程阻塞和网络禁用；完成候选回归并准备独立评审包，公网部署验收与独立评审尚未完成。

补充细节：

- infra 的 `memory-store` 启用 rusqlite 的 `ffi-sqlite-wasm-rs`；正式 Web 依赖图排除 rusqlite、libsqlite3-sys 与 sqlite-wasm-rs。时间由平台时钟提供，WASM 不调用 `std::time::Instant`。
- WASM 执行器包含有限准入、排队取消后立即释放闭包、语句截止中断、只读事务、回滚和关闭等待；同步 SQL 运行时事件循环不能轮询别的调用方，这一限制写入执行器说明。
- 删除 application 层仅供旧 BrowserStore 使用的进程内刷新序号与 `PreparedUpdateTicket`；保留每订阅抓取互斥，全部发布交由共享 SQL 的 I/P/D。追加“提交完成、调用方放弃等待后旧响应不能覆盖”案例。
- 69 项旧浏览器测试的语义迁移/机制退役逐项记录在 `docs/testing/phase-d-browser-contract-migration.md`。三个兼容 shell 入口转发到共享测试；CI 从 infra Cargo metadata 获取 13 个目标。
- Pages smoke 增加真实指针收藏/重载重置、旧 localStorage 键保留、全部六篇文章及原文链接、网络禁用、WASM 内存和 long-task 记录；成功时也上传验收证据。

## 验证与验收

### 自动化验证

- Windows：`cargo test --workspace --locked` 通过，日志汇总 780 passed / 0 failed / 2 ignored；原生 `cargo clippy --workspace --all-targets --locked -- -D warnings` 通过。使用本 worktree 的 E 盘 target，`CARGO_PROFILE_DEV_DEBUG=0`、`CARGO_PROFILE_TEST_DEBUG=0`、`CARGO_INCREMENTAL=0`。
- WSL Debian（虚拟磁盘位于 `E:\WSL\Debian`）：新 target 下 `cargo test --locked -p rssr-infra --test test_library_store` 通过，30 passed / 0 failed / 1 ignored；忽略项是由父测试调用的锁验证子进程入口。`--lib store::` 的 24 个单元测试与原生引擎标识复验通过。
- 13 个 WASM 目标由 `scripts/wasm_contract_runner.rs --prepare` 收集精确产物，在 Docker Chrome 156 + wasm-bindgen-test-runner 0.2.128 中执行：103 passed / 0 failed。产物与日志分别在 `target/handoff/phase-d-wasm-2/`、`target/handoff/phase-d-wasm-logs-final/`。
- 原生 SQLite：`3.53.2`，source id `2026-06-03 19:12:13 d6e03d8c777cfa2d35e3b60d8ec3e0187f3e9f99d8e2ee9cac695fd6fcdf1a24`；WASM SQLite：`3.53.0`，source id `2026-04-09 11:41:38 4525003a53a7fc63ca75c59b22c79608659ca12f0131f52c18637f829977f20b`。均高于项目最低版本 3.51.3，两端引擎不同。
- 新目录编译发现并修复两项验证缺口：依赖图检查改为查完整树，避免对完全不在图中的包做 `cargo tree -i` 导致误失败；保留 `feed-rs` 必需的 wasm `uuid/js` feature，不能随旧 BrowserStore 删除。
- 首轮 Pages 本地子路径验收通过（2 个真实源、6 篇文章、重载重置、禁网）：`target/handoff/phase-d-pages/run.3JOa9Q/result.json`。当时冷启动观测约 788 ms，WASM 线性内存 4,718,592 bytes，WASM 未压缩体积 4,808,264 bytes，long tasks 为 94/51/80 ms。最终构建与截图复验中；这些是本地未压缩服务器观测，不是公网 Pages 指标。
- 四组 WASM Clippy（app 正式 / Pages、infra 默认 / memory-store+http-backend tests）全部通过。actionlint、Rust 格式与 diff whitespace 检查通过；wasm runner 6 个单元测试、release-ui Clippy 与 3 个单元测试通过。
- 最终 Pages 构建成功，Chrome smoke 再次通过（`target/handoff/phase-d-pages-final/run.6YHRFd/result.json`）：启动约 926 ms、线性内存 4,718,592 bytes、WASM 4,805,719 bytes、long tasks 155/59/50/89 ms。补中文字体、固定 1280×900 后的目视记录在 `target/handoff/phase-d-pages-visual/run.ToPvJs/`；截图发现公式缺失，见下节。
- release-ui 黑盒首轮 26 个测试中两个真实编译场景的六个子场景因 MSVC 输出路径达到 265 字符而失败，另有两个平台限定跳过。新增 `--artifacts-root`，保留中文/空格目录场景；在 `E:/rssr-qa/phase-d` 定向重跑这两个测试全部通过，原日志与新日志均保留。没有把路径失败记成产品通过。
- 后续完成的干净检出 CI、最终服务包/容器复验与数学正文验收见下方最终候选记录；公网部署仍待验收。

### 验证期间的磁盘与环境处理

- 首次 Windows 全量编译因 E 盘耗尽而失败（LNK1318、`no space on device`），不记作测试通过。临时转到 C 盘后，用户要求所有编译在 E 盘；立即停止该运行，保留部分日志到 E 盘，清掉约 2.8 GiB 的 C 盘临时编译缓存。后续构建仅使用 E 盘或位于 E 盘的 Debian 虚拟磁盘。
- 用户明确批准清理本 worktree 的 `target/debug` 与 `target/wasm32-unknown-unknown/debug`。PowerShell 删除被安全钩子拒绝后，改用 Cargo 官方 clean：先逐路径核对 dry-run，再分别清理 dev profile，共 47.8 + 7.3 GiB。源码、handoff、release 产物和其他 worktree 保留。
- VHDMP 事件记录 Debian VHD 写失败 `0xC000007F`；释放空间并重启空闲 WSL 后恢复。旧 WSL 测试二进制被 `file` 判为 `data`，执行报 `Exec format error`，故改用新的 Linux target 完整重编，不复用损坏缓存，不修改/重建用户 VHD。
- 旧分支仅作只读核对：部分分支已合入 main；`feat/reader-mathml` 等仍有未合并内容。未为释放空间而合并功能或删除其他 worktree。

### 手工验收

- 尚未完成实际公网 Pages 部署与最终独立评审；本地子路径/自动浏览器结果不代替这些门禁。
- 目视验收发现真实内容缺陷：每日数学第一篇的 120 个原始 MathML 公式被当前 ammonia 策略删除；陶哲轩文章使用 144 张 LaTeX 图片；补上图片加载等待后全部 144 张正常加载，先前截图过早。原始 RSS 快照包含这些内容，存储转换没有删它们；现有“正文长度 > 100”断言不足以覆盖公式保真。
- `feat/reader-mathml` 有经四轮评审的 MathML 保留方案以及后续解析预算工作；该分支同时包含旧架构与其他安全变更，不能直接用旧文件覆盖当前安全策略。已在 `c2e2278` 按现有 infra 正文契约适配，保留当前 `data:` URL 限制；详见 [公式整合记录](./2026-10-22-reader-mathml-integration.md)。

## 结果

阶段 D 核心已提交 `9b70455`，候选 `576cba4` 的远端自动验收通过；独立评审与实际公网 Pages 验收未完成，尚不可打 tag。

## 风险与后续事项

- 主要风险：WASM 时钟与任务生命周期、SQL 引擎版本差异、页面线程阻塞、遗漏旧测试语义。
- 最终阶段 C/D 独立评审方式已向用户询问；不阻塞实现。
- 无 tag、Release 或 GHCR 发布授权；这些动作仍等待用户确认。后续用户已要求处理完剩余工作再合并 main，条件与进度见[分支整合记录](./2026-10-22-branch-integration.md)。

## 给下一位 Agent 的备注

- 设计入口：`docs/design/v0.2-architecture.md`，存储节与阶段门禁。
- RSS 快照沿用 `assets/pages-demo/`，不得将真实内容替换为合成样例来通过验收。
- 已有阶段 C 证据见 `2026-10-22-c4c-round3-closure.md`、`2026-10-22-c6-web-distribution.md`、`2026-10-22-ci-acceptance-timeouts.md`；新候选须重新验证。

## 首轮远端候选与工具链修正

- `c2e2278` 已推送；CI `39863471290`（long）、Pages `39863477970`、四平台 Web 包 / Docker `39863485165` 已触发，不发布。
- CI 的 `web-smoke` 和 `wasm-contract-build` 都因 `cc-rs: failed to find tool "llvm-ar"` 失败；runner 为 Ubuntu 26.04，原生 Clang 编译已进行到归档阶段，不能据此称为业务测试失败。
- 修正：新增共用 `setup-wasm-c-toolchain` action，解析 Clang 的真实路径，使用同一安装目录的 `llvm-ar`，先实际编译与归档一个 wasm32 C 对象，再导出 Cargo 的 CC/AR 路径；CI 的两个构建 job 与 Pages 共用。
- 本机 WSL Debian 同样没有未带版本号的 `llvm-ar` 命令，但 `/usr/lib/llvm-19/bin/llvm-ar` 存在；直接执行 action 的完整脚本通过，探针输出全部写在本 worktree 的 E 盘路径。actionlint 与 diff 检查通过。
- 工具链修正已提交并推送 `576cba4`；远端实际采用 `/usr/lib/llvm-21/bin/llvm-ar`。

## 最终候选远端结果（576cba4）

| 验证 | 结果 | 证据 |
|---|---|---|
| 完整 CI，`acceptance_long=true` | **35 / 35 jobs 成功**；14 个 WASM targets、7 crate 原生检查、Android smoke、5 个主题与两端 release-ui runner | [39864068134](https://github.com/Develata/RSS-Reader/actions/runs/39864068134) |
| 服务器长场景 | **27 / 27 场景成功**；原三个超时场景 `reader-recovers-without-localizing`、`settings-read-fails-at-start`、`http-backend-only` 分别为 3、3、2 秒 | default UI job `120124304752` |
| Pages | fresh RSS 抓取、release 构建、6 篇正文 / 公式保真、内存重置、子路径及禁用在线操作均通过；**公网部署 skipped** | [39864073043](https://github.com/Develata/RSS-Reader/actions/runs/39864073043) |
| 四平台 Web 包 / Docker | **7 / 7 jobs 成功**，Linux / Windows / macOS Intel / macOS ARM 实际启动服务包，容器实际包检查通过；未发布 | [39864123959](https://github.com/Develata/RSS-Reader/actions/runs/39864123959) |

- 精确 source SHA：`576cba434274b4b686480455db2f7f2811bd107a`。相对于 `c2e2278`，只有 CI 配套归档器选择与交接记录变化，产品源码不变。
- Pages 本次从指定两个源重新抓取，2026-10-22 22:20:28 UTC 写出 356,387 bytes 快照；不是把仓库旧快照当新抓取。
- 运行、27 个场景及 artifact ID / GitHub digest 见 [verification.json](../testing/evidence/2026-10-22-phase-d/verification.json)。digest 是 GitHub 报告值；未声称本机下载并独立复算 ZIP。
- 源码工作树在本节记录前 clean；没有 tag、Release 或镜像发布。
- 记录勘误：按超时调查原始场景名与 `verification.json` 的最终日志逐项核对，修正上表误列的场景名；27 / 27 的结果不变。这里只更新文档，没有重复触发 CI。

## 分支清理与主干整合预检

- 本任务已批准的两处 debug 编译缓存共清理约 **55.1 GiB**；重新编译后 E 盘可用约 **46.4 GiB**。本机后续编译输出均在 E 盘，WSL Debian VHD 也位于 E 盘。
- 已核对其他 worktree 与合并状态；未删除其他 worktree、未合并未审核功能分支，也未修改根检出的任务外改动。单独删除分支指针对空间帮助有限，主要占用是各 worktree 的编译缓存。
- 刷新 `origin/main` 后为 `d9ebe35`，与候选左右差异 27 / 343 个提交；`git merge-tree --write-tree --name-only HEAD origin/main` 返回 1、**65 个冲突路径**。报告：`target/handoff/candidate-576cba4/main-merge-preview.txt`。这是预检，没有开始工作树合并。
- 主干的旧 host / sqlx 路径与已批准的 v0.2 Backend / 统一 SQL 路径发生职责、删除和改名冲突；需要按 v0.2 契约逐项对齐，不能把它作为普通磁盘清理直接合并。候选核心语义仍在 application/domain，平台差异仍限于 infra/backend/host capability。

## 剩余门禁与交接

- 最终独立复审方式仍待用户选择；具体复审包已写在 `target/handoff/2026-10-22-final-cd-review.md`，含精确候选、范围、接受的风险和验证证据。
- 实际公网 Pages 子路径尚未部署验收；本地及 CI 静态服务器通过不能替代它。
- 主干整合方案与后续 Pages 部署需处理上述架构分叉；尚不可宣布全部发布门禁关闭。
- 保持用户要求：达到可打 tag 后再等点头，本次未打 tag。

# C4c 远端推送与 CI 验证

> 历史交接：本记录仅说明当时的实现与验收，旧路径和命令可能已退役。当前状态见[文档索引](../../../README.md)，归档范围见[历史索引](../README.md)。

- 日期：2026-10-22
- 作者 / Agent：Codex
- 分支：`v0.2-architecture`
- 首轮受测 HEAD：`d292d8663d9e07dd43f74b57b5c465fa713dfbc3`
- 镜像修正后受测 HEAD：`fc95f5c43cb2fcaeea6a7b9cb6d22c4fdf07a634`
- 当前 HEAD（最终文档提交前）/ 最终受测 SHA：`509ea3528e09d9159d0edf73326db3de13cb526a`
- 相关 commit：`cedaad1`（CI 覆盖补齐）、`d292d86`（本地验证记录）、`fc95f5c`（系统依赖下载修正）、`25499ea`（资料库关闭顺序）、`509ea35`（UTC 测试规范化）。本记录随 `docs(ci): record C4c remote verification and fixes` 提交归档，该提交的父提交为 `509ea35`，只包含文档。
- 相关 tag / release：N/A
- 状态：`validated`（本次 Windows / WSL 定向验证通过；最终远端 CI 24 / 24 成功）

## 工作摘要

用户在 CI 本地更新完成后明确要求“push 远端试一下”。此授权覆盖推送当前 `v0.2-architecture` 并运行 CI，取代本次验证所涉及的 C4 中间不推送限制；不含合并、发布、C6 或独立复审。

## 影响范围

- 工作树：`E:\gitclone\RSS-Reader\target\audit-20261009\worktree`；主检出的用户改动保持原状。
- 远端：`origin` → `https://github.com/Develata/RSS-Reader.git` 的 `v0.2-architecture` 分支。
- 平台：GitHub Actions Ubuntu / Windows、wasm Chrome、Android ARM64。
- 变更：远端分支、CI 的 apt 下载步骤、infra 关闭连接的顺序、UI 测试的时间规范化与运行记录。

## 关键操作

- 推送前工作树干净，HEAD 为 `d292d86`；远端尚无该分支。
- 首次 HTTPS 推送返回 HTTP 408，复核分支未创建；普通 `git push --set-upstream origin v0.2-architecture` 重试成功。未使用 force、未推 tag。
- `git ls-remote --heads origin refs/heads/v0.2-architecture` 确认远端为完整 `d292d8663d9e07dd43f74b57b5c465fa713dfbc3`。
- 手动执行 `gh workflow run ci.yml --repo Develata/RSS-Reader --ref v0.2-architecture -f acceptance_long=true`。本分支 push 不触发 main 专用工作流；本次只调度 CI。
- [CI run 39776370021](https://github.com/Develata/RSS-Reader/actions/runs/39776370021)，事件 `workflow_dispatch`，受测 SHA 与上述远端一致。

## 验证与验收

- 远端分支 SHA 核对：通过。
- 首轮 CI：两次尝试均因依赖下载延迟主动取消；第一次尝试已有 22 项成功，仅 `rssr-app` 尚在安装依赖。修正下载后的新 run 完成，21 项成功、2 个原生测试 job 失败、汇总门禁失败，详见下文。
- Web 构建日志确认 Ubuntu 26.04.1（runner image `20261012.163.1`）、Rust 1.99.1、Dioxus CLI 0.7.9；四组 wasm Clippy 与 release 页面、服务器、夹具工具构建通过。
- 首轮 Android app / CLI ARM64 check、app bundle 与 APK 断言通过；两个平台 release UI runner、三个 wasm Chrome 契约和六个非 UI crate 均通过。首轮 `rssr-app` 未进入 Rust 检查，不能据此声称总门禁通过。
- 默认主题的日志确认 `ACCEPTANCE_LONG: true`、Chrome for Testing `156.0.8078.47`；服务器完整构建标识为 `dev-d292d8663d9e`。

首轮 `d292d86` 已下载五个 `ci-ui-*` artifact，逐份核对 `assertions.json`：

| 浏览器套件 | 远端结果 | 意外控制台错误 |
| --- | --- | --- |
| default regression | 337 / 337 通过 | 0 |
| atlas-sidebar、newsprint、amethyst-glass、midnight-ledger | 各 177 / 177 通过 | 各 0 |
| theme-matrix | 6 / 6 通过 | 0 |
| feed-smoke | 3 / 3 通过 | 0 |
| acceptance `--long` | 27 / 27 场景、108 / 108 检查通过；`long: true`，0 skipped | 0 |

- 本地产品验证与 CI 编排验证分别见 [C4c-2 / C4c-3](./2026-10-22-c4c2-verification-c4c3.md) 和 [CI 覆盖补齐](./2026-10-22-c4c-ci-coverage.md)。
- 本次本地远端操作证据保存在忽略目录 `target/handoff/evidence-c4c-ci-remote/`；浏览器摘要为 `browser-facts.json`，下载的原始报告、日志与截图在 `artifacts/ci-ui-*/`。

### 原生 UI 依赖下载延迟与重试

- 首次 `native-module (rssr-app)` job `119816362129` 于 12:12:14 UTC 开始安装系统依赖。约 20 分钟后仍在下载，尚未进入 Rust；其余执行项已通过。为取得运行日志，于 12:31 UTC 请求取消该 run，最终于 12:32:37 UTC 记录为 `cancelled`。
- 完整日志 `native-app-attempt1.log` 显示 apt 需下载 76.3 MB，含 172 个新包和 12 个升级包；`azure.archive.ubuntu.com` 的包多次出现 `Ign`，间隔约 30 秒，部分随后下载成功。可确认延迟发生在下载阶段；没有证据将其归因于产品代码，也未确定具体网络原因。
- 使用 GitHub 的单 job 重跑接口重跑原生 UI 及依赖门禁，受测 SHA 不变；第二次 job 为 `119826926216`，12:35:06 UTC 启动。通过的浏览器及其他模块没有重跑。
- 第二次安装仍持续缓慢，约 8 分钟后请求取消，日志于 12:44:05 UTC 记录取消。`native-app-attempt2.log` 再次显示从 Azure Ubuntu 镜像下载包缓慢，仍未进入 Rust。
- `fc95f5c` 将该临时 runner 的 Ubuntu 源从 `azure.archive.ubuntu.com` 改到 HTTPS `archive.ubuntu.com`，保留发行版 suites 和原有依赖包；更新与安装使用 20 秒连接超时、两次重试，安装步骤上限为 10 分钟。支持既有 `sources.list` 与 deb822 `ubuntu.sources`；security 源保持原值。
- 本地 actionlint 通过；WSL ShellCheck 的 25 个 CI `run` 块、四组长验收参数转发检查通过。用实际 sed 表达式验证 legacy HTTP / deb822 HTTPS 源替换，发行版 suites 和 security 源保持原值，结果通过。此验证不代替远端真实 apt 安装。
- 已普通推送 `fc95f5c`，并以 `acceptance_long=true` 触发 [修正后 run 39781265422](https://github.com/Develata/RSS-Reader/actions/runs/39781265422)。原生 UI 的 apt 安装用时 53 秒（12:47:45–12:48:38 UTC），Clippy 通过，随后进入真实测试并暴露下述两处问题。

### 真实测试失败与修复

- CLI job `119832908834` 的 `the_library_is_closed_before_the_cli_exits` 失败：命令成功退出后仍有 `library.sqlite3-wal` / `-shm`。本地 WSL 单次测试和 200 次顺序启动通过；限制为 2 个 CPU、5 路并发、每次使用独立目录，500 次真实 CLI 启动中 4 次留下两个文件，复现远端现象。
- 原因：旧执行器同时断开读、写队列，多个连接会同时进入 SQLite 关闭路径。当前锁定依赖的 `sqlite3WalClose` 只有取得 exclusive lock 才进行最后的 checkpoint / WAL 清理，连接同时关闭时可能互相阻挡，关闭 API 返回成功却留下日志。
- 修复在 infra 内完成：先封住全部队列准入，保留 writer 的 sender，等 reader worker 全部退出后才放开 writer 的最终关闭；已排队任务继续执行，关闭错误仍返回，异步 / 阻塞入口保持原契约。增加并发调用 `close` / `close_blocking`、等待运行中的读事务并清理 WAL 的测试。没有改 application / domain，也不删除残留文件来掩盖问题。
- UI job `119832908878` 的 `ui_commands_produce_the_recorded_intents` 失败：Ubuntu UTC 时区输出 `2026-04-01 10:00 UTC`，测试只识别 `UTC+hh:mm`，没有规范化成既有基准中的 `2026-04-01T10:00:00Z`。本地 WSL 以 `TZ=UTC` 运行同一测试复现同一行差异。
- 修复仅补充测试 normalizer 对裸 `UTC` 的处理，并覆盖正负时区、半小时时区、非法输入。产品时间显示和既有 `characterization.transcript` 未改变。
- 修复后同条件 500 次 CLI 启动全部正常退出，未留下 WAL / SHM（`close-after-parallel.json`）。这是一轮定向复测，不构成概率或长期稳定性保证。
- WSL `TZ=UTC`：`cargo fmt --all --check` 通过；`cargo test --locked -p rssr-infra --lib store::` 24 passed；`--test test_library_store` 30 passed / 1 ignored；CLI 11 passed；app 主测试 201 passed / 1 ignored、两个集成目标各 2 passed；`cargo clippy --locked -p rssr-infra -p rssr-cli -p rssr-app --all-targets -- -D warnings` 通过。
- Windows 运行相同命令：fmt 通过，store 单元测试 20 passed，store 契约 29 passed，CLI 10 passed，app 主测试 200 passed / 1 ignored、两个集成目标各 2 passed；相关三 crate 的全部 target Clippy 通过。两平台顺序编译，Cargo 均限制为 2 jobs。
- 同一个修复后 UI 测试二进制，另以 `TZ=Asia/Shanghai` 和 `TZ=America/New_York` 运行 `ui::runtime::characterization::`，各 2 passed；原基准文件无改动。
- `25499ea` / `509ea35` 已普通推送，2026-10-22 13:15:22 UTC 调度 [完整 run 39785231034](https://github.com/Develata/RSS-Reader/actions/runs/39785231034)，`acceptance_long=true`，受测完整 SHA 已核对。修复前后证据在 `target/handoff/evidence-c4c-ci-remote/`，包括 `close-before-parallel.json`、`ui-utc-before.log`、`windows-fix-results.json`、`wsl-fix-results.json` 与逐项验证日志。

### 最终远端验证（509ea35）

- [CI run 39785231034](https://github.com/Develata/RSS-Reader/actions/runs/39785231034)：`completed / success`，完整受测 SHA 为 `509ea3528e09d9159d0edf73326db3de13cb526a`；24 / 24 个 job 成功，其中包含 `lint-and-test` 汇总门禁。最后的汇总于 13:24:22 UTC 完成，整轮从创建到完成约 9 分钟。
- 七个原生 crate 的 Clippy / 测试、四组 wasm Clippy、Windows / Ubuntu release UI runner、三个 Chrome wasm 契约、Android app / CLI ARM64 编译及 app bundle / APK 断言、Web 构建、五个主题浏览器 job 全部通过。
- 原失败测试 `the_library_is_closed_before_the_cli_exits` 和 `ui_commands_produce_the_recorded_intents` 均通过；新增 UTC normalizer 测试通过。原生 UI 主测试 201 passed / 1 ignored，两个集成目标各 2 passed；CLI 11 passed。
- 默认主题 job 日志确认 `ACCEPTANCE_LONG: true`、Chrome for Testing `156.0.8078.47`；服务器日志确认构建标识 `dev-509ea3528e09`。
- 五个本轮 UI artifact 已下载，八份 `assertions.json` 逐项核对：default regression 337 / 337；四个其他主题各 177 / 177；theme-matrix 6 / 6；feed-smoke 3 / 3；长验收 27 / 27 场景、108 / 108 检查，`long: true`、0 skipped。八份报告均为 pass、0 失败检查、0 意外控制台错误。明细与各报告 SHA-256 在 `browser-facts-final.json`，原始证据在 `artifacts-final/`，与首轮的 `artifacts/` 分开保存。
- newsprint 的 CLI 下载遇到 TLS handshake timeout，改经 GitHub connector 下载后成功；ZIP SHA-256 与 GitHub artifact `12427834821` 的 digest 一致。此下载问题发生在成功 run 之后，不影响已完成的 CI 结果。
- 运行元数据与每个 job 的结果保存在 `run-final-completed.json`。本地源码与 workflow 相对受测提交没有后续改动；交接记录和设计 / 验证说明在验证完成后单独提交。

## 结果

- `v0.2-architecture` 的修复已推送，完整远端 CI 通过；本记录与四份说明文档作为最后的 docs 提交归档。最终分支 HEAD 比受测 SHA 多这一笔纯文档提交，源码和 workflow 与受测提交一致，不额外重复整轮 CI。

## 风险与后续事项

- CI 通过与独立代码复审是不同的验收；原待复审项和本次新增的关闭顺序修复留待后续独立复审，本轮没有运行独立 review。
- 不合并、不打 tag、不发布；仍停在 C6 之前。

## 给下一位 Agent 的备注

以本记录中的 run URL 与完整受测 SHA 为准，不能把旧的本地验收数字当成此 run 的远端结果。最后的 docs 提交可用 `git log -1 -- docs/handoffs/2026-10-22-c4c-remote-ci.md` 定位；其父提交必须是受测 `509ea35`。主检出与 detached 验证检出的源码未修改，未清理用户文件。

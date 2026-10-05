# web-auth Rust 生命周期迁移：阶段 1a

- 日期：2026-10-05
- 作者 / Agent：Codex（WSL 主实现，Windows 原生补验）
- 分支：`refactor/web-auth-owned-process`
- 当前实现 HEAD：`f0bf99742ede5bbbf9476874cec27e597959b695`；后续提交仅补测量/交接证据
- 相关 commit：`f0bf99742ede5bbbf9476874cec27e597959b695`（实现）；本次证据提交见本文件 Git 历史
- 相关 tag / release：N/A；不合并、不发布、不替换 v0.1.22 附件
- 状态：`draft`

## 工作摘要

按已批准的四阶段工具迁移，先交付 web-auth 的最小生命周期迁移。
后续 HTTP、测试服务器、metadata 和浏览器 host 尚未实施；
本次 20 对实测已触发性能停止门禁。候选只作为 Draft PR 审查，后续阶段停止；
最终 PR 精确 head 的 CI 终态会记入 PR 描述及本机 evidence/ci-final.json。

## 影响范围

- 模块：`scripts/release-ui/`、认证兼容入口与单份断言 adapter。
- 平台：WSL Debian/Linux 主开发；Windows 原生验证快照。macOS 未本机验证。
- 额外影响：runner 文档和既有 CI 测试矩阵；无产品 application/domain/infra 或页面 JS 改动。

## 关键变更

- Rust 调用既有 `OwnedProcess` API 管理 cargo、readiness curl 和断言进程；
  原阶段名、顺序、退出码语义及 summary schema 保持。
- `--web-auth-only` 供兼容入口及独立认证检查使用，`--plan` 如实显示内建路径。
- 30 次请求、connect 2 秒/max 10 秒、失败 sleep 1 秒的预算保持；不是 30 秒硬截止。
- bundle profile 与服务 Cargo profile 分离；没有向服务增加 `--release`；
  聚合 `--skip-build` 仍只跳过 dx。
- 日志分离；Windows readiness 选择调用者 PATH 的 curl，
  断言继续保留已有 MSYS `next=` 排除。
- 自审修正中途服务退出的错误码：保留断言 curl 实际返回的 52/56，不用通用 1 覆盖。

## 验证与验收

### 环境与基线

- Debian WSL：`/home/deve/gitclone/RSS-Reader`，虚拟磁盘 `E:/WSL/Debian`。
  起始工作区干净；fetch 后 origin/main 与审查基线一致。
- Windows：`E:/gitclone/RSS-Reader` 位于 `b745a846`，原有 22 项删除、
  两份 handoff 修改和 `.workbuddy/` 均保留；仅在 ignored target 导出补验快照。
- 已读两个环境的 Codex memory_summary 项目记录；未读会话原始存储。
  WSL `.agents/skills` 的 speckit 实施/宪章 skill 已检查，属于历史 spec 流程，
  本次依据已批准迁移计划实施，未另起 spec 或任务。
- WSL rustc/cargo 1.97.0；Windows rustc 1.98.1。两平台不合并性能样本。
- 隔离证据根：`target/tool-migration-20261005/`。起始 WSL 逻辑可用 537 GiB，
  E 盘物理可用约 135 GiB；不清理用户缓存。
- 固定 A 源码快照与 binary：`baseline-source/`、`baseline-build/`。
  离线初次构建因未缓存既有 process-wrap 失败，单独 fetch 已有锁定依赖后成功。
  下载 14.94 秒、clean build 9.32 秒、no-op 0.136 秒；均为单次数据。
- A 现有 20 项黑盒验收通过（57.17 秒）；无新增依赖。
- 实际 rssr-web 隔离构建通过（35.54 秒），用于双方相同服务输入；
  此成本单列，不作为工具迁移编译收益。

### 自动化验证

- WSL：独立工具 fmt/clippy/unit 已通过，新增后的 26 项黑盒已通过；
  自审后完整回归中 20 项既有用例和 5 项新用例通过；断流测试环境修正后，
  最终 6 项新认证用例全通过（仍待 CI 整套复核）。日志 `evidence/B-validation-review-fixes.log`。
- 真实 curl + HTTP peer：login、302/303 redirect、next=/feeds、cookie、
  session-probe 204、feeds/settings 200、logout；慢 readiness 超过 30 秒、
  卡住请求取消、孙进程、服务早退、stdout 无读取、空格/中文路径、失败退出码。
- Windows 第一次发现 readiness 选中系统 curl，已加调用者 PATH 解析并验证原调度失败消失。
  第二次完整验收中，嵌套快照长路径触发 MSVC LNK1104（6 个 launcher 子用例）；
  原始失败日志保留。现导出到更短的 `E:/gitclone/RSS-Reader/target/w1` 重跑。
- Windows 短路径完整 26 项执行：24 项通过（含全部 launcher 子用例）、1 项 Unix-only skip、1 项失败；
  唯一断流错误码用例随后修正测试预期；最终 6 项认证用例全部通过（49.36 秒）。
  本机完整日志 `E:/gitclone/RSS-Reader/target/w1/acceptance.log`，
  最终认证日志 `auth-final.log`，原生 clippy 通过。
- 断流夹具需显式为 localhost 设置 NO_PROXY；否则 WSL 的调用者代理会把断流改写为
  502。生产请求仍继承调用者环境，不偷偷覆盖代理配置。Windows curl 断流可返回 56，
  Unix 可返回 52；测试验证实际 curl 诊断与 runner/summary 的代码一致。
- 真实 rssr-web：旧 Bash 和新 Rust 两条路径认证都通过，结束后端口关闭。
  兼容三参数脚本也通过；日志 `evidence/real-product-A`、`real-product-B`、
  `compatibility-entry`。同一服务 binary、同一已有静态 bundle，没有浏览器/UI 验收结论。
- 远端 CI：本证据提交在首次 push 前写入；最终 PR head 的 checks/结论在 PR 描述
  追加，并保存在本机 `target/tool-migration-20261005/evidence/ci-final.json`。
  任何 CI 通过均不解除下面的性能阻塞。

### 手工验收

- 未运行浏览器 UI/Pages/macOS 验收；本步仅改变认证 smoke 的宿主生命周期。
- 重用已有 bundle 的 HTTP 返回不代表新鲜 Dioxus/UI 浏览器验收。

## 性能比较与原始证据

- 固定 A：`f69087a61a4627cbd3ba88fa082d7722df811dc5`；
  固定 B：`f0bf99742ede5bbbf9476874cec27e597959b695`。
- [原始数据/复跑脚本](../testing/evidence/2026-10-05-web-auth-owned-process/)：
  `paired-raw.json`、`paired-summary.json`、`measure.py`；每类 20 对，
  AB/BA 平衡交错，各有独立暖机。所有运行失败数为 0，40 次真实 auth 采样
  均检查服务 binary 无活进程、监听端口已关闭。
- 服务使用相同隔离构建产物、debug bundle 与 loopback NO_PROXY；
  一个公共 Cargo cwd shim 让 A/B 都从本 WSL checkout 启动同一产品。
  只测 web-auth；后续 browser stage 用 exit-0 stub，不计作浏览器验收。
- clean build：A 9.320 秒、最终 B 8.750 秒，各 n=1。中间开发态 B 11.571 秒
  也保留在原始记录中；这些单次编译数据不能证明提速。
  wrapper 首次独立构建 A 9.154 秒、B 9.451 秒；真实兼容入口首次执行含构建
  15.436 秒，不与暖机 runtime 混算。
- debug binary：A 21,311,880 字节、B 21,446,360 字节（+134,480，约 +0.63%）。
  新增依赖为 0；未测内存，不宣称内存下降。

| 测量（WSL，n=20/方） | A median ms | B median ms | 配对 B-A median ms |
| --- | ---: | ---: | ---: |
| 直接 binary --plan | 4.258 | 4.221 | -0.015 |
| Cargo no-op build | 115.112 | 115.200 | +0.049 |
| 已暖机 wrapper --plan | 114.604 | 114.674 | +0.082 |
| auth 调用总体（含 stub 后续阶段） | 1670.068 | 3226.464 | +1556.092 |
| **runner 内部 web-auth 阶段** | **1576.5** | **3121.5** | **+1546.5** |

内部阶段 A IQR 40.25 / MAD 6.5 / min 1536 / max 1656 ms；
B IQR 15 / MAD 3.5 / min 3117 / max 3199 ms；
配对差 IQR 14.25 / MAD 8 / min +1464 / max +1595 ms。
其他各组完整 n/median/IQR/MAD/min/max/失败数见 JSON。

外部墙钟包含 Python 有界 wait 轮询（可达约 50 ms 粒度），不能用很小差值宣称
收益；内部阶段 duration_ms 不含这一观察误差。readiness 包含在阶段中，
未单独测量；没有把约 330 秒预算改写成统计 readiness 值。不报告 p99。

## 结果

- **BLOCKED：显著性能退化，停止后续迁移。** 真实 auth 阶段 median
  约增加 98%，每对至少多 1.464 秒，远超 A 的 40.25 ms IQR；
  不需要放宽预算才能判定这是实质退化。
- 差值与直接拥有仍在运行的服务后，既有 Unix `OwnedProcess::stop`
  固定完整等待 1.5 秒的源码路径吻合。这是源码与测量支持的归因，
  本次未再改 process.rs 去规避门禁。
- 功能候选供 Draft PR 审查；不能标作性能验收通过、可合并或可发布。
  HTTP 断言仍为单份 Bash/curl/grep adapter。

## 风险与后续事项

- 下一步建议先单独审查服务退出后的 grace 等待：保留 1.5 秒作为处理顽固后代的
  上限，并证明整个所属进程组已经退出时能提前返回；不能仅缩短常量牺牲清理保障。
  需重新获得对停止点之后工作的方向确认，再改共享进程清理或进入 HTTP 迁移。
- 尚未引入 HTTP/WS/plist 库。无需把验收工具并入产品 rssr-cli。
- 阶段 2 的 HEAD/symlink 修复、三个平台 metadata、Chrome ownership 均未推进。

## 给下一位 Agent 的备注

- 主实现只在 Debian WSL；不要覆盖 Windows dirty 工作，也不要改用 C 盘构建。
- 入口：`scripts/release-ui/web_auth.rs`；单份 HTTP 合同：
  `scripts/run_rssr_web_auth_assertions.sh`；黑盒：
  `scripts/release-ui/tests/acceptance.py` 和 `auth_server.py`。
- WSL 本次基线/构建/测量目录约 2.6 GiB；结束时 WSL 可用约 532 GiB，
  E 盘物理可用约 129 GiB。另有两端验收产物；未清理用户缓存。
- 原始测量的精简 JSON/复跑脚本已入库，完整服务和失败日志保留 ignored target；
  Windows 两份补验快照仅用于测试，原 checkout dirty 状态未变。

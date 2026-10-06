# web-auth 退出观察延迟归因与有限轮询优化

- 日期：2026-10-06
- 作者 / Agent：Codex（Debian WSL 主开发，Windows 原生补验）
- 分支：`refactor/web-auth-owned-process`
- 当前实现 HEAD：`4cc564201503b0e1e03413f45cb257f548e55037`
- 相关 commit：实现 `4cc5642`；本证据提交 commit: pending（以包含本文件的提交为准）
- 相关 tag / release：N/A；不合并、不发布、不替换附件
- PR：[#23](https://github.com/Develata/RSS-Reader/pull/23)，保持 Draft
- 状态：`validated`（本地验证；最终精确 head CI 回执随后附于 PR）

## 工作摘要

按用户恢复授权，仅归因 PR #23 剩余延迟并做两处有限 sleep 优化。
30 组固定 A/B/C 未跟踪比较中，C 相对修改前 B 的 web-auth
配对耗时差 median **−71.5 ms**，29/30 更快；相对原始 A 为 **−7 ms**。
分段跟踪支持收益来自 readiness 退出观察与快速消失进程组的清理等待。
这支持保留本次修改；没有扩大到 HTTP 重写或后续迁移阶段。

## 影响范围

- 模块：`scripts/release-ui/web_auth.rs`、通用 `process.rs` 的 Unix stop。
- 平台：WSL/Linux 实现与性能；Windows 原生补验；macOS 无本机证据。
- 文档：runner 说明、本交接与
  [完整证据](../testing/evidence/2026-10-06-web-auth-polling-latency/README.md)。
- 主路径 `/home/deve/gitclone/RSS-Reader`，WSL 位于 E 盘；Windows 仅在
  `E:/gitclone/RSS-Reader/target/w3` 验证导出快照。
- 用户 Windows dirty、三个既存 dirty worktree 原样保留；无产品业务/页面 JS 变更。
  未创建子任务、启用 fast、更换模型设置或清理缓存。

## 关键变更

- readiness 前几个等待间隔为 5/10/20 ms，随后回到 40 ms。
- Unix stop 前两个等待间隔为 5/10 ms，随后回到 25 ms；通用 stop 的
  其他调用也适用。均有真实 sleep，额外探测次数有限。
- 完整保留 1.5 秒顽固后代宽限：先 reap leader，
  且仅 `kill(-pgid, 0)` 得到 ESRCH 才提前结束。组存在或 EPERM 等不确定
  结果继续宽限与强制清理，未降级为只检查 leader。
- adapter 通用等待、1 秒 retry、Console.finish、Windows Job Object、
  Unicode 文件名 adapter、MSYS next=/feeds 均未修改；无新增依赖。
- 不改变阶段名称/顺序、退出码、schema、30 次 2/10 秒 readiness、
  profile 仅选择 bundle、skip-build 仅跳过 dx 或单份认证断言。

## 验证与验收

下列 WSL 命令的 target 参数使用独立
`target/tool-migration-20261006-polling/validation-build`，全部通过：

- `cargo fmt --manifest-path scripts/release-ui/Cargo.toml --check`
- `cargo clippy --manifest-path scripts/release-ui/Cargo.toml --all-targets --locked --offline --target-dir <target> -- -D warnings`
- `cargo test --manifest-path scripts/release-ui/Cargo.toml --locked --offline --target-dir <target>`：4 单元。
- `cargo build --manifest-path scripts/release-ui/Cargo.toml --locked --offline --target-dir <target>`
- `python3 scripts/release-ui/tests/acceptance.py --bash /bin/bash --binary <target>/debug/release-ui -v`：
  全部 28 黑盒通过，suite 73.153 秒。
- Windows 同等 fmt/clippy/build、3 单元通过。首次与 WSL 并行回归时默认
  18081 端口冲突，8 个 failure（含 subtest）全部保留。随后两侧只读检查
  未见监听，端口可绑定；未定位短暂占用者。未改测试或杀进程，串行原样重跑：
  26 pass、2 Unix-only skip，suite 119.707 秒。
- 固定 C 快照上的真实三参数兼容入口、中文空格日志目录通过；
  9.775 秒含独立快照 target 的首次编译，不作运行时收益样本。
- 回归包含顽固孙进程、完整宽限、取消、卡住/慢请求、超过 30 秒 readiness、
  stdout 无读者、端口占用、中文空格路径、caller PATH 与真实认证合同。
- 30 组未跟踪最终运行、20 组初始归因及 20 对复测跟踪共 210 次运行：
  全部 exit 0、端口关闭，无记录到的固定产品 executable 残留。
- 根 workspace、浏览器/Pages/Android 本轮本地未重跑；由最终 PR head CI
  验证，不能借用旧 1b710ae CI。真实 macOS 未跑。
- Python 统计重算与 80 份原始 trace 再解析结果由证据审计脚本核对。

## 性能结果与边界

固定 A=`f69087a`、B=`1b710ae`（运行时代码等同 e6915a）、
C=`4cc5642`；WSL Rust/Cargo 1.97.0，同一产品 binary、
同一 debug index.html 输入。后续 browser stage 为统一 stub；
测量的是 web-auth 完整阶段，不是整体浏览器验收。无产品重新编译。

| 阶段耗时 ms | n | median | IQR | MAD | min | max |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| A | 30 | 1536 | 1.75 | 1 | 1494 | 1576 |
| B | 30 | 1617 | 40 | 20.5 | 1557 | 1679 |
| C | 30 | 1528 | 25.75 | 9 | 1478 | 1649 |
| 配对 C−B | 30 | −71.5 | 67.5 | 41 | −154 | +10 |
| 配对 C−A | 30 | −7 | 32.25 | 8 | −56 | +115 |

六种顺序各 5 次，任意两版本先后顺序各 15 次；每版每个位置各 10 次。
全部保留，无删点。预定每版本一次暖机另列；最初独立诊断 A 的 K=3 原样保留。
后续 80 个计划内跟踪样本 K 均测得 2，但不把它当作未跟踪样本的已知值。

复测分段 B/C median：readiness 退出观察合计 68.878→9.507 ms，
stop 26.338→6.278 ms；retry 约 1001 ms。readiness wait 调用 4→5，
整个阶段 root wait 53→55；组探测仍为 2。adapter 未改，其浮动不计作优化。
初始 A 实际服务 stop median 2.032 ms；不能扣除旧独立 1.5 秒清理探针。
跟踪减未跟踪的配对 median 为 A +90.5/B +49.5 ms，因此跟踪总时间不作提速证据。

整个调用监督器主线程 CPU median A/B/C=6.430/7.572/7.076 ms；
配对 C−B median −0.420 ms（23/30 较低），C−A +0.865 ms。
wait4 统计 CPU 的配对 C−B −1.560 ms，15 低/15 高，未见一致增长。
主线程 timeslice 配对 median +1.5。该结果不证明所有机器上 CPU 更低；
maxrss 也不是整棵并发进程树的峰值，不宣称内存下降。

独立 clean tool build B 7.480/C 7.370 秒，no-op 66.842/78.492 ms，
均各 n=1，不下统计结论。debug binary +15,968 bytes（约 +0.0744%）。
原始样本、quartile/MAD 定义、工具链/hash、每条命令、编译成本、trace 扰动、
Windows 首次失败和成功重跑均在证据目录；不报告 p99。

## 结果

建议保留优化；本地未再看到相对原基线约 0.1 秒的系统性 median 退化。
但 C/A 仍有 9/30 更慢、最大 +115 ms，不能宣称零退化或尾延迟保证。
本阶段若最终精确 head CI 全通过，可建议进入合并审阅；合并由用户决定。
PR 保持 Draft，不因本次改善自动进入 HTTP Rust 化或阶段 2–4。

## 风险与后续事项

- 性能仅适用于本机 WSL 固定输入；Windows 本轮为功能补验，未测性能。
- Windows 短暂默认端口冲突的 PID 未定位，不声称其确切来源已证明。
- Bash/curl/grep、兼容 launcher 的 Python 仍是阶段 1a 的已知依赖。
- 本证据提交不继续改运行时代码。最终精确 head 的 CI/Pages run、jobs、
  终态计数、Linux/Windows runner 摘录会附在 PR 正文，并保存到
  `target/tool-migration-20261006-polling/evidence/ci-final.json` 和
  `ci-job-*-acceptance.txt`；这样不以 CI 回执再制造未验证的新 head。
- 未做 macOS 原生运行、后续迁移或真实发布。

## 给下一位 Agent 的备注

- 先看证据 README 与固定三个 commit；不要将旧轮次和本轮 wall 方法混算。
- 新 WSL 实验约 952 MB，检查时 WSL 可用 566.0 GB，E 可用 130.3 GB。
  旧 1.9 GiB 基线/产品 target 与 Windows 快照另外保留，无用户缓存清理。
- 原始 trace 压缩包可校验 SHA256；`recompute.py --traces <evidence-dir>`
  可从 syscall 文件核对全部分段，再从 JSON 核对统计。
- 用户仅授权当前 PR 的本轮归因/优化及交付；下一阶段仍需新指示。

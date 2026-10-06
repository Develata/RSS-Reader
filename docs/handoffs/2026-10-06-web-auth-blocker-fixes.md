# web-auth 清理等待与 Windows 路径阻塞修复

- 日期：2026-10-06
- 作者 / Agent：Codex（WSL 主实现，Windows 原生补验）
- 分支：`refactor/web-auth-owned-process`
- 实现 commit：`e6915a65335701fc8cbedec2dcfdda123c6c598f`
- 证据 commit：本文件所在提交（提交前为 pending；见 Git 历史）
- PR：[#23](https://github.com/Develata/RSS-Reader/pull/23)，保持 Draft
- 相关 tag / release：N/A；不合并、不发布、不替换附件
- 状态：`draft`；本地功能通过，整体性能门禁未判通过

## 工作摘要

按用户恢复授权，仅修复 PR #23 的两个阻塞：Unix 清理总是等待完整 1.5 秒，
以及 Windows curl 无法打开中文目录下的 headers/cookie 文件。
固定等待已消除，Windows 本地合同通过；完整认证相对原迁移基线仍有可测的
额外开销。本轮停止进一步实现，只完成证据与精确 head CI 收尾，不推进其他阶段。

## 影响范围

- 模块：独立 `scripts/release-ui` 的进程清理、单份 Bash 认证断言及夹具。
- 平台：Debian WSL 主开发；Windows 原生补验快照 `E:/gitclone/RSS-Reader/target/w2`。
- 产品 Rust crate、页面 JS、发布 workflow 未修改；macOS 未本机实测。
- 主仓库 `/home/deve/gitclone/RSS-Reader`，Debian 虚拟磁盘位于 E 盘。
  fetch 后 origin/main 仍为 `f69087a`，原 PR head 为 `a5d5e57`。
- Windows 仍在 `b745a846`；22 项删除、两份 handoff 修改和 `.workbuddy/`
  保留，其他既存 worktree 未改动。已读项目 memory_summary，未读取原始会话。

## 关键变更

- Unix 发送原有终止信号后回收 leader，再以 `kill(-pgid, 0)` 探测完整所属
  进程组；仅 ESRCH 提前返回。存在成员或无法确认时，继续 1.5 秒宽限及强制组清理。
  Windows Job Object 行为、Chrome 原有 5 秒 grace 未变。
- 将已锁定的传递依赖 libc 声明为 Unix 直接依赖；无新增包或依赖版本升级。
  process-wrap 的 signal API 不接受信号零，leader 状态也不能代替组存活探测。
- curl 只接收 ASCII 文件名，在用户指定的 Unicode 日志目录内运行；切换 cwd 前
  解析 Bash 调用者 PATH 中的 curl。grep 读取原日志目录，断言保持单份。
- 阶段名、顺序、退出码、summary schema、30 次 readiness（connect 2 秒/max 10 秒、
  失败 sleep 1 秒）、profile 与 --skip-build 语义保持。
  不跟随 redirect，MSYS next=/feeds 参数保护保持。
- 新增快速清理单元、leader 退出但孙进程忽略信号的回归，以及仅拒绝非 ASCII
  文件名参数、其余请求委托真实 curl 的夹具。验证真实 cookie、表单和 redirect。
  Git Bash PATH 夹具沿用既有 BASH_ENV 初始化方式，避免启动器重排 PATH 后绕开被测工具。

## 验证与验收

### 自动化验证

- `cargo fmt --manifest-path scripts/release-ui/Cargo.toml --check`：通过。
- `cargo clippy --manifest-path scripts/release-ui/Cargo.toml --all-targets --locked --offline --target-dir target/tool-migration-20261006/dev-build -- -D warnings`：通过。
- 同 manifest 的 `cargo test --locked --offline`：WSL 4 项单元通过，Windows 3 项通过（Unix 清理单元不编译）。
- `python3 scripts/release-ui/tests/acceptance.py --bash /bin/bash --binary target/tool-migration-20261006/dev-build/debug/release-ui -v`：
  WSL 全部 28 项通过，测试时间 76.971 秒，驱动墙钟 78.184 秒。
  这次完整执行时 PATH 新用例使用旧名称；之后只改名称与 Windows 分支初始化，
  Linux 功能路径未再变化，最终源码仍由 CI 整套复核。
- Windows `target/w2` 原生 clippy、unit、build 通过；同 acceptance.py（显式 Git Bash）
  全部执行：26 pass、2 Unix-only skip，驱动墙钟 140.501 秒。
- 覆盖原阶段调度与退出码、占用端口不杀原进程、服务早退、慢/卡住 readiness、
  >30 秒 readiness、SIGINT/SIGTERM、Windows console/强杀、孙进程、无人读取 stdout、
  中文/空格路径、真实 login/cookie/next/session-probe/logout。
- 新的顽固孙进程测试确认 leader 已退出时后代仍活着，runner 保留完整宽限，
  然后强制清理，端口关闭。快速清理单元同时确认 leader 已回收、stop 可重复调用。

### Windows 故障复现边界

- 旧 CI 的真实 curl 在中文 headers 路径返回 23；其旧 head 日志仍在 2026-10-05 证据中。
- 本机 System curl 8.21.0 宣告 Unicode；Git curl 8.22.0 不宣告 Unicode，
  但两者本机真实正常路径都能通过旧合同。因此不能声称已复现相同 CI curl build/代码页。
- 文件名限制夹具在真实 HTTP 链路中使旧 adapter 确定失败 23，新 adapter 通过，
  headers/cookie 仍留在中文目录。没有改全局编码、代理、凭据或安全设置。
- 开发中测试解释器和 Git Bash PATH 初始化问题已修正；中间失败日志保留在 ignored target。
  最终证据为 `windows-old-guard-final.txt`、`windows-new-guard-final.txt`、
  `windows-native-validation.json`，不以绕开中文路径、删测试来取得通过。
- 真实 Windows CI 的修复验收以本轮最终精确 head 的 job 终态为准。

### 真实服务与兼容入口

- 本轮重新独立构建共同的真实 rssr-web，A/B 42 次认证执行（含暖机）全部成功，
  每次结束检查该服务 binary 无活进程、监听端口关闭。
- 三参数兼容入口在中文日志目录再次通过，退出 0、端口关闭、服务无活进程；
  墙钟 3.679 秒（含 wrapper 本次构建检查），不混入 runtime 样本。
- 复用同一个已有静态 bundle，后续 browser stage 使用明确 exit-0 stub。
  这是 HTTP/宿主验证，不是新鲜 Dioxus bundle 或完整 UI 浏览器验收。

### CI 收尾

本证据提交后推送同一 Draft PR，持续跟踪**最终精确 head** 的全部 checks 到终态。
最终 head SHA、run/job 链接与终态写入 PR 描述及本机
`target/tool-migration-20261006/evidence/ci-final.json`。
本文件提交时 CI 尚待运行，不用修复前 head 的绿项代替修复后的验收。

## 性能复测

[原始样本、构建成本、脚本与平台证据](../testing/evidence/2026-10-06-web-auth-blocker-fixes/)。

- A：`f69087a61a4627cbd3ba88fa082d7722df811dc5`；
  B：`e6915a65335701fc8cbedec2dcfdda123c6c598f`。
  从两个 commit 各自导出不可变源码；WSL Rust/Cargo 1.97.0，同一工具链和输入。
- 六类各 20 对、AB/BA 平衡交错，独立暖机；240 个实测命令与 12 次暖机全部成功。
  原始 JSON 的另外 80 行是从同一次执行提取的内部清理/阶段时长，不是追加独立样本。
- 工具 clean build A 10.438 秒、B 12.011 秒，各 n=1；wrapper 首次构建
  A 9.151 秒、B 9.455 秒，各 n=1。共同 rssr-web clean build 33.152 秒，单独列示。
  全部构建命令、探针构建和开发态验证成本已保留；不据单次编译宣称稳定收益或退化。
- Debug binary A 21,311,880 字节、B 21,450,240 字节，增加 138,360 字节（约 0.65%）。
- 清理探针编译各自冻结的 process.rs，同样启动 /bin/sleep 30，
  测量 stop 内部时间并检查 leader 回收。该探针不是生产新入口。
- 服务 no-op 使用同一个产品 target/源码，A/B 标签仅表示交错顺序，不能解释为产品构建收益。

| WSL 测量（各 n=20） | A median ms | B median ms | 配对 B-A median ms |
| --- | ---: | ---: | ---: |
| 直接 binary --plan | 3.679 | 3.860 | +0.278 |
| 工具 no-op build | 114.764 | 114.864 | +0.003 |
| 已暖机 wrapper --plan | 114.612 | 114.818 | +0.148 |
| 共同服务 no-op build | 315.310 | 315.478 | +0.236 |
| stop 内部清理 | 1510.197 | 25.176 | -1485.033 |
| auth 调用墙钟（含后续 stub） | 1670.790 | 1770.821 | +100.170 |
| runner 内部 web-auth 阶段 | 1612.0 | 1677.5 | +98.5 |

内部清理 A IQR 0.697 / MAD 0.359 / min 1507.886 / max 1511.227 ms；
B IQR 0.070 / MAD 0.034 / min 25.106 / max 25.314 ms。
web-auth 阶段 A IQR 40 / MAD 4 / min 1569 / max 1656 ms；
B IQR 91 / MAD 43.5 / min 1631 / max 1833 ms；
配对差 IQR 66.75 / MAD 42 / min +16 / max +218 ms。
完整统计见 paired-summary.json。

Python 有界 wait 的墙钟观察带有最多约 50 ms 轮询粒度，因此小的墙钟差值不作性能
结论；内部时长另列。readiness 包含在完整阶段中，未独立测量。
不把 20 对样本当作可靠 p99；未测内存；Windows 未做配对性能测量。
两轮独立实验不合并样本，Windows/WSL 不混比，也不将挂载盘差异记作 Rust 收益。

## 结果

- 固定 1.5 秒等待已由直接探针与完整后代清理回归证明消除；顽固后代宽限保持。
- Windows 本地文件名回归通过，仍需最终 Windows CI 终态证据。
- 相对原基线，web-auth 独立 median 增加 65.5 ms（约 4.1%）；
  **配对差 median 为 +98.5 ms，20 对全部为正**。不能宣称迁移已无性能退化。
- 未预设或事后放宽预算，整体性能门禁保持未通过/待决策。
  本轮不继续扩大实现或进入后续阶段；只提交、推送、收集获准的 CI 收尾。

## 风险与后续事项

- 下一步需先决定是否接受约 0.1 秒配对额外开销，或单独授权定位剩余调度/启动成本。
  不把这轮数据包装成整体提速，也不自行扩展到 HTTP/状态机重构。
- 未在本机验证 macOS 信号/进程组行为；没有新增 macOS runner 或实际发布。
- HTTP 断言仍为 Bash/curl/grep，兼容 wrapper 仍需 Python；其余迁移阶段未实施。
- 清理保证限定于所属进程组/Job Object；主动逃离组的守护进程、系统断电未验证。
- 全部新增构建/日志留在 WSL E 盘虚拟磁盘与 Windows E 盘 target，
  未清用户缓存。采样后 WSL 可用约 529 GiB，E 盘物理可用约 124.7 GiB。

## 给下一位 Agent 的备注

- 入口：process.rs 的 stop、run_rssr_web_auth_assertions.sh、
  acceptance.py 的 ASCII 文件名和 stubborn leaf 用例。
- 完整原始日志在 `target/tool-migration-20261006/evidence`，
  Windows 快照在 `target/w2`；精简原始 JSON 和实跑脚本已入库。
- 复跑脚本是这轮实际使用的实验记录；需固定同一 commit 并换全新 BASE，
  不原地覆盖旧证据。任何继续实现均以用户后续决定为准。

# web-auth 退出观察延迟归因与有限轮询优化

- 日期：2026-10-06
- 作者 / Agent：Codex（Debian WSL 主开发，Windows 原生补验）
- 分支：`refactor/web-auth-owned-process`
- 修复前 HEAD：`1b710aed0276f5b07558ea94ae2698b385353a2b`
- 相关 commit：pending；本次实现冻结与最终性能证据分开提交
- PR：[#23](https://github.com/Develata/RSS-Reader/pull/23)，保持 Draft
- tag / release：N/A；不合并、不发布
- 状态：`draft`

## 工作摘要

按用户再次恢复授权，仅定位 PR #23 剩余延迟并做窄范围优化。
A 原基线 f69087a，B 当前修复版 1b710ae（运行时代码等同 e6915a）。
先做 20 组交错跟踪/未跟踪对照，再修改已实证的等待位置；不重写 HTTP 或引入框架。

## 影响范围

- 仅 scripts/release-ui/web_auth.rs、process.rs 的有限早期 sleep，以及证据文档。
- 主路径 /home/deve/gitclone/RSS-Reader；WSL 位于 E 盘。
- Windows E:/gitclone/RSS-Reader/target/w3 为独立验证快照。
- Windows 用户 dirty 和三个既存 dirty worktree 原样保留；未发现活跃仓库构建。
- 无产品逻辑、页面 JS、metadata、测试服务器或 Chrome host 迁移。

## 关键变更

- readiness 请求前几个等待间隔为 5/10/20 ms，随后回到 40 ms。
- Unix 清理前两个等待间隔为 5/10 ms，随后回到 25 ms。
- 均有真实 sleep，早期额外探测次数有限；不改变 1.5 秒顽固后代宽限。
- 仍先 reap leader，且仅 kill(-pgid,0) 得到 ESRCH 才提前结束。
  组存在或 EPERM 等不确定结果继续宽限和强制清理；没有降级到只看 leader。
- adapter 通用等待、1 秒 retry、Console.finish、Windows Job Object、
  单份 Unicode 路径 adapter 与 MSYS next=/feeds 均未修改；没有新增依赖。

## 验证与验收

- WSL fmt/clippy/4 单元与全部 28 黑盒通过（73.153 秒）。
- Windows 原生 fmt/clippy/3 单元通过。首次与 WSL 并行回归时默认 18081 发生
  端口冲突（8 个失败，均保留）；两侧完成后查不到占用者且可绑定。
  不改测试、不杀进程，独立原样重跑 28 项：26 pass、2 Unix-only skip，
  119.707 秒。未定位短暂占用 PID，不把并发冲突断言为候选回归。
- 固定候选的最终性能复测尚未开始。
- 包含顽固孙进程、SIGINT/SIGTERM、卡住请求、>30 秒 readiness、
  stdout 无读者、中文/空格路径、caller PATH 与真实认证合同。
- macOS 无原生验证，不以 WSL 或 Windows 替代。

## 初始归因证据

- 独立目录 target/tool-migration-20261006-polling/evidence。
- 同一产品 binary/bundle、Rust/Cargo 1.97.0；strace 6.13 仅跟踪自有子进程。
- 20 组诊断：A/B 各 20 跟踪、20 未跟踪。跟踪样本 K 均为 2，
  每次另有 7 个认证 curl 和 10 个 grep；不把 K 当作此前未记录实验的已知值。
- B readiness 退出观察延迟总和 median 68.613 ms；服务 stop median 26.555 ms。
  A 服务 stop median 2.032 ms，不能从 A 的认证总时间扣除独立 1.5 秒清理探针。
- adapter/外层 Bash 退出观察 median：B 23.493 ms、A 16.974 ms。
- 跟踪减未跟踪 stage median 配对差：A +90.5 ms、B +49.5 ms，
  因此跟踪仅用于分段诊断，不能替代未跟踪收益测量。
- 最初独立诊断 A 出现 K=3、cargo 到产品 exec 约 1.13 秒，单独原样保留，
  不混入后续预定 20 组，也不删除或当成失败样本。

## 结果

尚待冻结候选后的未跟踪收益/CPU 验收与最终 CI，不预先承诺零退化。

## 风险与后续事项

- 外部 CPU 为 wait4 的进程及已回收后代资源统计；最大 RSS 不是整棵并发树的峰值。
- 最终未跟踪实验还读取退出但未回收进程的 schedstat，
  单列监督器主线程 CPU；不代表 console 线程或整机 CPU。
- 后续阶段保持停止；若没有可测收益，将撤回无效优化。

## 给下一位 Agent 的备注

- 所有诊断与构建在 WSL E 盘或 Windows E 盘 target；不清用户缓存。
- 分段脚本/原始记录与最终固定 commit 及 CI 结论在证据提交中补齐。

# web-auth 清理等待与 Windows 路径阻塞修复

- 日期：2026-10-06
- 作者 / Agent：Codex（WSL 主实现，Windows 原生补验）
- 分支：`refactor/web-auth-owned-process`
- 当前 HEAD：`a5d5e57f8bc2df5b2941e67e0e923f1b6946d166`（修复前）
- 相关 commit：pending；实现冻结与测量证据将分开提交
- 相关 tag / release：N/A；不合并、不发布
- 状态：`draft`

## 工作摘要

按用户恢复授权，仅修复 Draft PR #23 的两个阻塞：Unix 清理总是等待完整
1.5 秒，以及 Windows curl 无法打开中文目录下的 headers/cookie 文件。
不推进其他迁移阶段。

## 影响范围

- 模块：独立 `scripts/release-ui` 的进程清理、单份 Bash 认证断言及夹具。
- 平台：Debian WSL 主开发；Windows 原生补验快照位于 `target/w2`。
- 产品 Rust crate、页面 JS、发布 workflow 未修改。

## 关键变更

- Unix 在发送原有终止信号后回收 leader，再以 `kill(-pgid, 0)` 探测整个
  所属进程组；只有 ESRCH 才提前返回。存在成员或探测不确定时仍保留
  1.5 秒宽限及强制组清理；Windows Job Object 行为不变。
- 将已锁定的传递依赖 libc 声明为 Unix 直接依赖，无新增包或版本升级。
  process-wrap 的 signal API 不接受信号零，不能用其 leader 状态替代组探测。
- curl 只接收 ASCII 文件名，在请求的 Unicode 日志目录中运行；切换 cwd 前
  解析 Bash 调用者 PATH 中的 curl。grep 仍读取原日志路径，认证断言保持单份。
- 增加无用等待回归、leader 先退出且孙进程忽略信号的清理回归、限制文件名
  编码并委托真实 curl 的 HTTP 回归。Git Bash PATH 测试沿用现有初始化办法。

## 验证与验收

- WSL fmt、clippy、4 项单元通过；完整 28 项黑盒通过（76.971 秒）。
  完整执行时新 PATH 用例仍使用旧名称，后续仅调整名称和 Windows 分支初始化。
- Windows 原生 clippy、单元、完整黑盒通过：28 项中 26 pass、2 Unix-only skip；
  驱动墙钟 140.501 秒。受控 ASCII 文件名夹具使旧断言失败 curl 23，新断言通过。
- 固定 A/B 性能复测尚未开始；最终精确 head CI 尚未启动。
- 原始执行记录：`target/tool-migration-20261006/evidence/`；
  Windows：`E:/gitclone/RSS-Reader/target/w2/`。
- 本地本轮未运行 macOS、Android、浏览器 UI；后续 CI 结果逐 head 记录。

## 结果

尚未完成性能与最终 CI 验收，保持 Draft；不宣称阻塞已解除。

## 风险与后续事项

- 不缩短顽固后代的宽限，不以 leader 退出作为整组退出的证据。
- 保留旧测试/测量失败日志，不以修改全局编码或放宽测试处理 Windows 故障。
- 仅复测本阶段；HTTP Rust 化、服务迁移、metadata 和 Chrome ownership 均待后续指令。

## 给下一位 Agent 的备注

- 主路径 `/home/deve/gitclone/RSS-Reader`；WSL Debian 虚拟磁盘在 E 盘。
- Windows 原 checkout 的用户 dirty、其他既存 worktree 均保留。
- A 基线仍为 `f69087a61a4627cbd3ba88fa082d7722df811dc5`；修复前
  `a5d5e57` 的运行时代码等同 `f0bf997`。不把两轮测量混为同一实验。

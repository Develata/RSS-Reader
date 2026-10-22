# C6 Web 服务发布包、镜像与迁移

- 日期：2026-10-22
- 作者 / Agent：Codex
- 分支：`v0.2-architecture`
- 当前 HEAD（实现前）：`a13a1e5`
- 相关 commit：commit: pending
- 相关 tag / release：N/A
- 状态：`draft`（实现与本地检查已进行，远端容器/平台验证待运行）

## 工作摘要

用户授权当前 agent 逐项复核，通过后进入 C6；[C4c 闭环](./2026-10-22-c4c-round3-closure.md) 已完成。按阶段 C 已确认的范围实现四平台“服务＋页面”包、统一构建标识、镜像复用同一 Linux artifact、容器内 CLI、卷迁移验收与部署说明。没有新增产品后端、迁移用户资料库或改变认证协议。

## 影响范围

- 构建：`web-packages.yml`、Release / Docker workflow、Dockerfile / Compose。
- 工具：Python 标准库打包/校验、真实程序/API/容器 smoke、CDP 真实新旧页面检查。
- 平台：Linux x86_64、Windows x86_64、macOS x86_64 / aarch64。
- 文档：部署、测试说明与本交接。主检出、detached 验证检出和实际用户卷未修改。

## 关键变更

- 每个服务包包含程序、完整 `public/`、构建标识及 SHA-256 清单；移除 v0.2 只发布页面的流程，Pages 演示独立。
- 从固定 source SHA 得到 `git-<sha>`，全部服务/页面使用同一标识。Linux 基线为 Debian bookworm；macOS 在对应架构 runner 实际执行服务包。
- Docker 消费同一次 workflow 的 Linux 包与 CLI；检查后推送受测镜像，不再第二次编译或重建镜像。普通分支/手动 Docker 验证不发布；正式 tag / Release 流程才上传镜像和 Release。
- `/data` 保持运行用户所有；CLI 与服务共用数据；旧认证卷在迁移成功前保持挂载。Compose 要求明确公开 origin，源码构建要求明确 build id。
- 迁移测试只生成自己的旧格式凭据/会话及随机命名卷，验证只读迁移、旧会话失效与移除旧卷后的持久凭据。网络专用 NAT64 出口前提补入部署文档。

## 验证与验收

- 打包工具 4 组单元测试通过（ZIP/tar、哈希篡改、未知/不匹配标识、拒绝覆盖、目录穿越/链接）；测试使用合成文件，不冒充产品验收。
- Windows 真实 debug 包已打包、解压、核对 12 个文件，并通过新库启动、登录、页面、匹配版本写入和旧版本拒绝。初始包标识为 `c4c-round3-e8853b6`，来自本轮 C4c 新构建。
- Compose 合并配置通过；不提供公开 origin 时拒绝。
- Python / JS 语法与四个 workflow 的 actionlint 通过。
- Windows 真实 Chrome 新旧页面检查通过：新包 `c6-package-a13a1e5` 的页面发送自身编译标识并保存成功；旧包 `c4c-round3-e8853b6` 的实际页面连接新服务时，发送旧标识、收到 409、显示可见的重新加载提示，且未修改资料库。证据 `real-pages/result.json` 与两个 `*-browser.json`。
- 四平台远端 release 包和容器卷验证：待补。本地为 debug 包，不能代替远端 release artifact。
- 本机启动 Docker Desktop 被自动审批拦截：提示启动应用需批准，当前策略不允许申请批准。未改用其他方式绕过；容器验收安排在不发布镜像的远端分支工作流。
- 本地证据：`target/handoff/evidence-c6/`。

## 结果

实现已准备，最终远端验收与提交号待补。没有合并、打 tag、创建 Release 或发布镜像。

## 风险与后续事项

- C6 完成情况以实际报告为准；跨平台构建不能代替 GUI 实机验收。
- 外部独立评审、真实公网 HTTPS / NAT64 出口、实际用户卷迁移仍是后续边界。
- Linux/macOS runner 选择核对了 [GitHub 官方镜像列表](https://github.com/actions/runner-images/blob/main/README.md)，未用 ARM 上的编译成功冒充 Intel 实际运行。

## 给下一位 Agent 的备注

旧 `28859c2` 复审包不包含本轮修复或 C6。后续引用准确 source SHA、workflow run、artifact 哈希及平台；不要把本地 debug 包称为最终 release artifact。

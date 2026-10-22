# C6 Web 服务发布包、镜像与迁移

- 日期：2026-10-22
- 作者 / Agent：Codex
- 分支：`v0.2-architecture`
- 当前 HEAD（实现前）：`a13a1e5`
- 相关 commit：`129996a`（实现）、`e785ce7`（公共页面）、`d8f15d5`（macOS 真实路径）、`23e5087`（缓存隔离）、`ac974b9`（C4c 失败诊断与远端记录）；最终结果及 Release artifact 排除规则随本记录提交。
- 相关 tag / release：N/A
- 状态：`validated`（实现与本地/远端验收完成；未发布，外部独立评审另行安排）

## 工作摘要

用户授权当前 agent 逐项复核，通过后进入 C6；[C4c 闭环](./2026-10-22-c4c-round3-closure.md) 已完成。按阶段 C 已确认的范围实现四平台“服务＋页面”包、统一构建标识、镜像复用同一 Linux artifact、容器内 CLI、卷迁移验收与部署说明。没有新增产品后端、迁移用户资料库或改变认证协议。

## 影响范围

- 构建：`web-packages.yml`、Release / Docker workflow、Dockerfile / Compose。
- 工具：Python 标准库打包/校验、真实程序/API/容器 smoke、CDP 真实新旧页面检查。
- 平台：Linux x86_64、Windows x86_64、macOS x86_64 / aarch64。
- 文档：部署、测试说明与本交接。主检出、detached 验证检出和实际用户卷未修改。

## 关键变更

- 每个服务包包含程序、完整 `public/`、构建标识及 SHA-256 清单；移除 v0.2 只发布页面的流程，Pages 演示独立。
- 从固定 source SHA 得到 `git-<sha>`，全部服务/页面使用同一标识；公共页面只构建一次。Linux 服务/CLI 基线为 Debian bookworm；macOS 在对应架构 runner 实际执行服务包。
- Docker 消费同一次 workflow 的 Linux 包与 CLI；检查后推送受测镜像，不再第二次编译或重建镜像。普通分支/手动 Docker 验证不发布；正式 tag / Release 流程才上传镜像和 Release。
- Release 下载产物时排除 `*.dockerbuild`，遵循 [Docker action 的 build record 要求](https://github.com/docker/build-push-action/tree/v7#summaries)，避免新增镜像 job 的记录阻断 Release 下载。该排除规则经 actionlint 检查；没有触发正式 Release 验证。
- `/data` 保持运行用户所有；CLI 与服务共用数据；旧认证卷在迁移成功前保持挂载。Compose 要求明确公开 origin，源码构建要求明确 build id。
- 迁移测试只生成自己的旧格式凭据/会话及随机命名卷，验证只读迁移、旧会话失效与移除旧卷后的持久凭据。网络专用 NAT64 出口前提补入部署文档。

## 验证与验收

- 打包工具 4 组单元测试通过（ZIP/tar、哈希篡改、未知/不匹配标识、拒绝覆盖、目录穿越/链接）；测试使用合成文件，不冒充产品验收。
- Windows 真实 debug 包已打包、解压、核对 12 个文件，并通过新库启动、登录、页面、匹配版本写入和旧版本拒绝。初始包标识为 `c4c-round3-e8853b6`，来自本轮 C4c 新构建。
- Compose 合并配置通过；不提供公开 origin 时拒绝。
- Python / JS 语法与四个 workflow 的 actionlint 通过。
- 文档本地链接检查：6 份变更文档、75 个链接，0 错误。
- Windows 真实 Chrome 新旧页面检查通过：新包 `c6-package-a13a1e5` 的页面发送自身编译标识并保存成功；旧包 `c4c-round3-e8853b6` 的实际页面连接新服务时，发送旧标识、收到 409、显示可见的重新加载提示，且未修改资料库。证据 `real-pages/result.json` 与两个 `*-browser.json`。
- 四平台远端 release profile 包与容器卷验收均通过，见下表。本地 debug 新旧页面检查与远端 release profile 包分别记录，没有把本地包当成远端 artifact。
- 首次 Docker 手动运行 `39816937457`：Linux 在安装工具阶段失败，官方 Dioxus 0.7.9 二进制需要 `GLIBC_2.39`，bookworm 不满足；未到产品编译。修复为在宿主 runner 构建一份公共页面，四个平台下载相同页面；Linux 服务/CLI 仍在 bookworm 编译，统一 source SHA / build id 不变。
- 同次 macOS ARM 的服务/页面编译与包校验完成，但启动临时新库时 SQLite 返回 `SQLITE_CANTOPEN_SYMLINK`（1550）：Python 临时目录为 `/var/folders/...`，macOS 的 `/var` 是系统符号链接。测试改为对自己创建的临时目录 `resolve()` 后再传入；保持产品的 `SQLITE_OPEN_NOFOLLOW` 与属主检查。部署文档补充真实路径说明。
- run `39818454019` 的 Linux / Windows 命中 Cargo 缓存后，恢复了旧 `target/web-verify`，解压器按约定拒绝覆盖已有目录。将页面输出、打包、解压与报告移到 runner 临时目录，保留拒绝覆盖检查，不让验证目录参与 Cargo 缓存。
- 本机启动 Docker Desktop 被自动审批拦截：提示启动应用需批准，当前策略不允许申请批准。未改用其他方式绕过；容器验收安排在不发布镜像的远端分支工作流。
- 本地证据：`target/handoff/evidence-c6/`。

本地主要命令（Python 为本机 Scoop Miniconda 的 Python 3，Node 24）：

```text
python scripts/test_package_web.py
python -m py_compile scripts/package_web.py scripts/smoke_web_package.py scripts/smoke_web_container.py
node --check scripts/browser/web_package_smoke.mjs
actionlint .github/workflows/ci.yml .github/workflows/docker.yml .github/workflows/release.yml .github/workflows/web-packages.yml
python scripts/smoke_web_package.py target/handoff/evidence-c6/windows-current --logs target/handoff/evidence-c6/real-pages --chrome "C:/Program Files/Google/Chrome/Application/chrome.exe" --old-package target/handoff/evidence-c6/windows-package
```

远端验证入口：

- `gh workflow run ci.yml --repo Develata/RSS-Reader --ref v0.2-architecture -f acceptance_long=true`：[run 39816994053](https://github.com/Develata/RSS-Reader/actions/runs/39816994053)，source `129996af85798c07c623255be88e18a5706426d1`，最终 **24/24 jobs 成功**。首次三个长验收场景在网络收尾判定超时，本机定向回放通过，仅重跑失败 job 后完整 **27/27 场景**通过；根因未确定，见 C4c 交接，不宣称稳定性缺口已修复。
- `gh workflow run docker.yml --repo Develata/RSS-Reader --ref v0.2-architecture -f all_platforms=true`：最终 [run 39819315734](https://github.com/Develata/RSS-Reader/actions/runs/39819315734)，source `23e5087451879ed934d216d05506d454450d3765`，**7/7 jobs 成功**。统一标识 `git-23e5087451879ed934d216d05506d454450d3765`。此入口不推送 GHCR，不创建 Release；日志中登录、提取发布标签与 push 三步均 skipped。

### 四平台与镜像证据

每个平台实际解压、校验 4 个产品文件（程序、index、JS、WASM；另有清单），启动新库、登录、获取页面、匹配构建写入和旧标识拒绝均通过。下列 SHA-256 为 runner 对内部发布包算出的哈希，不是 GitHub 外层 artifact ZIP 的哈希。

| 包 | SHA-256 | 验收 job |
| --- | --- | --- |
| `rssr-web-linux-x86_64.tar.gz` | `f8deed4adfa01fca443b7a666914bc3532b4f1ecd1e03f99d61a962b17a45811` | `119966766768` |
| `rssr-web-windows-x86_64.zip` | `94738ec65706cc43254955643f199c4bf7f7a0b609b3c9294c7d97d47186a1f2` | `119966767358` |
| `rssr-web-macos-x86_64.tar.gz` | `28f57cbca2378c9c6f9bea92439182869e5e71c1b2c76460d1ae3a15894adb81` | `119966766973` |
| `rssr-web-macos-aarch64.tar.gz` | `efec0e34404cae5fd78bff16b9d58cf064fea0ccc7065192ce82972f38d72b94` | `119966766975` |

容器 job `119970113743` 的 **9 项检查通过**：全新数据卷与匹配写入、旧构建拒绝、镜像程序与受测 Linux 包哈希相同、运行用户 CLI 共享库、root/无关 UID 被拒绝、在线 CLI 备份、正常重启保留会话与 CLI 写入、只读旧认证卷迁移及旧 cookie 拒绝、移除旧卷后凭据仍有效。所有生成的容器/卷清理完成后才记录 pass。

Buildx 报告受测镜像 digest `sha256:76ed955320e834297280f14b36de576004b35b18cf18e6f125f3d30d557685e9`；这是 runner 本地构建，未推送注册表。容器证据 artifact `c6-container-check` ID `12444651445`，GitHub 报告 ZIP SHA-256 `5e43d813143ce4d4b60736e300f0eb9c8b63bbb6f4c870dd651e2c302658f90e`。公共页面 artifact ID `12443911363`，四个平台下载同一份页面。

## 结果

C6 的实现与分支验收完成，已推送 `v0.2-architecture`。最终结果记录提交不会重新编译上述固定快照产物。没有合并、打 tag、创建 Release 或发布镜像。

## 风险与后续事项

- 跨平台服务启动验收不能代替 GUI 实机验收；正式发布和 GHCR 推送路径未执行。
- 本轮镜像实测为 `packaged` target；本地源码 Docker `source-built` target 仅完成接线检查，未重新执行完整源码构建。
- 外部独立评审、真实公网 HTTPS / NAT64 出口、实际用户卷迁移仍是后续边界。
- 常规 CI 首次网络收尾超时已继续调查，复现并修复驱动跨文档等待缺陷；[后续交接](2026-10-22-ci-acceptance-timeouts.md)记录 BFCache 时序、修复验证及原始三次缺请求身份的归因边界。首次失败证据保留。
- Linux/macOS runner 选择核对了 [GitHub 官方镜像列表](https://github.com/actions/runner-images/blob/main/README.md)，未用 ARM 上的编译成功冒充 Intel 实际运行。

## 给下一位 Agent 的备注

旧 `28859c2` 复审包不包含本轮修复或 C6。后续引用准确 source SHA、workflow run、artifact 哈希及平台；不要把本地 debug 包称为最终 release artifact。

## 2026-10-22 正式发版条件核对

- 核对对象：本地与远端 `v0.2-architecture` 均为 `b03d4094dfdf429c9a640d8c0cc54c934e60a86f`，工作树干净。本节是同日交接追加，随本记录提交；未更改产品或 workflow。
- GitHub API 再次确认 [CI 39830409992](https://github.com/Develata/RSS-Reader/actions/runs/39830409992) 首次运行成功，24/24 jobs 成功，源码为 `9e2c8ecb660a82b5ca7507c4e47eba4409330a8b`；该源码到 `b03d409` 只有交接与验证证据文档变化。
- GitHub Releases 与 tag refs 查询：最新公开版为 `v0.1.23`，尚无 `v0.2*` tag。没有创建 tag、Release 或推送镜像，也没有为本次文档核对重新构建产物。
- [已确认架构](../design/v0.2-architecture.md) 要求完成阶段 A–D 后统一发布 `v0.2.0`。阶段 D 尚未完成：`crates/rssr-app/Cargo.toml` 的 `pages-demo` 仍启用 `rssr-infra/browser-store`，三个旧 wasm harness 仍以该 feature 为前提。演示切换到内存 SQLite、迁移语义测试并删除旧存储/harness，以及实际 Pages 子路径、下载量/启动时间/内存/主线程阻塞与禁网验收，均不能由 C6 服务包验收代替。
- [阶段 C 计划](../design/v0.2-phase-c-plan.md) 仍保留最终独立评审门禁。用户已授权的 C4c 逐项复核完成，不代表 C6 后的阶段 C 最终独立评审已完成。
- `.github/workflows/release.yml` 明确以 tag 注入发布版本；workspace 的 `0.1.0` 是现有约定，不单独列为发版阻挡。
- 结论：C6 实现与分支验收完成；整个 `v0.2.0` 尚不满足原定发布条件。下一步完成阶段 D、收齐阶段评审与发布前验收证据，再固定最终发布提交。旧版本已接受的验收缺项不能自动视为本版本的豁免。

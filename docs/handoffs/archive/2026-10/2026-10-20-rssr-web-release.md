> 历史分支记录（main，来源 `d9ebe35`）。保留当时的实现和验证上下文；当前 v0.2 的适配与状态见[整合记录](./2026-10-22-branch-integration.md)。下文的旧路径、架构及待办不构成当前契约。

> 历史交接：本记录仅说明当时的实现与验收，旧路径和命令可能已退役。当前状态见[文档索引](../../../README.md)，归档范围见[历史索引](../README.md)。

# P3：rssr-web 的 Linux 二进制发布与文档修订

- 日期：2026-10-20
- 作者 / Agent：Claude Code
- 分支：feat/rssr-web-remote-ui（P3 与 P2 在同一分支上连续提交）
- 当前 HEAD：见 `git log feat/rssr-web-remote-ui`
- 相关 commit：本记录同次提交
- 相关 tag / release：N/A（未触发发布）
- 状态：`draft`。发布流水线的改动要等真正打 tag 时才能在 CI 上验证；Docker 镜像的结果见文末。

## 工作摘要

按 `docs/design/rssr-web-server-mode.md` 的 P3：发布流水线新增 `rssr-web-linux-x86_64.tar.gz`
（服务端二进制 + 同版本前端包 + systemd 单元示例），补上 Linux 二进制部署文档，并修订 `specs/001`
里"无服务端依赖""Web 只用 localStorage"的条文。

## 影响范围

- `.github/workflows/release.yml`：`build-web` job 在打完前端包后编译 `rssr-web`、打包并实际启动它做冒烟
  （健康检查、`/api/v1/capabilities`、文章库创建、文章库目录权限为 700）
- `docs/deployment/rssr-web.service`（新增）、`docs/deployment/web.md`（Linux 二进制部署一节）
- `README.md`（发布附件表新增一行）、`specs/001-minimal-rss-reader/plan.md`（存储、约束、宪章检查三处，标注修订日期）

## 关键变更

- 服务端与它托管的前端包必须同版本：这是 P2 去掉"旧版服务端退回本地"兼容的前提，所以两者在同一个 job、
  用同一份源码打包。
- systemd 示例：`StateDirectory=rssr-web`、`UMask=0077`、`ProtectSystem=strict`、`ProtectHome`、
  `NoNewPrivileges`；文章库与凭据分开存放（D3）；凭据从 root 拥有、权限 0600 的 `EnvironmentFile` 读取。

## 验证与验收

### 自动化验证

- `release.yml` 用 PyYAML 解析通过。
- 发布冒烟步骤的命令序列在本机模拟过一遍：用 debug 版 `rssr-web.exe` 与 debug 前端包按同样的目录结构打包，
  再解包、启动、检查。打包结构、`/healthz`、能力探测返回 `{"protocol":1,"mode":"server"}`、文章库文件创建，
  全部通过。最后的 `stat -c '%a'` 权限检查只在 Linux 上有意义，本机未执行。
- 没有触发真正的发布流水线。

### 手工验收

- 未在 Linux 主机上按文档实际部署 systemd 单元。

## 结果

- 草案状态：代码与文档就绪；CI 与 Linux 主机部署未验证。

## 风险与后续事项

- 第一次打 tag 发布时要盯住 `build-web` job 新增的三步，尤其是冒烟（端口 18080，以及运行器上的 `curl`）。
- systemd 单元假定 HTTPS 由反向代理终止；直接对外监听需要改 `BIND` 并关闭 `SECURE_COOKIE`，不推荐。
- 分支栈未合并、未 push：design → refactor/rssr-host → feat/rssr-web-server-api → feat/rssr-web-remote-ui。

## 给下一位 Agent 的备注

- 入口：`.github/workflows/release.yml` 中 `build-web` job 的后半段。

## Docker 镜像构建（P1 阻断项的验证）

- 构建：`git archive --format=tar dbc8e73 | docker build --progress=plain -t rssr-web:p1-review -`
  （从提交树构建，避开本机 `.claude/worktrees` 混入构建上下文；`.dockerignore` 已在 a067b54 补上）。
  基础镜像 `rust:1.99-bookworm` 解析并拉取成功；第 6 步 `dx bundle` 用时 899 秒，
  第 7 步 `cargo build -p rssr-web --release` 用时 154 秒，构建 exit 0。在 1.88 上无法编译的原生依赖
  （`File::try_lock`）现在能编译通过。
- 运行：`docker run -p 127.0.0.1:18081:8080`，传入临时用户名、密码与 `RSS_READER_WEB_AUTH_STATE_FILE=/app/auth/auth.json`。
  - `/healthz` 返回 `ok`；`/api/v1/capabilities` 返回 `{"protocol":1,"mode":"server"}`。
  - 容器内 `id` 为 `uid=999(rssr)`，`/app/data` 权限 `drwx------`，SQLite 双库已创建。
  - 日志：`opening rssr-web library data_dir=/app/data outbound=PublicOnly`。
  - 容器已删除；镜像 `rssr-web:p1-review` 保留在本机，可随时删除。
- 注意：在 Git Bash 里 `docker run -e …=/app/…` 需要 `MSYS_NO_PATHCONV=1`，否则路径会被改写成 Windows 路径，
  容器启动失败。这是本机 shell 的问题，与镜像无关。
- 镜像对应 dbc8e73 的源码，不含 P2 的改动。P2 只改了浏览器端与 `rssr-host::client`，服务端镜像的构建链路没有变化。

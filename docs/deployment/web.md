# Web / Docker 部署

本文说明 v0.2 开发分支的服务器模式，不代表这套部署已发布。正式 Web 页面由匹配构建的 `rssr-web` 托管，经同源 `/api/v1` 使用服务器上的 `LocalBackend`。单独托管正式 Web 静态文件不能提供可用的阅读器；仅 [Pages 演示](./github-pages.md) 可独立静态托管。

服务器的数据目录（容器内 `/data`）保存 `library.sqlite3`（订阅、文章、正文、已读/收藏与设置）、`auth.sqlite3`（凭据与会话）和 `outbound.json`（出站策略）。多个浏览器与同数据目录的 CLI 直接访问同一资料库；浏览器不保存文章副本。浏览器仍可保存设备偏好，以及重新加载或登录前的本标签页草稿（`sessionStorage`）。

订阅发现、抓取、自动刷新、WebDAV 与图片本地化由服务器完成。默认出站策略为 `public_only`，私有地址等例外通过 `outbound.json` 显式配置；规则与拒绝优先级见 [阶段 C 出站策略](../design/v0.2-phase-c-plan.md#c1-出站访问策略infra)。受限制的客户端直接连接校验后固定的 IP，不继承环境 HTTP 代理；DNS、重定向各跳与远端可达性仍需在实际部署环境核对。不要为通过测试而放宽出站规则。

`/feed-proxy`、`/session-probe` 与本地浏览器口令门禁已在 C4a（2026-10-20）退役。已登录访问旧 feed 代理路径返回 404，不再转发上游文档。阅读正文的清洗与隔离由当前阅读器实现及浏览器验收覆盖。

正式部署使用 HTTPS；本机开发可用回环 HTTP。正式 Web 与 Pages 均不依赖 Web Locks；Pages 使用独立内存 SQLite。正式 Web 不再离线可读。

## 服务与页面发布包

v0.2 发布流程按宿主平台打包同一次构建的服务和页面：

| 平台 | 包名 |
| --- | --- |
| Linux x86_64 | `rssr-web-linux-x86_64.tar.gz` |
| Windows x86_64 | `rssr-web-windows-x86_64.zip` |
| macOS Intel | `rssr-web-macos-x86_64.tar.gz` |
| macOS Apple Silicon | `rssr-web-macos-aarch64.tar.gz` |

每个包包含 `rssr-web`（Windows 为 `.exe`）、`public/` 和记录构建标识及文件 SHA-256 的 `build.json`。先解压整个包，在包目录里启动服务，不能只替换页面或只复制程序。v0.2 不再发布单独的 `RSS-Reader-web.tar.gz`；Pages 演示仍独立构建与部署。这里描述的是分支中的发布流程，实际可下载版本以 Release 为准。

Linux / macOS 本机示例（Windows 可在 PowerShell 中设置同名 `$env:` 变量后运行 `.\rssr-web.exe`）：

```bash
export RSSR_DATA_DIR="$HOME/.local/share/rss-reader-server"
export RSS_READER_WEB_STATIC_DIR="$PWD/public"
export RSS_READER_WEB_BIND=127.0.0.1:8039
export RSS_READER_WEB_PUBLIC_ORIGIN=http://localhost:8039
export RSS_READER_WEB_USERNAME=admin
export RSS_READER_WEB_PASSWORD='请替换为自己的强密码'
./rssr-web
```

数据目录使用真实路径，认证库拒绝沿符号链接打开文件。macOS 的 `/var`、`/tmp` 是系统别名；临时运行可先创建目录，再用 `cd <目录> && pwd -P` 得到真实路径后设置 `RSSR_DATA_DIR`。

[Web packages workflow](../../.github/workflows/web-packages.yml) 从一个固定 source SHA 生成统一 `RSSR_BUILD_ID=git-<完整 SHA>`；四个平台均检查解压包并实际启动服务。Linux 在 Debian bookworm 中构建，镜像直接复制这份受测包与同次构建的 CLI，不再次编译。发布流程在容器验收之后推送同一受测镜像；分支上的 Docker 手动验证不推送镜像。检查入口和实际完成情况见 [C6 验证](../testing/web-distribution.md)。

## 使用已发布镜像

仓库根目录的 [`docker-compose.yml`](../../docker-compose.yml) 直接拉取 `ghcr.io/develata/rss-reader:latest`，默认映射宿主端口 `8039` 到容器 `8080`，并挂载数据卷 `/data`。在克隆的仓库中创建仅供本机使用的 `.env`：

```dotenv
RSS_READER_WEB_USERNAME=admin
RSS_READER_WEB_PASSWORD=请改成至少八位的密码
RSS_READER_WEB_PUBLIC_ORIGIN=http://localhost:8039
RSS_READER_WEB_ENV=development
RSS_READER_WEB_TRUST_PROXY_HEADERS=false
RSS_READER_PORT=8039
```

```bash
docker compose up -d
```

然后访问 `http://localhost:8039/login`。

- `RSS_READER_WEB_PUBLIC_ORIGIN` 是浏览器地址栏里的“协议://主机[:端口]”，必须与实际访问的地址完全一致（改了 `RSS_READER_PORT` 要一起改）。服务只接受来自这个地址的登录、退出等提交，其他来源一律返回 403。容器监听 `0.0.0.0`，没有这个设置无法启动。
- 首次启动时，`rssr-web` 在 `/data` 里建立认证库 `auth.sqlite3`，所有者凭据按顺序取第一个可用的：`RSS_READER_WEB_PASSWORD_HASH`、`RSS_READER_WEB_PASSWORD`（转换成 Argon2 哈希）、旧版认证状态文件（见下文“从 v0.1 升级”）。建立之后以认证库为准，这些变量不再读取，可以从 `.env` 移除明文密码。不要把含密码的 `.env` 提交到版本库。
- 改密码用 `rssr-web passwd`，从标准输入读入新密码，同时吊销全部会话：

```bash
printf '%s\n' '新的强密码' | docker compose exec -T rss-reader rssr-web passwd
```

`latest` 与当前开发分支不一定相同；本文未核验远端镜像是否已包含 v0.2，需要固定版本时设置 `RSS_READER_IMAGE=ghcr.io/develata/rss-reader:<tag>`。下载包及镜像的实际版本以对应 [Release](https://github.com/Develata/RSS-Reader/releases) 为准。

## 会话与退出登录

- 登录后浏览器会一直记住登录（cookie 有效期 180 天），使用时自动续期；30 天没有使用，或者从登录起满 180 天，需要重新登录。会话保存在服务器的认证库里，重启服务后仍然有效。
- 退出登录：打开 `/logout`，在确认页点“退出登录”。这会吊销这个浏览器的会话；复制出去的 cookie 也随之失效。
- 改密码（`rssr-web passwd`）吊销全部会话；`rssr-web sessions revoke-all` 只吊销会话、保留密码，例如怀疑 cookie 泄露或恢复备份之后。
- 服务器时钟要准确（NTP 同步）：时钟回拨超过 5 分钟时，相关会话会被作废，需要重新登录。
- 认证库写入失败（例如磁盘满）时偏于拒绝：需要记录使用时间却写不进时，这次请求返回 503；已判定失效的会话即使删除失败也不再被接受。
- 正常停止（`docker stop`、`docker compose restart`、systemd、Ctrl-C）且收尾成功（没有删不掉的失效会话，并在时限内完成）后再启动，会话保留。服务异常退出（崩溃、断电、被强制结束）后再启动，所有浏览器需要重新登录：服务器无法确认上次运行里失效的会话都已删除，宁可全部作废。数据目录里的 `auth.sqlite3-running` 就是这个运行标记，不要手工删除。
- 数据目录必须是单机上的本地卷（例如 Compose 的命名卷），不要放在多台主机共享的 NFS、SMB 等网络文件系统上：服务器排他锁与 SQLite 的 WAL 在那里都不可靠。
- 同一个数据目录只能运行一个 `rssr-web` 服务器：第二个会报错退出，不影响正在运行的那个（`auth.sqlite3-server.lock` 是这把锁）。`rssr-web passwd` 等子命令可以在服务器运行时使用。
- 没有设置 `RSS_READER_WEB_PUBLIC_ORIGIN` 时（只允许开发环境），服务只能监听回环地址，并只信任与监听地址一致的 `http://127.0.0.1:<端口>` 和 `http://localhost:<端口>`。

## 正式部署

通过你控制的 HTTPS 反向代理对外提供服务，并设置：

```dotenv
RSS_READER_WEB_ENV=production
RSS_READER_WEB_PUBLIC_ORIGIN=https://reader.example.com
RSS_READER_WEB_USERNAME=请设置用户名
RSS_READER_WEB_PASSWORD_HASH=请填入Argon2哈希
RSS_READER_WEB_TRUST_PROXY_HEADERS=false
```

页面和服务器必须来自同一构建；服务器会拒绝构建标识不匹配的写请求。镜像和发布包均注入明确的构建标识；旧页面对新服务的验证结果以 [C6 记录](../handoffs/2026-10-22-c6-web-distribution.md) 为准。

`production` 要求 `RSS_READER_WEB_PUBLIC_ORIGIN` 是 https 地址，cookie 随之带 `Secure`。反向代理要原样转发浏览器的 `Origin`、`Cookie`、`X-RSSR-Build`、`X-RSSR-CSRF` 请求头和 `Set-Cookie` 响应头。页面与 `/api/v1` 使用同一 origin；不要重定向 `/api` 写请求，也不要把 API 的 JSON 错误改成登录 HTML。`RSS_READER_WEB_TRUST_PROXY_HEADERS` 仅在可信反向代理正确覆盖转发头时启用；默认保持 `false`。

登录失败按客户端分别计数，连续失败达到上限（默认 15 分钟内 5 次）后，该客户端在一段时间内（默认 15 分钟）不能再尝试：

- 默认按 TCP 对端地址区分客户端。放在反向代理后面且没有开启上面的选项时，所有访问者都显示为代理的地址，共用同一个计数。
- 开启 `RSS_READER_WEB_TRUST_PROXY_HEADERS` 后，取 `X-Forwarded-For` 的最右一项，也就是可信代理追加的对端地址；左侧各项由客户端自己填写，不会被采用。因此只在恰好一层可信代理、且它会追加该头时开启。
- 成功登录过的浏览器会收到一个只发给 `/login` 的已知设备标记（`rssr_web_device`，HttpOnly，约 400 天有效），它的失败次数单独计数。即使攻击者与你共用同一个地址并耗尽了那份计数，已经登录过的浏览器仍能用正确密码登录；没有这个标记的新浏览器则要等计数恢复。标记的签名密钥保存在认证库里，改密码不影响它；认证库丢失后已有标记失效。
- 不论用户名是否存在，失败都记在同一个客户端（或设备）上，限速不会暴露用户名是否正确。
- 同时进行的密码校验有名额上限，名额满时请求直接按限速拒绝；已登录过的浏览器另有一个名额，其他来源占满时仍能校验。
- 最多单独跟踪 4096 个客户端的失败次数；超出后新的客户端共用一份计数，攻击期间可能被一起限速，已登录过的浏览器不受影响。

密码哈希可在源码工作区生成：

```bash
cargo run --locked -p rssr-web -- --print-password-hash '请换成自己的强密码'
```

把输出写入部署环境的 secret 管理器；不要把密码或哈希提交到仓库。哈希只在认证库建立时读取一次，之后改密码用 `rssr-web passwd`。数据卷丢失时认证库随之丢失：再次启动会按初始凭据的顺序重新建立，所有浏览器需要重新登录。

若把 Argon2 哈希写进 Compose 的 `.env` 文件，请使用单引号包住整个值，例如 `RSS_READER_WEB_PASSWORD_HASH='$argon2id$...'`；Compose 会对未引号或双引号的 `$` 内容做变量插值，[单引号值保持原样](https://docs.docker.com/compose/how-tos/environment-variables/variable-interpolation/#env-file-syntax)。可用 `docker compose config --quiet` 检查配置语法，避免用会打印密码哈希的完整 `config` 输出分享诊断。

Compose 的端口发布默认不限制宿主监听地址。对外部署时应由防火墙或反向代理控制访问，并确认 HTTPS 转发与 `RSS_READER_WEB_PUBLIC_ORIGIN` 与实际入口一致。`/healthz` 可用于健康检查。

## 备份与恢复

数据目录（容器内 `/data`）里有三样东西：资料库 `library.sqlite3`（订阅、文章、正文与设置，连同 SQLite 的 `-wal`、`-shm`）、认证库 `auth.sqlite3` 与出站策略 `outbound.json`。

备份资料库用 `rssr-cli backup`，服务器运行时也可以做：

```bash
rssr-cli --data-dir /data backup /data/backups/library-2026-10-19.sqlite3
```

- 得到的是某一时刻的一致副本（SQLite `VACUUM INTO`）；资料库是 WAL 模式（服务器与 CLI 打开时设置），服务器照常读写。在 Linux 等 Unix 系统上备份文件只有所有者可读写；Windows 上它继承所在目录的权限，请放在只有自己能访问的目录。
- 只复制已有的资料库：数据目录里没有 `library.sqlite3` 时报错，不会建一个空库，也不会导入 v0.1 的旧库。
- 目标文件必须不存在，已存在时拒绝、不覆盖。对同一目标同时进行的备份互相等待：等锁以 10 秒为限，每 50 毫秒试一次，打开锁文件与调度的时间另算。`<文件>.lock` 会留在原处，它必须是普通文件、不能是链接，备份只锁它、从不写入。后一个备份拿到锁后发现目标已存在即报错。
- 文件名必须以 `.sqlite3` 结尾（小写），例如 `library-2026-10-19.sqlite3`。其他名字留给备份自己的锁和临时文件，以及 SQLite 的附属文件：叫 `library.sqlite3-wal` 的备份，会在资料库下次打开时被当作它的日志覆盖；只认 `.sqlite3` 结尾，不论文件系统怎样折叠大小写，都不会撞上这些名字。
- 临时文件放在目标旁边的目录 `<文件>.partial` 里（备份结束后目录为空就删掉）。进程被强行结束留下的临时文件，由下一次对同一目标的备份先清理掉，哪怕目标随后被判定为已存在；清理只删这个目录里名为 `partial-<32 位十六进制>`（及其 `-journal`、`-wal`、`-shm`）的文件，目录里别的东西不动。这个名字若是普通文件或链接，备份报错。在不区分大小写的文件系统上，只是大小写不同的名字落到同一个锁文件和同一个目录，也就是同一目标。
- 复制默认以 10 分钟为限（`--timeout <秒>`，过大的值等于不设时限）。这个时限只管复制语句：SQLite 每执行一批指令检查一次，单条很大的记录要复制完才轮到检查，所以是近似上限；语句结束之后不再按时限中断。超时或按 Ctrl-C 时，复制在下一次检查时停止，命令等它停下、尝试删掉临时文件后再报告（删不掉的留在 `<文件>.partial` 里，由下一次备份清理）。Ctrl-C 若来得太晚，备份已经写入，命令照实报告已备份。等锁和复制的时限都不限制整条命令的总时长。
- 等锁时按 Ctrl-C 会停下。打断不了、Ctrl-C 只在之后的检查点生效的：SQLite 等待其他连接释放锁（忙等超时是累计睡眠的预算，不是严格的钟表时间，这段睡眠计入时限）、关闭连接（它是资料库最后一个连接时，不论在哪个进程，关闭会把 WAL 写回资料库），以及发布前后的文件操作。
- 复制期间服务器照常写入，这些写入积在 `library.sqlite3-wal` 里，复制结束后才能回收：数据卷要为备份期间的写入留出空间。
- 备份以硬链接发布，目标所在的文件系统必须支持硬链接：Linux 常见的 ext4、xfs、btrfs 与 Windows 的 NTFS 支持，FAT、exFAT 与部分网络文件系统不支持；放在数据卷里时，取决于宿主机上数据卷所在的文件系统。不支持时报错，不会改用可能覆盖文件的改名。建立硬链接报错时（网络文件系统可能已建好链接却丢了回复），命令会再看一眼目标：在 Linux 等 Unix 系统上能认出已是这次的备份就报告已备份；目标名下有文件却认不出时报“无法确认”，请自行检查。
- 前提：备份所在目录只由运行备份的用户掌控，其中没有刻意制造的别名——没有硬链接、符号链接或挂载（包括 bind mount）让 `<文件>.lock` 或 `<文件>.partial` 成为别的文件、别的目录的另一个名字。协议能查到的几种（锁文件不是普通文件，Linux 等 Unix 系统上锁文件还有别的名字，`<文件>.partial` 不是目录）会报错，但这只是保险，不是防御：它查不出 bind mount，也不防备备份进行时有人替换其中的名字。别的程序放进 `<文件>.partial` 的、名字像临时文件的文件，会被当作残留删掉。
- 清理残留时会读 `<文件>.partial` 里的每一项（逐项检查 Ctrl-C），目录里堆了很多别的文件时这一步会相应变慢。
- 以拥有数据目录的用户运行（镜像内附带 CLI 的用法随部署工作补充）。认证库与出站策略另行保存：认证库丢失后按初始凭据的顺序重建，所有浏览器需要重新登录。

恢复：

1. 停止服务器与所有 CLI，确认没有进程打开资料库。
2. 把旧的 `library.sqlite3` 与它的 `-wal`、`-shm` 一起移走，再把备份文件放到 `library.sqlite3`，属主与权限和数据目录一致（仅所有者可读写）。
3. 重新放置 `outbound.json`。
4. 两条路：
   - 已有 `auth.sqlite3`：运行 `rssr-web sessions revoke-all`，保留凭据、吊销全部会话；
   - 新主机、没有 `auth.sqlite3`：按上文初始凭据的顺序有意地提供当前凭据。
5. 启动服务器。

## 从 v0.1 升级

- **文章不迁移：**旧版 Web 的浏览器文章库、已读与收藏不会导入服务器。升级前在旧版导出 JSON 配置或 OPML，新版登录后导入，再由服务器刷新；feed 已不再提供的旧文章可能无法重新取得。新版不删除旧浏览器数据，但也不读取它作为后备；需要旧文章时先保留旧站点数据。
- **不再离线可读：**正式 Web 需要服务器在线，离线或服务器不可用时显示读取失败；Pages 演示的浏览器样例库是另一种构建。
- 凭据相关的环境变量不用改。新增必填的 `RSS_READER_WEB_PUBLIC_ORIGIN`，以及数据卷 `/data`（仓库的 Compose 文件已包含）。
- 没有设置 `RSS_READER_WEB_PASSWORD_HASH` / `RSS_READER_WEB_PASSWORD` 时，首次启动从旧认证状态文件取凭据：`RSS_READER_WEB_AUTH_STATE_FILE`（Compose 默认 `/app/auth/auth.json`），未设置时为 `$HOME/.rssr-web-auth.json`。服务只读取其中的密码哈希，不修改这个文件。
- 认证库建立成功之前，保持旧认证卷 `/app/auth` 挂载；之后可以移除。
- 旧的登录 cookie 全部失效，升级后每个浏览器需要重新登录一次。
- 不再使用的变量：`RSS_READER_WEB_SESSION_SECRET`、`RSS_READER_WEB_SESSION_TTL_HOURS`（会话规则见上文）、`RSS_READER_WEB_SECURE_COOKIE`（由公开地址的协议决定）。设置了只记一条警告。

## 本地验证部署态

正式 Web 开发使用同源服务。下面的手工入口从同一源码构建页面与服务器，用临时资料库和仓库测试凭据启动服务，打印登录地址：

```bash
bash scripts/run_rssr_web_browser_smoke.sh --port 18081
```

自动化验证使用独立资料库与本地 feed 服务器：

```bash
bash scripts/run_server_web_regression.sh --suite feed-smoke
bash scripts/run_server_web_regression.sh --suite acceptance --long
bash scripts/run_release_ui_regression.sh --with-fixed-smokes
```

需要自行启动时，先 `dx build --platform web --package rssr-app --locked`，再设置 `RSS_READER_WEB_STATIC_DIR=target/dx/rssr-app/debug/web/public`、独立 `RSSR_DATA_DIR`、开发用凭据和回环监听地址，运行 `cargo run --locked -p rssr-web`。构建期间保持源码版本相同；若显式设置 `RSSR_BUILD_ID`，页面和服务器必须使用同一个值。`dx serve` 本身不提供服务器 API，不能作为正式 Web 的完整开发入口。

`--skip-build` 只跳过页面构建；入口仍会构建服务器与夹具工具。提交或改源码后应重新构建页面，避免旧页面与新服务器的标识不匹配。套件、控制台记录与产物说明见 [服务器浏览器回归](../testing/server-web-regression.md)。本地夹具通过不证明真实外部 feed 可达。

## 其他配置与构建

[`docker-compose.yml`](../../docker-compose.yml) 是环境变量和健康检查的权威模板。数据目录由 `RSSR_DATA_DIR`（或 `--data-dir`）指定，镜像内默认 `/data`；目录不存在时服务以仅所有者可访问（0700）的权限创建它。目录与 `auth.sqlite3`（以及已有的 `-wal`、`-shm`、`-journal` 文件）必须属于运行服务的用户，其他用户不能写入目录、不能读取认证库，否则服务拒绝启动。服务启动时把资料库 `library.sqlite3` 及其伴随文件改为仅所有者可读写（例如容器里用默认 umask 运行的 CLI 建出的资料库）；这些文件属于其他用户时拒绝启动。这些检查只在 Linux 等 Unix 系统上进行；Windows 上不检查文件 ACL，服务器部署请使用 Linux 或容器。在容器里运行 `rssr-web passwd` 等子命令时，用镜像默认的运行用户（`docker compose exec` 的默认值）。可选项还有 `RSS_READER_IMAGE` 和 `RSS_READER_PORT`。`RSS_READER_WEB_ENV` 默认为 `development`，正式部署应显式设置。

要从当前源码构建镜像：

```bash
export RSSR_BUILD_ID="git-$(git rev-parse HEAD)"
docker compose -f docker-compose.yml -f docker-compose.build.yml up --build
```

源码构建必须显式提供 `RSSR_BUILD_ID`，且不能为 `dev-unknown`；同一值交给页面和服务的构建脚本。未提交的产品修改需要使用不同的明确标识，不能沿用旧提交标识冒充同一产物。容器内以默认的 `rssr` 用户执行 `docker compose exec rss-reader rssr-cli show-settings`；也可明确指定 `docker compose exec --user rssr rss-reader rssr-cli ...`。新卷和资料库必须属于该用户；已有数据不要通过换用户或放宽权限规避检查。

网络专用 NAT64 前缀可能处在普通全局 IPv6 地址中，应用不能仅从地址识别其转换目标。如果部署网络会把该前缀映射到内网 IPv4，必须在出口防火墙阻断，或在 `outbound.json` 的 `deny` 中列明该前缀；不能只依赖 `public_only` 推断其安全。

仓库的 `.dockerignore` 会排除 `.env` 和 `.env.*`，避免把本地认证变量随 `COPY . .` 放进镜像；其他私有文件也应放在构建上下文之外。

更详细的浏览器验证入口见[测试与回归索引](../testing/README.md)。

# Web / Docker 部署

`RSS-Reader-web.tar.gz` 是 Dioxus 静态站点包。浏览器直接请求 feed 时受目标站点 CORS 策略约束。需要登录门禁和同源 `/feed-proxy` 时，使用仓库里的 `rssr-web` 服务；它托管静态包、验证登录并代抓 feed，不参与桌面端、Android 或 CLI 的运行。Web 的订阅和阅读状态保存在访问者当前浏览器的 `localStorage`。服务器的数据目录（容器内 `/data`）保存认证库 `auth.sqlite3`（登录凭据与会话），以及服务器端的资料库 `library.sqlite3` 与出站策略 `outbound.json`（没有时以默认的服务器策略写入）。资料库供服务器接口 `/api/v1` 使用（v0.2 阶段 C）；网页改用服务器之前它是空的，**不是网页数据的备份**。

新版浏览器存储使用 Web Locks 协调多个标签页，需要支持该能力的浏览器和安全上下文：正式入口使用 HTTPS，本机 localhost / 127.0.0.1 可使用 HTTP；手机通过局域网 IP 访问普通 HTTP 不满足此条件。缺少能力会显示错误，不会退回可能覆盖数据的无锁写入。升级静态包后应重新加载所有旧标签页，避免混用两种存储协议；不会自动刷新正在阅读的页面。

`/feed-proxy` 只代理公开的 HTTP(S) feed：本地和内网地址（包括 IPv4 映射的 IPv6 地址）会被拒绝；DNS 解析限时，重定向每跳重新校验目标。部署在内网的私有 feed 不能通过这个入口抓取。

代理内容仍属于不可信数据。`rssr-web` 自身在上游响应上设置无例外的 CSP `sandbox`、`default-src 'none'`、`base-uri 'none'`、`form-action 'none'` 和 `X-Content-Type-Options: nosniff`。直接打开 HTML/SVG 等代理文档时，脚本和应用同源存储访问被隔离；这不依赖反向代理另加 CSP。原始 MIME/字符集、正文和最终 URL 继续供客户端读取，以保留 RSS/Atom 解码与 HTML 订阅发现。反向代理应保留这些响应头。

此入口直接连接校验后固定的目标 IP，不继承 `HTTP_PROXY` / `HTTPS_PROXY` / `ALL_PROXY` 等环境代理，以免代理端再次解析主机名而绕过目标校验。部署环境需允许服务器直接访问公开 feed，并让 DNS 返回真实公网地址。若 DNS 返回 `198.18.0.0/15` 等 fake-IP 地址，请为该服务调整 DNS/网络配置；这些地址会被目标校验拒绝。

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

`latest` 指向最近一次镜像发布，与当前 `main` 不一定相同；需要固定版本时设置 `RSS_READER_IMAGE=ghcr.io/develata/rss-reader:<tag>`。下载包及镜像的实际版本以对应 [Release](https://github.com/Develata/RSS-Reader/releases) 为准。

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

`production` 要求 `RSS_READER_WEB_PUBLIC_ORIGIN` 是 https 地址，cookie 随之带 `Secure`。反向代理要原样转发浏览器的 `Origin`、`Cookie` 请求头和 `Set-Cookie` 响应头。`RSS_READER_WEB_TRUST_PROXY_HEADERS` 仅在可信反向代理正确覆盖转发头时启用；默认保持 `false`。

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

## 从 v0.1 升级

- 凭据相关的环境变量不用改。新增必填的 `RSS_READER_WEB_PUBLIC_ORIGIN`，以及数据卷 `/data`（仓库的 Compose 文件已包含）。
- 没有设置 `RSS_READER_WEB_PASSWORD_HASH` / `RSS_READER_WEB_PASSWORD` 时，首次启动从旧认证状态文件取凭据：`RSS_READER_WEB_AUTH_STATE_FILE`（Compose 默认 `/app/auth/auth.json`），未设置时为 `$HOME/.rssr-web-auth.json`。服务只读取其中的密码哈希，不修改这个文件。
- 认证库建立成功之前，保持旧认证卷 `/app/auth` 挂载；之后可以移除。
- 旧的登录 cookie 全部失效，升级后每个浏览器需要重新登录一次。
- 不再使用的变量：`RSS_READER_WEB_SESSION_SECRET`、`RSS_READER_WEB_SESSION_TTL_HOURS`（会话规则见上文）、`RSS_READER_WEB_SECURE_COOKIE`（由公开地址的协议决定）。设置了只记一条警告。

## 本地验证部署态

如果需要在不运行 Docker 的情况下验证登录和 `/feed-proxy`，先构建 Web 包，再从同一工作区启动服务：

```bash
dx bundle --locked --platform web --package rssr-app --release --debug-symbols false --out-dir target/web-e2e
cargo run --locked -p rssr-web -- --print-password-hash '请换成自己的测试密码'
```

将第二条命令的输出放入 `RSS_READER_WEB_PASSWORD_HASH`，然后用一个临时数据目录启动：

```bash
RSS_READER_WEB_BIND=127.0.0.1:8060 \
RSS_READER_WEB_STATIC_DIR=target/web-e2e/public \
RSSR_DATA_DIR=target/web-e2e/data \
RSS_READER_WEB_USERNAME=admin \
RSS_READER_WEB_PASSWORD_HASH='<填入上一步哈希>' \
RSS_READER_WEB_ENV=development \
cargo run --locked -p rssr-web
```

访问 `http://127.0.0.1:8060/login`；要验收真实代理 feed，使用仓库已有的 [`rssr-web` 代理 Feed Smoke](../testing/rssr-web-proxy-feed-smoke.md)。静态 Web 包的成功构建不证明远端 feed 在直接浏览器模式下都能抓取。

## 其他配置与构建

[`docker-compose.yml`](../../docker-compose.yml) 是环境变量和健康检查的权威模板。数据目录由 `RSSR_DATA_DIR`（或 `--data-dir`）指定，镜像内默认 `/data`；目录与 `auth.sqlite3`（以及已有的 `-wal`、`-shm`、`-journal` 文件）必须属于运行服务的用户，其他用户不能写入目录、不能读取认证库，否则服务拒绝启动。这些检查只在 Linux 等 Unix 系统上进行；Windows 上不检查文件 ACL，服务器部署请使用 Linux 或容器。在容器里运行 `rssr-web passwd` 等子命令时，用镜像默认的运行用户（`docker compose exec` 的默认值）。可选项还有 `RSS_READER_IMAGE` 和 `RSS_READER_PORT`。`RSS_READER_WEB_ENV` 默认为 `development`，正式部署应显式设置。

要从当前源码构建镜像：

```bash
docker compose -f docker-compose.yml -f docker-compose.build.yml up --build
```

仓库的 `.dockerignore` 会排除 `.env` 和 `.env.*`，避免把本地认证变量随 `COPY . .` 放进镜像；其他私有文件也应放在构建上下文之外。

更详细的浏览器验证入口见[测试与回归索引](../testing/README.md)。

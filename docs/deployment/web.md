# Web / Docker 部署

`RSS-Reader-web.tar.gz` 是 Dioxus 静态站点包。浏览器直接请求 feed 时受目标站点 CORS 策略约束。需要登录门禁和同源 `/feed-proxy` 时，使用仓库里的 `rssr-web` 服务；它托管静态包、验证登录并代抓 feed，不参与桌面端、Android 或 CLI 的运行。Web 的订阅和阅读状态保存在访问者当前浏览器的 `localStorage`，容器卷只保存服务登录凭据，**不是文章库备份**。

## 使用已发布镜像

仓库根目录的 [`docker-compose.yml`](../../docker-compose.yml) 直接拉取 `ghcr.io/develata/rss-reader:latest`，默认映射宿主端口 `8039` 到容器 `8080`。在克隆的仓库中创建仅供本机使用的 `.env`：

```dotenv
RSS_READER_WEB_USERNAME=admin
RSS_READER_WEB_PASSWORD=请改成至少八位的密码
RSS_READER_WEB_AUTH_STATE_FILE=/app/auth/auth.json
RSS_READER_WEB_ENV=development
RSS_READER_WEB_SECURE_COOKIE=false
RSS_READER_WEB_TRUST_PROXY_HEADERS=false
RSS_READER_PORT=8039
```

```bash
docker compose up -d
```

然后访问 `http://localhost:8039/login`。首次启动时，`rssr-web` 会把明文密码转换成 Argon2 哈希，同时生成随机 session secret，并把两者写入持久卷中的 `/app/auth/auth.json`。成功启动后可从 `.env` 移除 `RSS_READER_WEB_PASSWORD`，保留认证状态卷；重启会继续读取已有哈希与 secret。不要把含密码的 `.env` 提交到版本库。改变用户名、密码哈希或 session secret 会改变登录结果；更换 secret 会使旧会话失效。

`latest` 指向最近一次镜像发布，与当前 `main` 不一定相同；需要固定版本时设置 `RSS_READER_IMAGE=ghcr.io/develata/rss-reader:<tag>`。下载包及镜像的实际版本以对应 [Release](https://github.com/Develata/RSS-Reader/releases) 为准。

## 正式部署

通过你控制的 HTTPS 反向代理对外提供服务，并设置：

```dotenv
RSS_READER_WEB_ENV=production
RSS_READER_WEB_SECURE_COOKIE=true
RSS_READER_WEB_USERNAME=请设置用户名
RSS_READER_WEB_PASSWORD_HASH=请填入Argon2哈希
RSS_READER_WEB_AUTH_STATE_FILE=/app/auth/auth.json
RSS_READER_WEB_TRUST_PROXY_HEADERS=false
```

`production` 要求 `Secure` cookie。`RSS_READER_WEB_TRUST_PROXY_HEADERS` 仅在可信反向代理正确覆盖转发头时启用；默认保持 `false`。密码哈希可在源码工作区生成：

```bash
cargo run --locked -p rssr-web -- --print-password-hash '请换成自己的强密码'
```

把输出写入部署环境的 secret 管理器；不要把密码、哈希或 session secret 提交到仓库。也可以先用明文密码完成一次启动，让服务持久化哈希，然后移除明文变量。认证卷丢失且未另行提供密码哈希时，服务无法沿用旧登录身份；更换 secret 后用户需要重新登录。

若把 Argon2 哈希写进 Compose 的 `.env` 文件，请使用单引号包住整个值，例如 `RSS_READER_WEB_PASSWORD_HASH='$argon2id$...'`；Compose 会对未引号或双引号的 `$` 内容做变量插值，[单引号值保持原样](https://docs.docker.com/compose/how-tos/environment-variables/variable-interpolation/#env-file-syntax)。可用 `docker compose config --quiet` 检查配置语法，避免用会打印密码哈希的完整 `config` 输出分享诊断。

Compose 的端口发布默认不限制宿主监听地址。对外部署时应由防火墙或反向代理控制访问，并确认 HTTPS 转发和 cookie 设置与实际入口一致。`/healthz` 可用于健康检查。

## 本地验证部署态

如果需要在不运行 Docker 的情况下验证登录和 `/feed-proxy`，先构建 Web 包，再从同一工作区启动服务：

```bash
dx bundle --locked --platform web --package rssr-app --release --debug-symbols false --out-dir target/web-e2e
cargo run --locked -p rssr-web -- --print-password-hash '请换成自己的测试密码'
```

将第二条命令的输出放入 `RSS_READER_WEB_PASSWORD_HASH`，然后在一个临时目录或隔离的认证状态文件下启动：

```bash
RSS_READER_WEB_BIND=127.0.0.1:8060 \
RSS_READER_WEB_STATIC_DIR=target/web-e2e/public \
RSS_READER_WEB_AUTH_STATE_FILE=target/web-e2e/auth.json \
RSS_READER_WEB_USERNAME=admin \
RSS_READER_WEB_PASSWORD_HASH='<填入上一步哈希>' \
RSS_READER_WEB_ENV=development \
cargo run --locked -p rssr-web
```

访问 `http://127.0.0.1:8060/login`；要验收真实代理 feed，使用仓库已有的 [`rssr-web` 代理 Feed Smoke](../testing/rssr-web-proxy-feed-smoke.md)。静态 Web 包的成功构建不证明远端 feed 在直接浏览器模式下都能抓取。

## 其他配置与构建

[`docker-compose.yml`](../../docker-compose.yml) 是环境变量和健康检查的权威模板。可选项包括 `RSS_READER_WEB_SESSION_SECRET`（显式提供时至少 32 字符）、`RSS_READER_WEB_SESSION_TTL_HOURS`（默认 12）、`RSS_READER_IMAGE` 和 `RSS_READER_PORT`。`RSS_READER_WEB_PASSWORD_HASH`、`RSS_READER_WEB_PASSWORD` 二选一；如果两者都为空，只有已有认证状态文件包含哈希时才能启动。`RSS_READER_WEB_ENV` 默认为 `development`，正式部署应显式设置。

要从当前源码构建镜像：

```bash
docker compose -f docker-compose.yml -f docker-compose.build.yml up --build
```

仓库的 `.dockerignore` 会排除 `.env` 和 `.env.*`，避免把本地认证变量随 `COPY . .` 放进镜像；其他私有文件也应放在构建上下文之外。

更详细的浏览器验证入口见[测试与回归索引](../testing/README.md)。

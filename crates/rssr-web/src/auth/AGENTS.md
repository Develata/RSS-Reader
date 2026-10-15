# rssr-web 认证模块说明

本目录负责 `rssr-web` 的登录、会话、请求来源校验和限速（v0.2 阶段 C，C2；计划见 `docs/design/v0.2-phase-c-plan.md`）。

## 职责边界

- `store.rs`
  - 认证库 `auth.sqlite3`（数据目录里，与资料库分开）：所有者凭据（Argon2 哈希、凭据代数、设备标记密钥）与会话（令牌摘要、创建时间、最后使用时间、凭据代数）
  - 会话规则：30 天未使用过期（加 1 小时记录粒度）、180 天绝对上限、时钟回拨超过 5 分钟拒绝；任何拒绝都删除该会话，续期只更新仍存在的行
- `credentials.rs`
  - Argon2 哈希；认证库建立时的初始凭据（哈希变量 → 明文变量 → 旧认证状态文件）
- `legacy_state.rs`
  - v0.1 认证状态文件：只读，从不写入或改权限
- `origin.rs`
  - 信任的来源：`RSS_READER_WEB_PUBLIC_ORIGIN`，未设置时只信任实际监听的回环地址
- `session.rs`
  - 会话、门禁与已知设备 cookie
- `rate_limit.rs`
  - 登录限速
- `auth.rs`
  - 路由与中间件：写请求的 Origin 校验、页面与 `/api` 的会话检查、`/api` 写请求的 CSRF 校验、登录、退出

## 安全边界

- 这里只做 Web 部署态的访问控制
- 不把这层安全语义投射到 desktop / Android
- 默认不信任代理头，只有显式开启后才读取 `X-Forwarded-For / X-Real-IP`
- 认证库包含密码哈希与会话摘要：文件以 0600 创建，Unix 上数据目录与认证库必须属于运行用户、其他用户不能写目录或读认证库，否则拒绝启动
- 会话令牌只在 cookie 里，库里只存摘要；CSRF 令牌由会话令牌派生，不另存
- 服务器不缓存密码哈希：登录在同一次读取里拿到哈希与凭据代数，建立会话时再核对代数

## 不应在这里做的事

- 不在这里处理 feed 抓取代理逻辑
- 不在这里写前端应用路由或静态资源打包逻辑
- 不在这里引入 RSS 领域逻辑或资料库查询
- 不增加测试专用路由：冒烟测试经真实登录进入，测试页面以静态文件提供

## 修改约束

- 避免新增 `expect`/`unwrap` 让错误配置直接 panic
- 生产环境约束必须继续保持硬性校验（https 公开来源）
- 认证库出错时拒绝请求（偏于关闭）
- 登录限速、cookie、会话规则变更时，要优先考虑：
  - 旧部署升级（凭据迁移、旧认证卷只读）
  - 反向代理部署
  - 本地开发态（回环地址）
  - 受控时钟测试（时间由调用方传入）

## 变更后建议检查

- `cargo test -p rssr-web`（含 `http_tests.rs`：真实 HTTP 下的 Origin、CSRF、`/api` 边界与退出）
- `cargo clippy -p rssr-web --all-targets -- -D warnings`
- Unix 上的属主与权限测试需在 Linux（例如 WSL）运行
- `scripts/run_rssr_web_auth_assertions.sh` 经 release-ui runner：`--web-auth-only`

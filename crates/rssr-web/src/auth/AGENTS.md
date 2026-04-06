# rssr-web 认证模块说明

本目录负责 `rssr-web` 的登录、会话和限速能力。

## 职责边界

- `config.rs`
  - 认证配置加载
  - 自动生成并持久化密码哈希 / session secret
- `session.rs`
  - 会话 token 与 cookie 处理
- `rate_limit.rs`
  - 登录限速
- `auth.rs`
  - 登录/登出/守卫路由与页面

## 安全边界

- 这里只做 Web 部署态的访问控制
- 不把这层安全语义投射到 desktop / Android
- 默认不信任代理头，只有显式开启后才读取 `X-Forwarded-For / X-Real-IP`
- 认证状态文件里包含敏感数据：
  - `password_hash`
  - `session_secret`
  因此权限与落盘行为必须保守

## 不应在这里做的事

- 不在这里处理 feed 抓取代理逻辑
- 不在这里写前端应用路由或静态资源打包逻辑
- 不在这里引入 RSS 领域逻辑或数据库查询

## 修改约束

- 避免新增 `expect`/`unwrap` 让错误配置直接 panic
- 生产环境约束必须继续保持硬性校验
- 登录限速、cookie、session 规则变更时，要优先考虑：
  - 明文密码配置的过渡路径
  - 自动生成认证状态文件
  - 反向代理部署
  - 本地开发态

## 变更后建议检查

- `cargo check -p rssr-web`
- `cargo test -p rssr-web`
- `cargo clippy -p rssr-web --all-targets -- -D warnings`


# Web Bootstrap 模块说明

本目录负责 `rssr-app` 在纯 Web / `rssr-web` 部署态下的运行时装配。

## 职责边界

- `state.rs`
  - 浏览器本地状态结构
  - `localStorage` 持久化与损坏恢复
- `query.rs`
  - 只读查询
  - 文章列表、订阅列表、阅读导航的 Web 内存态实现
- `mutations.rs`
  - 状态修改
  - 已读、收藏、最近打开订阅、设置写回
- `refresh.rs`
  - feed 刷新、自动刷新调度
- `exchange.rs`
  - JSON / OPML / 配置交换
- `config.rs`
  - 配置包校验与 OPML 编解码
- `feed.rs`
  - feed 拉取与解析辅助

## 不应在这里做的事

- 不在这里渲染 UI
- 不在这里写桌面端 / Android 专用逻辑
- 不在这里引入 `sqlx` 或原生 SQLite 路径
- 不把 `rssr-web` 的服务端认证/代理职责混进来

## 修改约束

- Web 端是本地优先实现，当前真实持久化方案是 `localStorage` 序列化状态
- 任何会影响用户数据的变更，都要优先考虑：
  - 状态损坏恢复
  - 导入坏数据时的降级路径
  - 保存失败时不应静默破坏现有状态
- 查询路径优先减少重复线性扫描，避免在热点路径上反复 `find` / `filter`
- 状态写回尽量保持“锁内修改，锁外序列化/持久化”

## 代码风格

- 查询、修改、刷新、交换逻辑继续分文件，不回流到 `web.rs`
- 错误优先返回可读消息，不使用 `expect` 假设浏览器状态永远有效
- Web 专属辅助函数命名要能一眼看出是 Web 实现，不要伪装成通用仓储

## 变更后建议检查

- `cargo check -p rssr-app --target wasm32-unknown-unknown`
- 关键改动涉及状态时：
  - 登录后的 `/entries`
  - 配置导入导出
  - 刷新 feed
  - 阅读页导航


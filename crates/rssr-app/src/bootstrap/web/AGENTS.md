# Web Bootstrap 模块说明

本目录负责 `rssr-app` 在纯 Web / `rssr-web` 部署态下的运行时装配。

## 职责边界

- `../web.rs`
  - 组装 infra 浏览器适配器与 application 用例，建好 `LocalBackend<WebHost>`；`WebHost` 只提供浏览器的差异（`spawn_local`、gloo 计时器、浏览器时钟、远端配置的 `fetch` 存储、演示站的网络与自动刷新开关）
- 添加订阅、刷新编排、`RefreshFlight` 与自动刷新循环在 `rssr_infra::local_backend`，与原生端共用；不要在此重建。
- 浏览器状态、仓储查询、feed 拉取解析和 OPML 编解码已位于 `rssr-infra/src/application_adapters/browser/`；不要在此重建平行实现。

## 不应在这里做的事

- 不在这里渲染 UI
- 不在这里写桌面端 / Android 专用逻辑
- 不在这里引入原生 SQLite（`rssr_infra::store`）路径
- 不把 `rssr-web` 的服务端认证/代理职责混进来

## 修改约束

- Web 端是本地优先实现，当前真实持久化方案是 `localStorage` 序列化状态
- 任何会影响用户数据的变更，都要优先考虑：
  - 状态损坏恢复
  - 导入坏数据时的降级路径
  - 保存失败时不应静默破坏现有状态
- 查询路径优先减少重复线性扫描，避免在热点路径上反复 `find` / `filter`
- `BrowserStore` 的写入必须在同一次 Web Lock 内完成读取最新提交、修改、序列化及发布提交头；不能把持久化移到锁外，否则不同标签页会丢失更新。
- 网络请求放在存储锁外；返回后由仓储在锁内合并并提交。保存失败必须保留上一份可读提交，并向调用方报告失败。

## 代码风格

- 业务语义放在 application；浏览器存储与协议适配放在 infra；本目录只保留 host 装配和能力协调。
- 错误优先返回可读消息，不使用 `expect` 假设浏览器状态永远有效
- Web 专属辅助函数命名要能一眼看出是 Web 实现，不要伪装成通用仓储

## 变更后建议检查

- `cargo check -p rssr-app --target wasm32-unknown-unknown --locked`
- 存储/端口变化运行相关 wasm 浏览器契约测试，入口见 `scripts/run_wasm_contract_harness.sh`。
- 关键改动涉及状态时：
  - 登录后的 `/entries`
  - 配置导入导出
  - 刷新 feed
  - 阅读页导航

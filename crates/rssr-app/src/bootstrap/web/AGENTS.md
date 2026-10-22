# Web Bootstrap 模块说明

核对：2026-10-22，v0.2 阶段 D。

- 正式 Web：`HttpBackend` 以页面 origin 和构建标识调用服务器 `/api/v1`；不编译 `memory-store`，不保存浏览器文章库。
- Pages：`demo::open_library` 经共享仓储端口载入预抓取 RSS；`compose_offline_library_use_cases` 装配同一组 SQL 仓储和用例；`LocalBackend<WebHost>` 提供页面任务、时钟与计时器，禁用网络及自动刷新。
- 浏览器连接归 `rssr-infra/src/store/wasm_executor.rs`；这里不写 SQL、事务或解析逻辑，不引入原生文件库路径。
- 资料库每次重新加载重建；旧 BrowserStore / Web Locks 路径已删除，旧浏览器数据不读取、不迁移、不清除。设备偏好与正式 Web 的 sessionStorage 草稿是独立职责。
- 页面只通过 Backend 使用用例。添加、刷新、RefreshFlight 与自动刷新编排归 LocalBackend，不在此复制。
- 网络禁用同时守住 host 和 composition 的网络端口；不装配浏览器 WebDAV、feed proxy 或在线订阅客户端。

变更后分别检查正式 wasm 与 `pages-demo` 的 check/clippy、依赖图、共享 SQL 浏览器契约和 `scripts/test_pages.sh`。正式 Web 用真实服务器回归，不能以演示页面代替服务器验收。

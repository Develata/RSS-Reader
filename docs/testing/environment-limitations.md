# 环境限制索引

这份文档记录当前仓库中“已知会影响验证结果，但不应直接判定为代码回归”的环境限制。

它的目标是：

- 帮助后续验证时先区分环境问题和代码回归
- 给每个限制提供可复验、可规避的处理方式
- 避免同一个限制反复写进 handoff 却没有长期索引

## 使用规则

- 如果某次验证失败先落在这里的限制项上，结果应标记为 `env-limited`，而不是直接记为功能回归
- `env-limited` 结论必须同时记录：
  - 日期
  - 验证环境
  - 失败入口
  - 复验方式
- 如果后续发现某限制已经不再成立，应更新本页，而不是只在新的 handoff 里零散备注

## 限制项

### 1. `test_webdav_local_roundtrip` 的 loopback 端口权限限制

- 受影响入口：
  - [test_webdav_local_roundtrip.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-infra/tests/test_webdav_local_roundtrip.rs)
  - `cargo test --workspace`
  - `cargo test -p rssr-infra --test test_webdav_local_roundtrip`
- 触发环境：
  - 受限沙箱
  - 禁止本地 loopback 端口绑定的执行环境
- 现象：
  - 测试在本地临时 HTTP 服务监听阶段失败
  - 全量测试中只剩这一项报错，容易被误认为 infra 或 WebDAV 逻辑回归
- 为什么不算代码回归：
  - 在允许本地端口绑定的环境中，该测试可通过
  - 当前主线已有多次记录表明问题来自环境权限，而不是配置交换逻辑本身
- 规避方式 / 复验方式：
  - 在受限环境下，将该项明确标记为 `env-limited`
  - 在允许 loopback 端口绑定的环境中单独复跑 `cargo test -p rssr-infra --test test_webdav_local_roundtrip`

### 2. 纯静态 Web 路径的 CORS 限制

- 受影响入口：
  - `dx serve --platform web --package rssr-app`
  - 纯静态 bundle 的浏览器直连路径
  - Web 端 feed 添加、刷新、图片加载
- 触发环境：
  - 浏览器直接访问不开放 CORS 的 RSS / Atom 源
  - 浏览器直接加载跨源正文图片
- 现象：
  - 某些 feed 在 Web 端无法添加或刷新
  - 某些正文图片在浏览器中无法加载
  - Console 可能出现跨域相关报错
- 为什么不算代码回归：
  - 这是浏览器安全模型边界，不是本仓库业务逻辑独有问题
  - 同样的 feed 在 `rssr-web` 同源代理模式下通常可成功导入或刷新
- 规避方式 / 复验方式：
  - Web smoke 优先使用已知开放 CORS 的 feed 源
  - 需要验证跨源抓取时，优先通过 `rssr-web` 的同源代理模式复验

### 3. WSLg 宿主窗口行为问题

- 受影响入口：
  - Desktop 手工验收
  - `cargo run -p rssr-app` 在 WSL Ubuntu + WSLg 图形宿主下的窗口交互
- 触发环境：
  - WSLg
  - 特定宿主窗口管理行为
- 现象：
  - 标题栏右上角最小化、最大化、关闭按钮无响应
- 为什么不算代码回归：
  - 主功能链路仍可通过
  - 更像宿主图形环境或窗口管理行为，不是阅读、订阅、刷新、配置交换等主 use case 的业务回归
- 规避方式 / 复验方式：
  - 记录为宿主环境备注，不单独阻塞主线功能验证
  - 如需确认是否为应用回归，应在非 WSLg 桌面环境复验相同步骤

## 与 handoff 的关系

- 本页负责长期维护的环境限制索引
- `docs/handoffs/` 负责记录“某次工作在哪个环境里遇到了哪条限制”
- 如果某个限制只写在 handoff 里而没有进入本页，说明它还没有被正式纳入长期验证规则

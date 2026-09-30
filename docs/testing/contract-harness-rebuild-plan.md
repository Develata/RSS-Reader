# Contract harness：当前基线与维护

原重建计划已完成；保留文件名以兼容引用。当前入口核对于 2026-09-30，历史重建过程可从 Git 历史追溯。

## 范围与分层

三组 harness 验证同一 application 契约在 SQLite/native 和 browser adapter 上的行为。两种 target 分开执行，不为测试统一形状把 wasm adapter 编入 native，也不复制一套临时存储模型。

| 语义 | Native 测试 target | 浏览器测试 target |
| --- | --- | --- |
| 刷新 | `test_refresh_contract_harness` | `wasm_refresh_contract_harness` |
| 订阅生命周期 | `test_subscription_contract_harness` | `wasm_subscription_contract_harness` |
| 配置交换 | `test_config_exchange_contract_harness` | `wasm_config_exchange_contract_harness` |

测试位于 `crates/rssr-infra/tests/`，以 `RefreshService`、`SubscriptionWorkflow`、`ImportExportService` 为应用入口。Native fixture 使用真实 SQLite 仓储与迁移；browser fixture 使用 infra 的 `BrowserStore` 和浏览器仓储，隔离与存储辅助代码见 `tests/support/browser_storage.rs`（相对于 rssr-infra）。远程配置传输可使用内存 fake；页面、session、DOM selector 不属于这些测试的契约。

## 已覆盖的行为与边界

- 刷新：目标查询、Updated / NotModified / Failed、feed 元信息、文章写入、错误清理与失败保留、新增数量及存储提交。
- 浏览器刷新：真实 localStorage 发布、正文/索引的一致性与失败恢复、多实例写入协调；具体边界以当前 harness 的断言为准。
- 订阅：新增、URL 规范化/去重、删除后的文章与 last-opened 状态清理。
- 配置：JSON / OPML 交换、远端 push/pull 结果、订阅成员变化后的状态清理；配置不承载文章库。
- source 分类：有效 XML、HTML/login shell、损坏 XML、304、非成功状态及代理/直连 fallback 的分类。请求顺序的纯函数测试与 store-side 持久化测试各守自己的边界。

真实网络、CORS 和部署态 `/feed-proxy` 仍需 [代理 smoke](./rssr-web-proxy-feed-smoke.md) 与 [浏览器 feed smoke](./rssr-web-browser-feed-smoke.md)。同源 fixture 通过不能证明远端站点策略正常；`--no-run` 通过也不能证明浏览器契约通过。

## 执行

```bash
cargo test --locked -p rssr-infra --test test_refresh_contract_harness --test test_subscription_contract_harness --test test_config_exchange_contract_harness
bash scripts/run_wasm_contract_harness.sh wasm_refresh_contract_harness wasm_subscription_contract_harness wasm_config_exchange_contract_harness
```

浏览器入口需要锁文件匹配的 wasm-bindgen runner、Chrome 与 ChromeDriver；工具准备见 [主线验证矩阵](./mainline-validation-matrix.md)。原单模块脚本继续兼容，但不使用会无差别编译全部 integration tests 的 `wasm-pack test`。

统一脚本由 std-only Rust runner 获取 Cargo 返回的精确产物，每次浏览器执行使用独立临时配置和 profile。默认测试超时 60 秒、driver 启动超时 15 秒；显式传入的超时变量保留。

CI 分离构建与运行：`--prepare DIR <harness...>` 在完整产物就绪后发布 manifest；`--prebuilt DIR <harness>` 只运行相应产物。残缺或重复产物应拒绝，不按 mtime 猜测目标；不能将 prepare 成功写成测试通过。

## 维护要求

围绕新语义或已复现的失败增补断言，避免测试页面实现细节。存储变化同时检查索引/正文、部分失败、重试、并发写入和清理；共享用例变化分别运行相关 native 与 wasm harness。环境失败必须保留错误及复验结果，见 [环境限制](./environment-limitations.md)。执行数量和结论写入本次 handoff，不把某次运行的数字当作固定通过门槛。

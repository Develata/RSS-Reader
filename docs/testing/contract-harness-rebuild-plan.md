# Contract harness：当前基线与维护

核对：2026-10-22，v0.2 阶段 D。

## 共享 SQL 契约

原生与 Pages 用同一份 SQL 仓储、迁移器和测试源码。测试通过 `cfg_attr` 分别在 native Tokio 和真实 Chrome 的 wasm-bindgen-test 中运行；浏览器构建由 `memory-store` 启用。正式 Web 的 HttpBackend 另用服务器契约及真实 UI 验收。

`crates/rssr-infra/Cargo.toml` 的 `package.metadata.rssr.wasm-contracts` 列出完整浏览器矩阵：订阅、配置交换、刷新、I/P/D 序号、查询与导航、Unicode 搜索、设置、原子性、OPML 和执行器。各 target 在两端同名；`wasm_memory_store` 只测浏览器执行器。

旧三个 harness、BrowserStore 与 Web Locks/localStorage 存储层已删除。69 项旧测试的迁移或退役理由见 [逐项对照](phase-d-browser-contract-migration.md)。旧存储配额、双槽、缓存 epoch 不属于 SQLite 的契约，不报告为“迁移通过”。

`tests/support/library.rs` 用真实内存 SQLite 和产品端口；原生文件锁、跨进程与磁盘故障仍只在原生执行。网络传输可用受控服务或内存 fake；页面、session、DOM selector 不属于仓储测试。

## 执行

```bash
cargo test --locked -p rssr-infra --test test_library_refresh_contract_harness --test test_library_subscription_contract_harness --test test_library_config_exchange_contract_harness
bash scripts/run_wasm_contract_harness.sh test_library_refresh_contract_harness test_library_subscription_contract_harness test_library_config_exchange_contract_harness
bash scripts/run_wasm_contract_harness.sh test_library_contracts test_library_refresh_tickets wasm_memory_store
```

浏览器入口需要锁文件匹配的 wasm-bindgen runner、Chrome、ChromeDriver，以及支持 wasm 的 clang / llvm-ar。原单模块 shell 入口兼容转发到共享 SQL target。工具准备见 [主线矩阵](mainline-validation-matrix.md)。

统一脚本由 std-only Rust runner 获取 Cargo 返回的精确产物，每次浏览器运行使用独立配置/profile，默认测试超时 60 秒、driver 启动超时 15 秒；显式超时变量保留。

CI 用 `--prepare DIR <harness...>` 编译一次、完整后发布 manifest；再用 `--prebuilt DIR <harness>` 在独立 runner 执行。残缺或重复产物拒绝，不按 mtime 猜目标；prepare 成功不等于测试通过。

## 边界与维护

索引/正文、原子性、重试、并发写入和清理的变更分别运行 native 与 wasm 的同一案例。WASM 记录实际 SQLite 版本及 source id；原生文件恢复证据不能由同页内存连接替代。

正式 Web 的登录与抓取链路用 [feed smoke](rssr-web-browser-feed-smoke.md)，多浏览器可见性与恢复用 [服务器验收](server-web-regression.md)。Pages 的真实 RSS 内容、重载重置、子路径及性能另用 `scripts/test_pages.sh`。模拟服务器通过不能冒充实际 GitHub Pages 部署。

本次运行命令、实际结果及缺口写入 handoff，不将旧数量当成固定门槛。

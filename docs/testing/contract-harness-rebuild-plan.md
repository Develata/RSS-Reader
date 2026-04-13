# contract harness 重建计划

这份文档用于指导把 `zheye-mainline-stabilization` 中有价值但已不适合直接移植的 3 份 contract harness 测试，按当前 `main` 的结构重建到主线。

## 目标

重建这 3 份测试：

- `refresh contract harness`
- `config exchange contract harness`
- `subscription contract harness`

重建后的目标不是复刻旧分支文件，而是验证**同一套 application contract** 在当前主线的两类实现上保持行为一致：

- SQLite / native adapter
- browser persisted-state adapter

## 为什么不能直接抄旧分支

旧分支中的 harness 建立在较早的结构之上，当前 `main` 已有这些关键变化：

- Web/browser backend 已正式外移到 `rssr-infra/src/application_adapters/browser/`
- `rssr-app/src/bootstrap/web/*` 已明显变薄，不再适合作为测试锚点
- `native / cli / web` 都已经接到新的共享 use case
- 页面层和 session 结构继续演进，不应被测试文件反向绑定

因此，旧分支中的这些文件只能作为参考：

- `crates/rssr-infra/tests/test_refresh_contract_harness.rs`
- `crates/rssr-infra/tests/test_config_exchange_contract_harness.rs`
- `crates/rssr-infra/tests/test_subscription_contract_harness.rs`

## 当前主线可复用的落点

### application 层

- `crates/rssr-application/src/refresh_service.rs`
- `crates/rssr-application/src/subscription_workflow.rs`
- `crates/rssr-application/src/import_export_service.rs`

### native / sqlite 适配

- `crates/rssr-infra/src/application_adapters/refresh.rs`
- `crates/rssr-infra/src/application_adapters/non_refresh.rs`
- `crates/rssr-infra/src/db/*`

### browser 适配

- `crates/rssr-infra/src/application_adapters/browser/adapters.rs`
- `crates/rssr-infra/src/application_adapters/browser/query.rs`
- `crates/rssr-infra/src/application_adapters/browser/state.rs`
- `crates/rssr-infra/src/application_adapters/browser/config.rs`
- `crates/rssr-infra/src/application_adapters/browser/feed.rs`

### 已有测试基线

- `crates/rssr-infra/tests/test_application_refresh_store_adapter.rs`
- `crates/rssr-infra/tests/test_feed_refresh_flow.rs`
- `crates/rssr-infra/tests/test_config_package_io.rs`
- `crates/rssr-infra/tests/test_settings_repository.rs`
- `crates/rssr-infra/tests/test_webdav_local_roundtrip.rs`

## 重建原则

### 1. 只测 contract，不测页面

新的 harness 必须围绕 application contract 组织，不能重新依赖：

- `rssr-app` 页面组件
- `bootstrap/web.rs`
- session / bindings / presenter

### 2. 一个行为，两种 fixture

每份 harness 都应该同时跑：

- SQLite fixture
- browser-state fixture

并在同一断言语义下验证：

- 输入
- 副作用
- 输出

### 3. browser fixture 应直接基于当前 browser adapter

不要再复制旧分支里那套临时 browser model。优先基于当前主线已有实现：

- `PersistedState`
- `BrowserFeedRepository`
- `BrowserEntryRepository`
- `BrowserSettingsRepository`
- `BrowserAppStateAdapter`
- `BrowserOpmlCodec`

### 4. 避免把所有场景塞进一份超大测试

每个 harness 内应继续拆分多个 focused test，而不是一个巨型 end-to-end 脚本。

### 5. 接受 host / wasm 分层，而不是强行一份测试吃掉所有 target

当前主线里：

- SQLite / native adapter 主要在 `cfg(not(target_arch = "wasm32"))`
- browser adapter 主要在 `cfg(target_arch = "wasm32")`

这意味着旧分支里那种“同一份 host 测试同时跑 sqlite fixture 与 browser fixture”的结构，当前主线不能直接复刻。

更合理的重建方式是：

- host harness：
  - 先保证共享 use case + sqlite adapter 的 contract 基线
- wasm/browser harness：
  - 单独为 browser fixture 设计 target-specific 测试基座

不要为了追求形式上的“一个文件测完全部实现”而去回退当前主线的模块边界。

## 分阶段重建顺序

### 阶段 1：refresh contract harness

优先级最高。

原因：

- `RefreshService` 是最核心的共享 use case 之一
- 已有 `test_application_refresh_store_adapter.rs` 可作为起点
- refresh 行为最适合同时验证 sqlite / browser-state

建议新文件：

- `crates/rssr-infra/tests/test_refresh_contract_harness.rs`

最小覆盖：

- 更新 feed metadata
- 新文章写入
- `NotModified` 分支
- refresh failure 写回
- `refresh_all` 顺序与结果聚合

当前进度：

- 已完成 host / sqlite baseline
- browser / wasm baseline 已完成

### 阶段 2：subscription contract harness

建议新文件：

- `crates/rssr-infra/tests/test_subscription_contract_harness.rs`

最小覆盖：

- 新增订阅
- URL 规范化后的去重
- 删除订阅时的软删除
- 删除后 entry 清理
- `last_opened_feed_id` 清理

当前进度：

- 已完成 host / sqlite baseline
- browser fixture 尚未开始

### 阶段 3：config exchange contract harness

建议新文件：

- `crates/rssr-infra/tests/test_config_exchange_contract_harness.rs`

最小覆盖：

- JSON export/import
- OPML export/import
- remote push/pull
- 删除订阅后的配置同步
- browser fixture 下 settings 与 last-opened state 的保持/清理

当前进度：

- 已完成 host / sqlite baseline
- browser / wasm baseline 已完成

## 每阶段的建议实现方式

### refresh harness

- SQLite fixture：
  - 继续复用内存 sqlite + migrate
- browser fixture：
  - 直接基于 `PersistedState`
  - 用当前 `Browser*` adapters 组装 `RefreshStorePort`
  - 但实现形式应拆成单独的 wasm/browser harness，而不是和 host harness 强塞到同一个 test target

当前入口设计：

- host baseline：
  - `cargo test -p rssr-infra --test test_refresh_contract_harness`
- wasm/browser harness 编译入口：
  - `cargo test -p rssr-infra --target wasm32-unknown-unknown --test wasm_refresh_contract_harness --no-run`
- 后续 browser 实际执行入口建议：
  - `bash scripts/setup_chrome_for_testing.sh`
  - `bash scripts/run_wasm_refresh_contract_harness.sh`

当前最小 browser 基座建议先覆盖：

- `BrowserRefreshStore::list_targets`
- `BrowserRefreshStore::get_target`
- `BrowserRefreshStore::commit(NotModified)`
- `BrowserRefreshStore::commit(Updated)` 清理旧 `fetch_error`
- `BrowserRefreshStore::commit(Failed)` 保留既有 `last_success_at`
- browser localStorage 写回是否成功

### refresh source-side harness 设计

当前 `BrowserFeedRefreshSource` 已有第一批 body classification contract，但仍没有完整网络请求级 harness。

这里的 source-side 只指：

- 请求顺序
- fallback 判定
- HTML shell / login shell 识别
- 非成功状态映射
- XML 解析失败映射

不包含：

- browser state 写回
- `last_fetched_at / last_success_at / fetch_error` 更新
- entries upsert

最小覆盖集建议固定为 4 类：

- network / CORS failure
- proxy shell / login shell
- non-success status
- parse failure

其中：

- request 顺序与 fallback 规则，优先继续落在 `feed_request.rs` / `feed_response.rs` 的纯函数测试
- source outcome 映射，优先围绕 `BrowserFeedRefreshSource` 的 `Failed / NotModified / Updated` 分类做 wasm/browser harness
- `/feed-proxy` 部署壳是否返回真实 XML，继续由 `run_rssr_web_proxy_feed_smoke.sh` 兜住

当前进度：

- 已完成：
  - valid XML body -> `Updated`
  - HTML shell body -> `Failed`，message 前缀为 `解析订阅失败:`
- 待实现：
  - network / CORS failure
  - proxy shell / login shell 的 request-level fallback
  - non-success status -> `Failed`
  - bad XML parse failure

当前不建议为了覆盖 source-side 去做的事：

- 不为 `BrowserFeedRefreshSource` 专门引入 host-only mock 结构
- 不把 source-side harness 绑到 `rssr-app` 页面或 `data-action`
- 不把 `/feed-proxy` 部署壳问题和 store-side 持久化问题混进同一份断言

建议下一步实现顺序：

1. 先把 source-side 最小覆盖集写成设计清单并固定文档入口
2. 再判断是否需要新增“response classification helper”来降低 wasm harness 复杂度
3. 最后再补真正的 source-side harness

### subscription harness

- SQLite fixture：
  - 复用 `SqliteAppStateAdapter`
- browser fixture：
  - 直接复用 `BrowserFeedRepository`
  - `BrowserEntryRepository`
  - `BrowserAppStateAdapter`

### config exchange harness

- SQLite fixture：
  - 复用 `SqliteSettingsRepository`
  - `SqliteAppStateAdapter`
  - `InfraOpmlCodec`
- browser fixture：
  - 复用 `BrowserSettingsRepository`
  - `BrowserAppStateAdapter`
  - `BrowserOpmlCodec`
  - 为 `RemoteConfigStore` 继续使用内存 fake 即可

## 明确不要做的事

- 不把旧分支的测试文件直接复制进当前主线
- 不在 harness 里直接依赖 `rssr-app` 页面结构或 `data-action`
- 不把 browser fixture 再实现成一套脱离主线的临时模型，除非当前 adapter 无法支持断言
- 不把 `test_webdav_local_roundtrip` 的环境限制混进 contract harness 本体
- 不为了 source-side harness 回退当前 `wasm32` 专属 browser adapter 边界

## 建议验收

每一阶段完成后，至少执行：

- `cargo fmt --all`
- `cargo check -p rssr-infra`
- `cargo test -p rssr-infra --test <对应 harness 文件>`
- `git diff --check`

阶段 3 额外建议：

- `cargo test -p rssr-infra --test test_webdav_local_roundtrip`

## 当前执行入口

### host / sqlite refresh contract harness

- `cargo test -p rssr-infra --test test_refresh_contract_harness`

### wasm / browser refresh contract harness

- `bash scripts/setup_chrome_for_testing.sh`
- `bash scripts/run_wasm_contract_harness.sh wasm_refresh_contract_harness`
- `bash scripts/run_wasm_refresh_contract_harness.sh`

### wasm / browser subscription contract harness

- `bash scripts/setup_chrome_for_testing.sh`
- `bash scripts/run_wasm_contract_harness.sh wasm_subscription_contract_harness`
- `bash scripts/run_wasm_subscription_contract_harness.sh`

### wasm / browser config exchange contract harness

- `bash scripts/setup_chrome_for_testing.sh`
- `bash scripts/run_wasm_contract_harness.sh wasm_config_exchange_contract_harness`
- `bash scripts/run_wasm_config_exchange_contract_harness.sh`

当前仓库不再使用：

- `wasm-pack test --headless --chrome crates/rssr-infra ...`

原因：

- `rssr-infra/tests/` 中仍包含大量 native-only integration tests
- `wasm-pack test` 会把整 crate 的 integration tests 一起拖进 wasm target
- 正确入口应只编译并执行 `wasm_refresh_contract_harness` 单个 `.wasm` 产物

## 推荐执行顺序

1. 先完成 refresh harness 的 host / sqlite baseline
2. 再补 browser / wasm harness 的测试基座
3. 之后按同样模式推进 subscription harness
4. 最后推进 config exchange harness

## 当前阶段判断

截至当前主线：

- `refresh contract harness` 已有 host / sqlite baseline 与 browser / wasm baseline
- `refresh store-side` 的 browser contract 已覆盖 target lookup、successful commit、failed commit 与 localStorage 写回
- `refresh source-side` 的 failure triage 与契约说明已补齐，第一批 body classification harness 已落地，request-level harness 仍待实现
- `subscription contract harness` 已有 host / sqlite baseline 与 browser / wasm baseline
- `config exchange contract harness` 已有 host / sqlite baseline 与 browser / wasm baseline

## 当前结论

`zheye-mainline-stabilization` 的功能代码现在基本都已被当前 `main` 覆盖；剩下最值得持续吸收的工程资产，是这 3 份 contract harness 的测试思想，而不是它们的原始文件本身。

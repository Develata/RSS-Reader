# 主线验证矩阵

这份文档用于把当前主线的自动化测试、contract harness、环境限制和手工 smoke test 收拢成一套长期维护的验证流程。

它回答 3 个问题：

- 当前主线每个核心能力应该看哪些自动化入口
- 哪些能力即使自动化通过，仍然必须做手工 smoke
- 某项失败时，应该先判断为代码回归，还是环境限制

## 使用方式

建议按下面顺序执行主线验证：

1. 跑自动化基础口
2. 对照本矩阵判断是否还有必做 smoke
3. 遇到失败时，先查 [环境限制索引](./environment-limitations.md)
4. 将本次验证结果回写到“最近一次验证来源”

## 自动化基础口

- `cargo test --workspace`
- `cargo check -p rssr-app --target wasm32-unknown-unknown`
- `cargo test -p rssr-web`

说明：

- 如果 `cargo test --workspace` 只剩 `test_webdav_local_roundtrip` 在受限环境中失败，不应直接判定为功能回归；先看 [环境限制索引](./environment-limitations.md)

## 主线最小验证矩阵

| 能力项 | 自动化入口 | 是否需要手工 smoke | 是否受环境限制 | 通过标准 | 最近一次验证来源 |
|---|---|---|---|---|---|
| add/remove | `cargo test -p rssr-application`；`cargo test -p rssr-infra --test test_subscription_contract_harness` | 是。Desktop / Web / CLI 至少各验证 1 次 add/remove 链路 | 低 | URL 可添加；软删除后重激活正确；删除后列表与 app state 保持一致；无异常报错 | `TODO: YYYY-MM-DD / doc or handoff` |
| refresh | `cargo test -p rssr-application`；`cargo test -p rssr-infra --test test_refresh_contract_harness`；`cargo test -p rssr-infra --test test_application_refresh_store_adapter`；`cargo test -p rssr-infra --test test_feed_refresh_flow` | 是。至少验证 single refresh 和 refresh all 各 1 次 | 中。Web 端受 feed 可达性、CORS 与代理模式影响 | single / all 都可完成；成功、失败、not modified 语义与状态写回正确；不产生异常重复写入 | `TODO: YYYY-MM-DD / doc or handoff` |
| config/exchange | `cargo test -p rssr-application`；`cargo test -p rssr-infra --test test_config_exchange_contract_harness`；`cargo test -p rssr-infra --test test_config_package_codec`；`cargo test -p rssr-infra --test test_config_package_io`；`cargo test -p rssr-infra --test test_opml_interop` | 是。至少做 1 轮 JSON 导入导出与 1 轮 OPML 导入导出 | 中。WebDAV 联通受网络与本地端口环境影响 | JSON / OPML roundtrip 可用；损坏或非法配置被拒绝；导入后订阅与设置恢复符合预期 | `TODO: YYYY-MM-DD / doc or handoff` |
| reader rendering | `cargo test -p rssr-app`；`cargo test -p rssr-infra --test test_feed_parse_dedup` | 是。Desktop / Web 各打开 1 篇 HTML-heavy 文章 | 中。真实渲染仍要以 UI 行为确认 | 阅读页优先展示完整 HTML；summary fallback 中的 HTML-like 内容不会原样显示标签；内容经过清洗 | `TODO: YYYY-MM-DD / doc or handoff` |
| web startup | `cargo check -p rssr-app --target wasm32-unknown-unknown`；`cargo test -p rssr-app`；`cargo test -p rssr-web` | 是。`dx serve --platform web --package rssr-app` 必做 | 中。纯静态 Web 与 `rssr-web` 代理模式边界不同 | 首屏 5 秒内可交互；无黑屏、无“页面无响应”；路由切换正常；Console 无新的 panic / unreachable / 死循环迹象 | `TODO: YYYY-MM-DD / doc or handoff` |
| paste/input | 当前无专门自动化入口；由 `cargo test -p rssr-app` 提供基础兜底 | 是。Desktop 至少验证新增订阅输入框粘贴；再抽查 1 个设置文本输入框 | 中。宿主输入链路可能有平台差异 | 新增订阅输入框可聚焦、可粘贴、可提交；至少另一个文本输入也无明显粘贴阻断 | `TODO: YYYY-MM-DD / doc or handoff` |
| settings validation | `cargo test -p rssr-infra --test test_config_package_codec`；`cargo test -p rssr-infra --test test_config_exchange_contract_harness`；`cargo test -p rssr-infra --test test_settings_repository` | 是。UI 或 CLI 至少做 1 次非法值保存验证 | 低 | 非法边界值被拒绝；合法值可保存并持久化；刷新或重启后仍保持 | `TODO: YYYY-MM-DD / doc or handoff` |
| remote pull cleanup | `cargo test -p rssr-application`；`cargo test -p rssr-infra --test test_config_exchange_contract_harness`；`cargo test -p rssr-infra --test test_webdav_local_roundtrip` | 是。至少做 1 次真实 WebDAV pull smoke | 高。受 loopback 端口、网络和 WebDAV 端点环境影响 | 远端删除的 feed 会从本地移除；相关 entries 被清理；`last_opened_feed_id` 等 app state 被清理 | `TODO: YYYY-MM-DD / doc or handoff` |

## 推荐的主线验证顺序

### 1. 自动化基础口

- `cargo test --workspace`
- `cargo check -p rssr-app --target wasm32-unknown-unknown`
- `cargo test -p rssr-web`

### 2. 平台最小 smoke

- Desktop：
  - add/remove
  - refresh
  - reader rendering
  - paste/input
  - settings validation
- Web：
  - web startup
  - add/remove
  - refresh
  - reader rendering
  - config/exchange
- CLI：
  - add/remove
  - refresh
  - config/exchange
  - settings validation
  - 如具备环境，再做 remote pull cleanup

### 3. 结果记录

建议每次主线验证至少记录：

- 日期
- 执行环境
- 自动化基础口结果
- env-limited 项
- Desktop / Web / CLI smoke 结果
- 关联文档或 handoff

可使用下面的占位格式回写：

```text
最近一次验证来源：
- 日期：YYYY-MM-DD
- 环境：macOS / WSL / Chrome / loopback allowed?
- 自动化：pass / pass with env-limited item / fail
- smoke：desktop pass / web pass / cli pass / partial
- 记录：docs/... or docs/handoffs/...
```

## 参考记录

- 浏览器全链路回归： [global-browser-regression.md](./global-browser-regression.md)
- 手工回归基线： [manual-regression.md](./manual-regression.md)
- 桌面最终验收： [manual/final-acceptance-checklist.md](./manual/final-acceptance-checklist.md)
- 全量验证记录： [../handoffs/2026-04-08-full-test-verification.md](../handoffs/2026-04-08-full-test-verification.md)
- hotfix 后验证记录： [../handoffs/2026-04-08-hotfix-html-paste-web-startup.md](../handoffs/2026-04-08-hotfix-html-paste-web-startup.md)

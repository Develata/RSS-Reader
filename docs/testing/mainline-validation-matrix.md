# 主线验证矩阵

这份文档把当前主线的自动化测试、手工 smoke、环境限制判断口径收拢成一套长期维护的验证流程。

它回答 3 个问题：

- 当前主线每个核心能力应该看哪些自动化入口
- 哪些能力即使自动化通过，仍然必须做手工 smoke
- 某项失败时，应该先判断为代码回归，还是环境限制

## 使用方式

建议按下面顺序执行主线验证：

1. 跑自动化基础口
2. 对照本矩阵判断是否还有必做 smoke
3. 遇到失败时，先查 [环境限制索引](./environment-limitations.md)
4. 将本次验证结果回写到相应 handoff

## 自动化基础口

- `cargo test --workspace`
- `cargo check -p rssr-app --target wasm32-unknown-unknown`
- `cargo check -p rssr-app --target aarch64-linux-android`
- `cargo test -p rssr-web`

说明：

- 如果 `cargo test --workspace` 只剩 `test_webdav_local_roundtrip` 在受限环境中失败，不应直接判定为功能回归；先查 [环境限制索引](./environment-limitations.md)

## 主线最小验证矩阵

| 能力项 | 自动化入口 | 是否需要手工 smoke | 是否受环境限制 | 通过标准 |
|---|---|---|---|---|
| add/remove | `cargo test -p rssr-application`；`cargo test -p rssr-infra --test test_application_refresh_store_adapter`；`cargo check -p rssr-cli` | 是 | 低 | URL 可添加；删除后列表与 app state 保持一致；无异常报错 |
| refresh | `cargo test -p rssr-application`；`cargo test -p rssr-infra --test test_feed_refresh_flow`；`cargo test -p rssr-infra --test test_application_refresh_store_adapter` | 是 | 中 | single / all 都可完成；成功、失败、not modified 语义正确；不产生异常重复写入 |
| config/exchange | `cargo test -p rssr-application`；`cargo test -p rssr-infra --test test_config_package_codec`；`cargo test -p rssr-infra --test test_config_package_io`；`cargo test -p rssr-infra --test test_opml_interop` | 是 | 中 | JSON / OPML roundtrip 可用；损坏或非法配置被拒绝；导入后订阅与设置恢复符合预期 |
| reader rendering | `cargo test -p rssr-app`；`cargo check -p rssr-app --target wasm32-unknown-unknown` | 是 | 中 | 阅读页优先展示完整 HTML；HTML-like fallback 不再被原样显示标签；内容经过清洗 |
| web startup | `cargo check -p rssr-app --target wasm32-unknown-unknown`；`cargo test -p rssr-web` | 是 | 中 | 首屏可交互；无黑屏、无页面无响应；主要路由切换正常；Console 无新的 panic / 死循环 |
| paste/input | 当前无专门自动化入口；由 `cargo test -p rssr-app` 提供基础兜底 | 是 | 中 | 新增订阅输入框可聚焦、可粘贴、可提交；至少另一个设置输入框无明显阻断 |
| settings save | `cargo test -p rssr-infra --test test_settings_repository`；`cargo test -p rssr-infra --test test_config_package_codec` | 是 | 低 | 非法边界值被拒绝；合法值可保存并持久化；刷新或重启后仍保持 |
| remote pull cleanup | `cargo test -p rssr-infra --test test_webdav_local_roundtrip`；`cargo test -p rssr-infra --test test_config_package_io` | 是 | 高 | 远端删除的 feed 会从本地移除；相关 entries 与 app state 被清理 |

## 推荐的主线验证顺序

### 1. 自动化基础口

- `cargo test --workspace`
- `cargo check -p rssr-app --target wasm32-unknown-unknown`
- `cargo check -p rssr-app --target aarch64-linux-android`
- `cargo test -p rssr-web`

### 2. 平台最小 smoke

- Desktop：
  - add/remove
  - refresh
  - reader rendering
  - paste/input
  - settings save
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
  - settings save

### 3. 结果记录

建议每次主线验证至少记录：

- 日期
- 执行环境
- 自动化基础口结果
- `env-limited` 项
- Desktop / Web / CLI smoke 结果
- 关联 handoff 或测试文档

## 参考记录

- [发布前 UI 回归清单](./release-ui-regression-checklist.md)
- [全局浏览器回归报告](./global-browser-regression.md)
- [手工回归测试清单](./manual-regression.md)
- [Headless 重构视觉等价验收](./headless-refactor-equivalence.md)
- [2026-04-08-daily-rollup](/home/develata/gitclone/RSS-Reader/docs/handoffs/2026-04-08-daily-rollup.md)

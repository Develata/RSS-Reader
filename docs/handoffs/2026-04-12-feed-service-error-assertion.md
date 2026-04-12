# Feed Service Error Assertion

- 日期：2026-04-12
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：f7eff7a
- 相关 commit：f7eff7a
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

完成一轮轻量全局复查后，清理 `feed_service` 单测中的错误文案 substring 断言，改为结构化断言底层 URL 解析错误。

## 影响范围

- 模块：
  - `crates/rssr-application/src/feed_service.rs`
- 平台：
  - 测试代码
- 额外影响：
  - N/A

## 关键变更

### String Protocol Cleanup

- `add_subscription_rejects_invalid_urls` 不再使用 `error.to_string().contains("订阅 URL 不合法")`。
- 测试改为断言 `anyhow::Error` 链中可 downcast 到 `url::ParseError`。
- 生产代码的错误 context 文案保持不变，用户可见行为不变。

### Search Result

- 本轮复查后，`crates/*/src` 中已无 `to_string().contains`。
- 其余 `contains(...)` 命中主要是合法的数据集合判断、HTML 内容判断或测试文本断言，不属于 runtime 字符串协议。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo test -p rssr-application feed_service::tests::add_subscription_rejects_invalid_urls`：通过
- `cargo test --workspace`：通过
- `git diff --check`：通过
- `rg "to_string\\(\\)\\.contains" crates/rssr-application/src crates/rssr-app/src crates/rssr-infra/src -n`：无命中
- release UI regression：未执行；本次只改测试断言，不改生产代码或 UI 路径

### 手工验收

- 未执行；本次为测试断言清理。

## 结果

- 本次交付可合并；测试不再依赖错误文案 substring 判断。
- 用户可见行为无变化。

## 风险与后续事项

- `crates/rssr-infra/tests/test_config_package_codec.rs` 仍有错误文案 substring 断言，但它们位于契约/编码测试中，用于确认用户可见诊断文案；是否改成结构化错误需要先调整 codec 错误模型。
- 后续如继续清理，应优先区分“测试用户文案”与“运行时控制流字符串协议”。

## 给下一位 Agent 的备注

- 阅读入口：`crates/rssr-application/src/feed_service.rs`
- 当前 runtime 代码中未发现 `to_string().contains` 字符串协议。

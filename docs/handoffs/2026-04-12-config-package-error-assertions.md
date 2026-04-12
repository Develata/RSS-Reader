# Config Package Error Assertions Cleanup

- 日期：2026-04-12
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：c618417
- 相关 commit：c618417
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

收敛配置包 codec 测试中的重复错误消息断言，保留当前用户可见诊断文本契约，避免把一次低风险测试整理扩大成错误模型重构。

## 影响范围

- 模块：
  - `crates/rssr-infra/tests/test_config_package_codec.rs`
- 平台：
  - Linux
  - Web / desktop / Android 运行时代码未变更
- 额外影响：
  - tests

## 关键变更

### Config Package Codec Tests

- 新增 `assert_decode_error_contains` 测试 helper，集中表达“decode 应失败且诊断消息包含指定文本”的断言。
- 替换重复的 `error.to_string().contains(...)` 测试片段，减少后续维护分散点。
- 未引入结构化 config package codec error model；当前断言仍保护导入失败时的用户可见诊断文本。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo test -p rssr-infra --test test_config_package_codec`：通过，7 个测试通过
- `cargo test --workspace`：通过
- `git diff --check`：通过

### 手工验收

- release UI regression：未执行；本次只改 infra 测试 helper，未修改生产代码、UI、runtime 或 workflow。

## 结果

- 本次测试清理已验证，可合并。
- 用户可见配置包导入诊断行为未改变。

## 风险与后续事项

- 这些测试仍依赖错误消息片段，因为配置包导入错误当前没有独立的结构化错误枚举。
- 如果后续目标是完全消除字符串错误断言，应先设计 `decode_config_package` 的 typed error model，再迁移测试。

## 给下一位 Agent 的备注

- 入口文件是 `crates/rssr-infra/tests/test_config_package_codec.rs`。
- 若继续推进错误模型收敛，先审查 `crates/rssr-infra/src/config_package.rs` 的 decode 失败路径和 UI 展示路径，避免只为测试整洁而改变用户可见诊断。

# Browser State Seed Warning Cleanup

- 日期：2026-04-12
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：dde9f9c
- 相关 commit：dde9f9c
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

清理 `test_browser_state_seed_contracts` 里的低价值 `dead_code` warning，保留 fixture 对完整 browser persisted contract 的镜像语义，不改生产代码。

## 影响范围

- 模块：
  - `crates/rssr-infra/tests/test_browser_state_seed_contracts.rs`
- 平台：
  - Linux 验证环境
  - Web persisted state contract test
- 额外影响：
  - 无

## 关键变更

### Fixture Contract Intent

- 为 `FixturePersistedFeed`、`FixturePersistedEntry`、`FixturePersistedEntryFlag` 增加 `#[allow(dead_code)]`。
- 增加简短说明，明确这些 struct 用于镜像完整持久化合同，不要求每个测试断言都读取全部字段。

### 行为保持

- 未修改测试输入 fixture、断言逻辑或生产模块。
- 仅消除 `clippy`/编译器对未直接读取字段的噪音告警。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo clippy --workspace --all-targets`：通过
- `cargo test -p rssr-infra --test test_browser_state_seed_contracts`：通过
- `git diff --check`：通过

### 手工验收

- 未执行；本次仅涉及测试代码 warning 清理，无用户可见行为变化。

## 结果

- 本次交付可合并；workspace `clippy` 不再被该测试的 fixture warning 污染。
- 当前剩余风险未扩大到运行时或跨端行为。

## 风险与后续事项

- 这次清理的是测试噪音，不是结构性架构变更。
- 如果后续继续做 repo 级卫生清理，可再审查是否还有类似“完整合同 fixture 但局部断言”的测试文件需要同样标注。

## 给下一位 Agent 的备注

- 阅读入口：`crates/rssr-infra/tests/test_browser_state_seed_contracts.rs`。
- 若继续做代码卫生清理，建议优先找 `clippy` 或 `cargo test` 输出里的低信噪比 warning，而不是重新扫一遍已干净的 application 收敛路径。

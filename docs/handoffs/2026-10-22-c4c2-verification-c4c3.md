# C4c-2 第二轮修复复验与 C4c-3 文档

- 日期：2026-10-22
- 作者 / Agent：Codex
- 分支：v0.2-architecture
- 当前 HEAD：开始于 `b417653`
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`draft`（复验与文档进行中；第三轮独立评审留给后续，停在 C6 之前）

## 工作摘要

承接 `target/handoff/2026-10-21-c4c2-finish-c4c3.md`。先验证 R2-01 的变异能被原生与浏览器捕获，再在独立干净检出完成 Windows / WSL 验证并更新正式 Web 文档。本轮不运行独立评审，不推送、不打 tag、不发布。

## 影响范围

- 模块：`scripts/release-ui/plan.rs`；正式 Web 的说明文档与测试入口。
- 平台：Windows 原生/wasm/Android 编译、Windows Chrome、WSL Debian。
- 工作树：工作在 `target/audit-20261009/worktree`；完整验证在 detached `target/audit-20261009/b2-baseline`。主检出的任务外改动保持原样。

## 关键变更

### 原生契约门禁目标名

核对文档命令时发现 release UI runner 的 `host-contracts` 仍指定改名前的三个目标。干净检出执行 `cargo test -p rssr-infra --locked -j 2 --test test_refresh_contract_harness --no-run` 退出 101（外层命令见运行记录），报 `no test target named`；当前目标实际为 `test_library_refresh_contract_harness`、`test_library_subscription_contract_harness`、`test_library_config_exchange_contract_harness`。只更新三个参数，不改执行阶段或产品行为。

### C4c-3 文档

进行中，待最终验证后填写文件清单与检查结果。

## 验证与验收

- R2-01 变异（`b417653`）：`drafts::follow` 忽略 `keep` 错误并返回 `Ok`。n5 原生两个目标测试在行为断言失败（exit 101；另一个匹配 `later_edits` 的测试通过）；m9 真实浏览器 `later-edit-not-kept-stays` 的三个行为断言失败（exit 1，14 秒），构建均成功，意外控制台错误为零。每次变异后还原，产品源码无持久改动。日志与 patch：`target/handoff/evidence-c4c2-r2/mutations/`。
- 首次干净检出 `b417653`（tree `ffc21f5c484ad2a50b20fe73a0508414fb6db981`）：Windows rustc 1.99.0 的 fmt、workspace clippy、四组 wasm clippy、Android check 均 exit 0；workspace tests 56 组，770 passed / 0 failed / 2 ignored。
- 首轮浏览器尚未进入断言，服务器构建报 `cargo: command not found`。本机同期正在 Scoop/Rust 升级，`rustup/current` 暂时不存在，观察到 `rustup-init.exe`；单纯补 PATH 的一次重试仍失败。两轮日志均保留。升级结束后版本为 Rust/Cargo 1.99.1，需在该稳定工具状态重跑完整验证。
- 本机 dx 0.7.10 与项目 Dioxus 0.7.9 不同，dx 发出版本提示但页面构建 exit 0；不把本地结果等同于 CI 锁定 CLI 0.7.9 的验证。
- 独立 runner：`cargo fmt --manifest-path scripts/release-ui/Cargo.toml --check`、`cargo test --manifest-path scripts/release-ui/Cargo.toml --locked -j 2 --target-dir target/release-ui-runner`、对应 `clippy --all-targets -- -D warnings` 均 exit 0；具体测试数量见 `target/handoff/evidence-c4c2-r2-final/runner-fix/tests.log`。最终干净检出与真实三个目标的执行待后续补齐。
- 人工页面与实机验收：未执行。

## 结果

当前 commit: pending。原生契约门禁修正后先单独提交；验证记录与 C4c-3 文档分别提交。所有完整验证均标注实际 commit/tree，不把旧工作区的通过当作新检出的证据。

## 风险与后续事项

- 第三轮独立评审、Ubuntu CI、C6 发布标识与旧页面对新服务器验收未运行。
- WSL 与 Windows 构建串行，所有本轮 Cargo 构建使用两任务上限；不更改全局工具，不清理他人进程或数据。

## 给下一位 Agent 的备注

- 汇总与辅助脚本在 `target/handoff/`，被 Git 忽略；最终命令/退出码/证据路径见 `report-c4c2-c4c3.md`（待完成）。
- 原 C4 历史记录：`docs/handoffs/2026-10-19-v0.2-phase-c4.md`；本轮只补实际结果，不重写旧失败事实。

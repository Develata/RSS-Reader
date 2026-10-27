# C4c CI 覆盖补齐

- 日期：2026-10-22
- 作者 / Agent：Codex
- 分支：`v0.2-architecture`
- 当前实现 HEAD：`cedaad12248e1266ab0e6e16e99abf4d104a1c68`
- 相关 commit：`cedaad1`（workflow、目标检查与操作文档）；本交接随后的 docs commit 可用 `git log -- docs/handoffs/2026-10-22-c4c-ci-coverage.md` 查询
- 相关 tag / release：N/A
- 状态：`validated`（本地配置与工具验证通过；远端 GitHub CI 未运行）

后续：用户随后明确授权推送并运行 CI；本记录保留本地交付时的事实，远端结果与新授权边界见 [C4c 远端 CI 记录](./2026-10-22-c4c-remote-ci.md)。

## 工作摘要

在 [C4c-2 补验与 C4c-3 文档交付](./2026-10-22-c4c2-verification-c4c3.md) 后，用户要求同时更新 CI。本次把已有本地验证的编译组合接入 CI，增加完整长验收的手动入口，并防止 release UI 执行计划再次引用失效的 Cargo 测试目标。

## 影响范围

- 模块：`.github/workflows/ci.yml`、`scripts/release-ui/plan.rs` 的测试模块、两份测试操作文档。
- 平台：GitHub Ubuntu / Windows runner；Web wasm 与 Android ARM64 的检查配置。
- 工作树：仅 `E:\gitclone\RSS-Reader\target\audit-20261009\worktree`。主检出仍为 `d9ebe35`，其原有 dirty / untracked 内容未改动。
- 不涉及产品逻辑、依赖升级、发布或 C6；独立复审继续留待后续。

## 关键变更

- `web-smoke` 分别执行四组 `-D warnings` wasm Clippy：app 正式 Web、app `pages-demo`、infra 无额外 feature、infra `browser-store,http-backend --tests`。随后仍单独构建正式 Web，服务器与页面来自同次检出。
- `android-smoke` 的 ARM64 check 同时包含 `rssr-app` 与 `rssr-cli`。
- `workflow_dispatch` 新增默认关闭的布尔输入 `acceptance_long`。打开时仅默认主题的 acceptance 追加 `--long`；push / PR 仍执行 24 个常规场景，手动完整验收对应当前 27 个场景。实际数量与成败以本次 artifact 为准。
- runner 新增 `planned_cargo_tests_exist_in_workspace` 测试，使用真实 workspace 的 `cargo metadata --locked --no-deps` 验证执行计划里的 package 与 `--test` 目标。现有 Ubuntu / Windows `cargo test` 步骤会自动执行它，不依赖黑盒测试的假 Cargo。
- [主线验证矩阵](../testing/mainline-validation-matrix.md) 与 [服务端浏览器回归](../testing/server-web-regression.md) 已同步入口、覆盖和证据边界。Dioxus CLI 仍锁定项目对应的 `0.7.9`。

## 验证与验收

以下检查在 `9d8ab29` 加本次改动的工作树上执行；实现与操作文档随后原样提交为 `cedaad1`。文件 SHA-256 和本轮日志在忽略目录 `target/handoff/evidence-c4c-ci/`。

| 检查 | 结果 |
| --- | --- |
| `actionlint -color`（1.7.12） | 全部 workflow 静态检查 exit 0；Windows 本次没有调用 ShellCheck，后者在 WSL 单独检查 |
| 提取 `ci.yml` 的 25 个 `run` 块，用 WSL ShellCheck 0.10.0 检查 | 25/25 通过；检查适配器声明 Actions 注入的 env 与 Bash `-e -o pipefail`，排除外部文件、重复追加及外部变量类提示 SC1091 / SC2129 / SC2154 |
| Windows rustc 1.99.1：`cargo fmt --manifest-path scripts/release-ui/Cargo.toml --check` | 通过 |
| Windows：`cargo clippy --manifest-path scripts/release-ui/Cargo.toml --all-targets --locked --target-dir target/release-ui-runner -j 2 -- -D warnings` | 通过 |
| Windows：同 manifest / target-dir 的 `cargo test --locked -j 2` | 3 passed / 0 failed |
| WSL Debian rustc 1.97.0：同 runner 的 fmt、Clippy `--all-targets --locked -- -D warnings`、`cargo test --locked` | 全部通过；4 passed / 0 failed（含 Unix 进程清理测试）；目标目录 `/home/deve/rssr-target/release-ui-runner` |
| 目标名变异：把计划中的 refresh 目标暂时恢复成旧 `test_refresh_contract_harness`，仅运行新增测试 | 如预期 exit 101，明确报告缺失该测试目标；逐字节恢复源码后重跑 Windows 全部 3 个测试通过 |
| 执行从 YAML 原样提取的 `web-ui` Shell 块，以记录参数的假脚本承接四种输入 | default / atlas-sidebar 各搭配 long=false / true，共 4/4 通过；只有 default + true 的 acceptance 收到 `--long`，其余调用及共用产物参数正确 |
| `git diff --check` | 通过 |

辅助验证脚本位于忽略目录 `target/handoff/scripts/check-c4c-ci-{wiring.py,mutation.py,runner-wsl.sh}`；主要证据为 `workflow-wiring.json`、`shellcheck.json`、`target-name-mutation.json` 及各平台 runner 日志。参数编排的假脚本不代表浏览器验收。

本次未重跑产品全量测试、真实浏览器或 Android bundle；产品源码未变，新增 CI 编译命令对应前一份交接在干净 `6c8c95c` 已执行的组合。前次本地证据不等于本次 Ubuntu CI 已运行，也不消除本机 dx 0.7.10 与 CI 0.7.9 的已记录差异。

## 结果

- CI 配置、工具回归检查及文档已在本地提交；当前工作树交付后应保持干净。
- 无 push、远端 workflow 触发、部署或发布；仍停在 C6 之前。

## 风险与后续事项

- 合并 / 推送授权后的真实 GitHub run 才能验证 Ubuntu 系统依赖、缓存、artifact 传输、Dioxus 0.7.9 构建及新增组合的整体耗时；`web-smoke` 保留 40 分钟、`web-ui` 保留每主题 15 分钟预算。
- 需要完整服务端验收时，在已更新的远端 workflow 选择 `acceptance_long=true`，检查 `ci-ui-default` 的验收报告及最终 `lint-and-test`。这不代替尚未进行的独立复审。
- [阶段 C 计划](../design/v0.2-phase-c-plan.md) 的 C4 中间不推送安排继续有效。

## 给下一位 Agent 的备注

从 `cedaad1` 的 diff 与本记录开始核对；完整 Windows / WSL 产品和浏览器证据仍以 [前一份交接](./2026-10-22-c4c2-verification-c4c3.md) 为准。runner 的新检查读取真实仓库 metadata，运行测试时须保留产品 workspace 与锁文件。

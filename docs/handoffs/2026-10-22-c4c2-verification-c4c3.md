# C4c-2 第二轮修复复验与 C4c-3 文档

- 日期：2026-10-22
- 作者 / Agent：Codex
- 分支：v0.2-architecture
- 当前 HEAD：本轮开始于 `b417653`；完整验证提交 `6c8c95c`
- 相关 commit：runner 修复 `6c8c95c`；本次验证记录随本提交；C4c-3 commit: pending
- 相关 tag / release：N/A
- 状态：`draft`（C4c-2 补验通过；C4c-3 待文档检查与提交；独立评审待进行）

## 工作摘要

承接 `target/handoff/2026-10-21-c4c2-finish-c4c3.md`。R2-01 的原生与浏览器变异均被行为断言捕获；Windows / WSL 完整验证在独立干净检出通过。核对文档命令时修正 release UI runner 的三个已改名目标。C4c-3 文档已编写，待最终检查与提交。本轮不运行独立评审，停在 C6 之前。

## 影响范围

- 模块：`scripts/release-ui/plan.rs`；正式 Web 的使用、部署、开发与验证文档。
- 平台：Windows 原生/wasm/Android 编译、Windows Chrome 自动化、WSL Debian。
- 工作树：工作在 `target/audit-20261009/worktree`；完整验证在 detached `target/audit-20261009/b2-baseline`。主检出没有执行写入，任务外改动保留。
- 依赖与 workflow：没有新增依赖，没有修改 workflow，没有 push、tag、发布、PR 或远端运行。

## 关键变更

### 原生契约门禁目标名

`cargo test -p rssr-infra --locked -j 2 --test test_refresh_contract_harness --no-run` 实际退出 101：`no test target named`。release UI runner 仍引用三个旧目标；改为 `test_library_refresh_contract_harness`、`test_library_subscription_contract_harness`、`test_library_config_exchange_contract_harness`，不改执行阶段或产品行为。

### C4c-3 文档

已编写，待最终检查与提交后补充文件清单与结果。

## 验证与验收

### 被测来源

完整验证 commit `6c8c95ca1483cfd4127ef875aa1144011ee2086b`，tree `f088dac729e2b654a276b554aa748a3f0f66646c`，检出前后均 clean。页面（dx debug）、服务器、夹具工具来自此树；各浏览器套件服务器日志均为 `dev-6c8c95ca1483`，驱动逐次核对页面的构建标识。变异基线为 `b417653`；到完整验证提交的 `crates/`、`assets/`、`build-support/`、Cargo 清单与锁文件无差异。后续文档提交不改变这组产品/验收源码。

### R2-01 变异

- n5：让 `drafts::follow` 忽略 `keep` 失败。`cargo test -p rssr-app --locked -j 2 --bins later_edits` 编译成功、exit 101：两个目标测试在 `Ok` / `Err` 与“离开已停止”的行为断言失败，另一个匹配测试通过。
- m9：同一变异，以固定 `RSSR_BUILD_ID=c4c2-r2-mutations-b417653` 分别构建页面、服务器与夹具工具。驱动 `--long --only later-edit-not-kept-stays` exit 1，14 秒；停留在同一文档、清除过时草稿、提示草稿未保留三个断言失败；意外控制台错误为 0，被忽略 6 个（debug 热重载 5，场景预期 401 为 1）。
- 每次变异后还原，产品源码无持久改动。证据：`target/handoff/evidence-c4c2-r2/mutations/` 的两个 patch、`commands.log`、`n5-native.log`、`m9-browser/assertions.json` 及构建日志。

### Windows 与 WSL

Windows Rust/Cargo 1.99.1；Node 24.21.0；dx 0.7.10；Android NDK 28.2.13676358，API 34。以下退出码均为 0：

| 命令 | 结果 |
| --- | --- |
| `cargo fmt --all --check` | 通过 |
| `cargo clippy --workspace --all-targets --locked -j 2 -- -D warnings` | 通过 |
| `cargo clippy -p rssr-app --target wasm32-unknown-unknown --locked -j 2 -- -D warnings` | 正式 Web 通过 |
| 上条加 `--features pages-demo` | 演示通过 |
| `cargo clippy -p rssr-infra --target wasm32-unknown-unknown --locked -j 2 -- -D warnings` | 无额外 feature 通过 |
| 上条加 `--features browser-store,http-backend --tests` | 两种适配及测试编译通过，不是 wasm browser harness 执行 |
| `cargo check -p rssr-app -p rssr-cli --target aarch64-linux-android --locked -j 2`（设置 NDK CC/AR/linker） | 编译通过，不是 Android 实机验收 |
| `cargo test --workspace --locked -j 2 --no-fail-fast` | 56 组，770 passed / 0 failed / 2 ignored |
| WSL Debian rustc 1.97.0：workspace clippy 同上 | 通过 |
| WSL：workspace test 同上 | 56 组，784 passed / 0 failed / 3 ignored |

WSL 在 Windows 全部结束后运行；`CARGO_TARGET_DIR=$HOME/rssr-target`，取消代理环境变量。所有 Cargo 构建上限为 2 jobs。原始命令、NDK 路径、退出码、测试结果在 `target/handoff/evidence-c4c2-r2-final/verified-6c8c95c/summary.log`、`wsl-summary.log` 与各阶段日志；脚本为 `target/handoff/scripts/c4c2-r2-final-verify.sh`、`c4c2-r2-final-wsl.sh`。

独立 runner 的 fmt、clippy 与测试均 exit 0，2 passed / 0 failed；证据在 `target/handoff/evidence-c4c2-r2-final/runner-fix/`。完整聚合使用 `bash scripts/run_release_ui_regression.sh --with-fixed-smokes --log-dir target/c4c2-6c8c95c-final`，没有使用 `--skip-automated`，实际执行修正后的三组契约目标。

### 浏览器验收

真实 Windows Chrome、同一干净源码的 debug 页面与服务器。聚合固定套件后运行 `bash scripts/run_server_web_regression.sh --suite acceptance --long --debug --skip-build --port 8131 --log-dir target/c4c2-6c8c95c-final/server-web-acceptance`，再运行同入口的 `--preset atlas-sidebar` regression；全部 exit 0。

| 套件 | 场景 / 检查 | 意外控制台错误 | 被忽略条目（按原因） |
| --- | --- | --- | --- |
| regression | 337 断言 | 0 | debug bundle live-reload endpoint is absent on rssr-web：225；a request the check failed on purpose：3 |
| theme-matrix | 6 断言 | 0 | debug bundle live-reload endpoint is absent on rssr-web：15 |
| feed-smoke | 3 断言 | 0 | debug bundle live-reload endpoint is absent on rssr-web：4 |
| acceptance | 27 场景 / 108 检查 | 0 | debug bundle live-reload endpoint is absent on rssr-web：183；expected by the scenario: 403 for /api/v1/entries/2/read：1；expected by the scenario: 405 for /login：1；a request the scenario failed on purpose：20；expected by the scenario: 401 for /api/v1/settings：1；expected by the scenario: 401 for /^\/api\/v1\//：6 |
| regression-atlas | 177 断言 | 0 | debug bundle live-reload endpoint is absent on rssr-web：77；a request the check failed on purpose：2 |

acceptance 的被忽略条目均为浏览器网络日志；预期回答的 URL、来源页面与状态码逐条记录在 `verified-summary.json` 的 `expected_answers`，同一 host 检查通过。该核对不是第三轮独立评审。每套件的 `assertions.json`、`server.log`、`summary.md` 和截图/DOM 保存在 `target/handoff/evidence-c4c2-r2-final/verified-6c8c95c/server-web-*/`，原始完整产物（含服务器工作目录）仍在干净检出的 `target/c4c2-6c8c95c-final/`。

### 失败与重跑

1. 初次 `b417653`（tree `ffc21f5c484ad2a50b20fe73a0508414fb6db981`）在 Rust 1.99.0 上通过 fmt、全部 clippy、Android check 与 770 个测试；浏览器在构建服务器时尚未进入断言即报 `cargo: command not found`。本机同期另有 Scoop/Rust 升级，观察到 `rustup/current` 暂时消失与 `rustup-init.exe`。补 PATH 的一次重试仍失败；没有修改全局工具或终止他人进程。升级结束后使用 Rust 1.99.1 完整重跑得到本记录结果。
2. 旧 runner 测试目标名的实际失败、单行参数修复与真实门禁复验见上。
3. dx 0.7.10 对项目 Dioxus 0.7.9 发出版本提示，构建 exit 0；本地结果不能等同于 CI 锁定 CLI 0.7.9 的执行。

首轮失败与重试日志保留在 `target/handoff/evidence-c4c2-r2-final/` 根目录及 `browser-retry-1/`；本轮通过结果仅取 `verified-6c8c95c/`，不覆盖旧失败记录。

### 手工验收

没有人工操作页面、人工截图视觉验收或 Android/macOS 实机验收。也没有重跑 Pages 演示浏览器契约、原生目录 `--native-target` 或公网 feed；本轮 wasm 编译与本地服务器夹具不能代替这些验收。

## 结果

C4c-2 第二轮修复的变异与干净检出验证已补齐。独立评审仍待后续；本记录不授予合并或发布结论。C4c-3 文档 commit: pending。

## 风险与后续事项

- 第三轮独立评审由用户后续安排，未调用其他模型进行复审。
- Ubuntu CI、本地 CLI 0.7.9、实际部署与实机尚未验收。
- C6 的发布命名、镜像构建标识注入、旧页面对新服务器未运行；没有提前推进。
- 新增的原生夹具准备说明经源码与命令参数核对，没有实际启动原生应用做目录验收。

## 给下一位 Agent 的备注

- 原 C4 记录：[2026-10-19-v0.2-phase-c4.md](./2026-10-19-v0.2-phase-c4.md)。本轮只补实际结果，不重写旧失败事实。
- 忽略目录中的详细报告：`target/handoff/report-c4c2-c4c3.md`（最终提交后生成），包含提交、变更文件、实际命令/退出码及证据路径。
- `target/handoff/` 被 Git 忽略；换机器交接时须另行复制证据和脚本，Git 提交只包含实现与文档。

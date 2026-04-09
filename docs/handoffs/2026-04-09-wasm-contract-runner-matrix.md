# wasm contract runner matrix

- 日期：2026-04-09
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：682da79
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

把 wasm browser contract harness 的执行入口收成统一 runner，并让 CI 对 refresh / subscription / config exchange 三条 contract 线都能用同一套 job 逐条执行。

## 影响范围

- 模块：
  - `scripts/run_wasm_contract_harness.sh`
  - `scripts/run_wasm_refresh_contract_harness.sh`
  - `scripts/run_wasm_subscription_contract_harness.sh`
  - `scripts/run_wasm_config_exchange_contract_harness.sh`
  - `.github/workflows/ci.yml`
  - `docs/testing/contract-harness-rebuild-plan.md`
- 平台：
  - Linux
  - Web
  - GitHub Actions
- 额外影响：
  - docs
  - workflow

## 关键变更

### 统一 runner

- 新增通用入口 `scripts/run_wasm_contract_harness.sh`
- 通过 harness 名称参数执行单个 `.wasm` 产物
- 保留三个薄 wrapper：
  - `run_wasm_refresh_contract_harness.sh`
  - `run_wasm_subscription_contract_harness.sh`
  - `run_wasm_config_exchange_contract_harness.sh`

### CI matrix

- `wasm-browser-contract` job 改为 matrix
- 当前覆盖：
  - `wasm_refresh_contract_harness`
  - `wasm_subscription_contract_harness`
  - `wasm_config_exchange_contract_harness`

### 计划文档

- 更新 `contract-harness-rebuild-plan.md`
- 统一记录三条 wasm harness 的实际执行入口

## 验证与验收

### 自动化验证

- `bash -n scripts/run_wasm_contract_harness.sh scripts/run_wasm_refresh_contract_harness.sh scripts/run_wasm_subscription_contract_harness.sh scripts/run_wasm_config_exchange_contract_harness.sh`：通过
- `cargo test -p rssr-infra --target wasm32-unknown-unknown --test wasm_refresh_contract_harness --no-run`：通过
- `cargo test -p rssr-infra --target wasm32-unknown-unknown --test wasm_subscription_contract_harness --no-run`：通过
- `cargo test -p rssr-infra --target wasm32-unknown-unknown --test wasm_config_exchange_contract_harness --no-run`：通过
- `git diff --check`：通过

### 手工验收

- 未执行
- WSL2 下 browser WebDriver 仍受 `environment-limitations.md` 中的 `env-limited` 约束影响

## 结果

- refresh / subscription / config exchange 三条 wasm contract harness 现在都能走同一套 browser runner / CI 模式
- 后续不需要再为新的 wasm harness 复制一套脚本和 workflow

## 风险与后续事项

- 真实 browser 执行结果仍依赖远端 CI runner 验证
- 如 CI 中 Chrome for Testing 或 chromedriver 版本出现漂移，需要优先检查 `setup_chrome_for_testing.sh`

## 给下一位 Agent 的备注

- 优先看 `scripts/run_wasm_contract_harness.sh`
- 其次看 `.github/workflows/ci.yml` 的 `wasm-browser-contract` matrix
- 如果继续补 wasm contract 线，只需新增测试文件并把 harness 名加入 matrix

# wasm browser runner 与 CI 入口

- 日期：2026-04-09
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：aede229
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

为 `wasm_refresh_contract_harness` 补齐了真正的 browser runner 入口与 CI 执行路径，同时把 WSL2 下的本机 WebDriver 限制纳入长期环境限制文档。

## 影响范围

- 模块：
  - `scripts/setup_chrome_for_testing.sh`
  - `scripts/run_wasm_refresh_contract_harness.sh`
  - `.github/workflows/ci.yml`
  - `docs/testing/*`
- 平台：
  - Linux
  - Web
  - GitHub Actions
- 额外影响：
  - docs
  - workflow

## 关键变更

### wasm browser runner

- 新增 `scripts/setup_chrome_for_testing.sh`
- 新增 `scripts/run_wasm_refresh_contract_harness.sh`
- 入口改为：
  - 先编译单个 `wasm_refresh_contract_harness`
  - 再用 `wasm-bindgen-test-runner` 执行单一 `.wasm` 产物

### CI

- 在 `CI` workflow 中新增 `wasm-browser-contract` job
- job 会：
  - 安装 wasm target
  - 安装 `wasm-bindgen-cli`
  - 拉取 Chrome for Testing 与 chromedriver
  - 执行 `scripts/run_wasm_refresh_contract_harness.sh`

### 文档与环境限制

- 更新 `docs/testing/contract-harness-rebuild-plan.md`
- 更新 `docs/testing/README.md`
- 在 `docs/testing/environment-limitations.md` 增加 WSL2 下 ChromeDriver 绑定异常说明

## 验证与验收

### 自动化验证

- `cargo fmt --all`：通过
- `cargo check -p rssr-infra`：通过
- `cargo test -p rssr-infra --target wasm32-unknown-unknown --test wasm_refresh_contract_harness --no-run`：通过
- `git diff --check`：通过

### 手工验收

- `bash scripts/setup_chrome_for_testing.sh`：通过
- `bash scripts/run_wasm_refresh_contract_harness.sh`：env-limited（WSL2 下 `chromedriver` 绑定失败）

## 结果

- 仓库现在有了稳定的单文件 wasm browser harness 执行入口
- GitHub Actions 已具备在非 WSL Linux runner 上真正执行这组 browser harness 的条件

## 风险与后续事项

- 当前本机 WSL2 环境下 `chromedriver` 仍会报 `bind() failed: Cannot assign requested address (99)`
- 这组 harness 的真正 pass/fail 闭环需要下一次 GitHub Actions 执行结果确认
- 后续可继续按同样模式为 subscription / config exchange harness 补 wasm runner

## 给下一位 Agent 的备注

- 先看 `scripts/run_wasm_refresh_contract_harness.sh`
- 再看 `.github/workflows/ci.yml` 里的 `wasm-browser-contract` job
- 如果 CI 里 browser runner 有额外依赖问题，优先从 Chrome for Testing 与 chromedriver 的 runner 环境差异开始排查

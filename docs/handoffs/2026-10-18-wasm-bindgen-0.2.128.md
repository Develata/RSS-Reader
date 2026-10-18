# wasm-bindgen 升到 0.2.128，harness 脚本核对运行器版本

- 日期：2026-10-18
- 作者 / Agent：Claude Code（Opus），与 Develata
- 分支：v0.2-architecture
- 当前 HEAD：见本提交（基于 `7b50fd1`）
- 相关 commit：本提交
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

把锁定的 wasm-bindgen 从 0.2.126 升到 0.2.128（C3 之后约定的事项），并让 `scripts/run_wasm_contract_harness.sh` 在运行前核对测试运行器的版本与 `Cargo.lock` 一致，不一致即失败并报出两个版本。起因：本机全局的运行器是 0.2.128，锁文件是 0.2.126，固定版本的运行器装在会话临时目录里两次被清掉，harness 只报出“路径不存在”。

## 影响范围

- 模块：
  - `Cargo.lock`（只改 wasm-bindgen 一组）
  - `scripts/run_wasm_contract_harness.sh`
  - `Dockerfile.ci-local`、`CLAUDE.md`（版本号）
- 平台：
  - Web（wasm32）。wasm-bindgen 只在 wasm 目标上编译（`cargo tree -i wasm-bindgen` 对 Windows 与 Android 目标都是空），桌面、Android、CLI 与 `rssr-web` 不受影响
- 额外影响：
  - CI 与 Pages 本来就从 `Cargo.lock` 读出 wasm-bindgen 版本、安装对应的运行器，不用改；`Dockerfile` 里 `dx bundle` 自己按锁文件装 wasm-bindgen-cli

## 关键变更

### 依赖

- wasm-bindgen 0.2.126 → 0.2.128（含 -macro、-macro-support、-shared、-test-shared），js-sys 与 web-sys 0.3.103 → 0.3.105，wasm-bindgen-futures 0.4.76 → 0.4.78，wasm-bindgen-test 0.3.76 → 0.3.78；新增 `syn 3.0.9`（0.2.128 的宏依赖，与 syn 1、2 并存）。各 `Cargo.toml` 不变（仍是 `"0.2"` 等）。
- 这一组互相用 `=` 精确绑定，`cargo update -p … --precise` 逐个升都会被另一个拦住；做法是临时把 `rssr-infra` 的要求写成 `=0.2.128`，整组 `cargo update`，再改回 `"0.2"`（锁文件仍满足，`cargo metadata --locked` 通过）。当时最新是 0.2.129，按约定取 0.2.128（与本机全局运行器一致）。

### harness 脚本

- 运行器取 `RSSR_WASM_BINDGEN_TEST_RUNNER`，没设时取 PATH 里的 `wasm-bindgen-test-runner`，与 `scripts/wasm_contract_runner.rs` 的选法一致；存在性检查也针对这个运行器。
- 运行前从 `Cargo.lock` 读出 wasm-bindgen 的版本（必须恰好一个），与 `运行器 --version` 比对，不一致就失败，提示安装对应版本的 wasm-bindgen-cli 或设置 `RSSR_WASM_BINDGEN_TEST_RUNNER`。`--prepare` 模式不需要运行器，不检查。
- 找不到工具时的提示由“需要在 PATH 里”改为“需要但没有找到”（运行器可以是显式路径）。

## 验证与验收

### 自动化验证

Windows（rustc 1.99.0，基准 `7b50fd1` 加本次改动，开始时空余提交内存 14.0 GB；构建 `-j 2`；日志在会话临时目录 `evidence-wbg-0.2.128/`）：

- 锁文件里 js-sys 0.3.105、wasm-bindgen 0.2.128、wasm-bindgen-futures 0.4.78、wasm-bindgen-test 0.3.78、web-sys 0.3.105；PATH 里的运行器 0.2.128。
- `cargo fmt --all --check`：退出码 0。
- `cargo check --workspace --all-targets --locked`：退出码 0。
- `cargo clippy -p rssr-app -p rssr-infra -p rssr-api -p rssr-domain -p rssr-application --target wasm32-unknown-unknown --locked -- -D warnings`：退出码 0。
- `cargo check -p rssr-infra --tests --target wasm32-unknown-unknown --locked`：退出码 0。
- `bash scripts/run_wasm_contract_harness.sh`（运行器 0.2.128，经新的版本核对）：refresh 33、subscription 31、config_exchange 5 passed，退出码均为 0。
- `dx bundle --platform web --package rssr-app --release --debug-symbols false --locked`（dioxus-cli 0.7.9，用 `cargo binstall` 装在 worktree 的 `target/tools/dx-0.7.9`；本机全局的是 0.7.10）：退出码 0，dx 自动安装的是 wasm-bindgen-cli 0.2.128，产出 `public/index.html` 与 `assets/`。
- 版本核对本身：指向旧的 0.2.126 运行器时退出码 1，报 “reports 'wasm-bindgen-test-runner 0.2.126', but Cargo.lock locks wasm-bindgen 0.2.128”；指向不存在的路径时退出码 1。

### 手工验收

- 浏览器里打开 web 包实际使用：未执行。
- 原生工作区测试、WSL、Android：未执行（这次的依赖只在 wasm 目标上编译，见“影响范围”）。
- 真实 Docker 镜像构建：未执行（与 C2 的待补项相同）。

## 结果

- 可合并。Web 包与浏览器契约测试用 0.2.128 通过；其他平台不受影响。

## 风险与后续事项

- 0.2.129 已发布，下次升级同样整组更新，并换对应版本的运行器。
- 新增的 syn 3 会让 wasm 构建多编译一个 syn。
- 自写的有界 feed 解析器：Develata 2026-10-18 同意之后再做（若做，放在工作区内的独立 crate，先写设计）。

## 给下一位 Agent 的备注

- 运行 harness 前确认 `wasm-bindgen-test-runner --version` 与 `Cargo.lock` 一致，或用 `RSSR_WASM_BINDGEN_TEST_RUNNER` 指向对应版本；脚本会核对。
- 不要把固定版本的工具装在会话临时目录（`%TEMP%`），装在 worktree 的 `target/tools` 下。

# Release UI launcher：遵循 Cargo 配置的编译器

- 日期：2026-10-04
- 作者 / Agent：dot
- 分支：refactor/release-ui-runner-rust
- 修改基线 HEAD：7cc59b296f23c59da797fe79cda33234bccfb1c9
- 相关 commit：本文件所在的 `fix: respect Cargo configured compiler in release launcher` 提交
- 相关 tag / release：N/A
- 状态：`draft`（提交时等待该提交的 GitHub CI；不得把旧 SHA 的绿色检查作为本轮验收）

## 工作摘要

修复 PR #20 的窄范围 P3：launcher 原先以裸 `rustc -vV` 推断 host，
可能与 Cargo 通过环境变量或配置文件实际选择的编译器不一致。
仅修改验收工具启动适配、黑盒回归和文档，不改变产品行为、依赖或网络安全规则。

## 影响范围

- 模块：`scripts/run_release_ui_regression.sh`、`scripts/release-ui/tests/acceptance.py`
- 平台：Unix / Linux 与 Windows Git Bash；macOS 本轮未实机验证
- 文档：runner 调用依赖与本交接
- workflow：沿用现有 Linux / Windows `release-ui-runner` CI job，无新增 workflow

## 关键变更

- Cargo 1.91 起支持 `--target host-tuple`，由 Cargo 解析所配置编译器的 host；
  不自行重写 Cargo 配置优先级，不调用可能被 PATH 抢占的裸 rustc。
- 使用 Cargo JSON 中所选 bin 的 `compiler-artifact.executable`，成功构建后直接 exec。
  不恢复历史上受 `CARGO_BUILD_TARGET` 影响的无 triple 旧产物路径问题。
- Python 3 仅作 artifact JSON 解析器，新增 launcher 启动依赖已明确记录；
  缺失时给出错误，不增加产品 Rust/Python 依赖或新的测试框架。
- 回归使用真实 Shell、真实 Cargo 和真实编译器，PATH 中放置原生 poison rustc。
  分别覆盖 RUSTC、CARGO_BUILD_RUSTC、build.rustc，以及高优先级覆盖低优先级 poison。
  保留旧 host probe 作为有界 A/B fixture：旧 probe 必须失败，新 launcher 必须输出有效 plan。
- 同时设置 wasm 默认 target、放置旧无 triple 二进制；新增缺失编译器在已有 host
  缓存时仍失败的检查。原有 target/config/stale/missing-unqualified 回归继续运行。

## 验证与验收

### 自动化验证

以下命令已由现有 CI 定义覆盖，提交时尚未取得本 SHA 的运行结果：

- `cargo fmt --manifest-path scripts/release-ui/Cargo.toml --check`：待远端 CI
- `cargo clippy --manifest-path scripts/release-ui/Cargo.toml --all-targets --locked --target-dir target/release-ui-runner -- -D warnings`：待远端 CI
- `cargo test --manifest-path scripts/release-ui/Cargo.toml --locked --target-dir target/release-ui-runner`：待远端 CI
- `cargo build --manifest-path scripts/release-ui/Cargo.toml --locked --target-dir target/release-ui-runner`：待远端 CI
- `python -X utf8 scripts/release-ui/tests/acceptance.py --bash "$BASH"`：待远端 Linux / Windows CI
- 本地命令 / Windows 本机 / full UI 聚合：未执行，本轮使用 GitHub connector 读写与远端 CI 验证
- A/B 日志由原有 acceptance artifact 上传步骤收集，保留各 compiler 模式的 before / after 日志

### 手工验收

- 源码与 Cargo 官方稳定接口文档核对：完成
- 产品浏览器 UI / 外部 RSS 可达性 / wasm-opt：本轮未重新验收，既有边界不因此变为通过

## 结果

- 本补丁提交后必须检查其精确 SHA 的 Linux / Windows CLI 与 CI 总门禁，绿色后方可考虑合并。
- 不在本次 connector 修改中合并 PR、打 tag 或发布；交由后续审阅确认。

## 风险与后续事项

- launcher 的最低 Cargo 版本为 1.91；仓库现用 stable，未声明支持更老 Cargo。
- Python 3 现在也用于 launcher 启动阶段，包括 `--help` / `--plan`。
- 标准验收针对真实 Cargo 输出；不支持用户自定义 Cargo wrapper 篡改 JSON 协议。
- 进程监督、取消、日志背压与产品阶段实现均未修改。

## 给下一位 Agent 的备注

- 入口与新增测试：`run_release_ui_regression.sh` /
  `Acceptance.test_real_launcher_respects_configured_compiler`
- 精确提交与 CI 结果应从 PR #20 当前 head 及 Actions 检查核对，不引用旧提交的结论。

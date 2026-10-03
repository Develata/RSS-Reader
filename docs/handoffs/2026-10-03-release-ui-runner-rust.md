# 发布验收入口 Rust 化：本地 fixture 验收与独立 Draft PR

- 日期：2026-10-03
- 作者 / Agent：Codex
- 分支：`refactor/release-ui-runner-rust`
- 当前 HEAD：`f1984c04f2a2875faaa9e175d746417ae80ebcf6`
- 相关 commit：**commit: pending**（本机验证完成，待定向提交）
- 相关 tag / release：N/A
- 状态：`draft`；本地 fixture 聚合通过，外部 feed 另列限制；远端交付待完成

## 工作摘要

把 409 行 `scripts/run_release_ui_regression.sh` 的参数、执行计划、阶段状态、
build-once、日志报告和退出码迁入独立 Rust CLI。已有成熟阶段脚本继续持有其检查，
Shell 总入口收敛为构建/exec，不建立双向事件协议或第二套阶段状态。

用户明确授权在此主目录从最新 main 建独立分支、验证、commit/push 并建 Draft PR。
首次在外部 feed 环境限制处停止，随后用户明确批准：补齐匹配工具与本地 fixture 验收，
单列 DNS/代理/HTTPS 限制后继续交付；禁止修改安全规则或绕过网络限制。本记录合并同日续做结果，
不把本地 fixture 通过写成完整外部可达性或发布验收通过。

## 影响范围

- 模块：`scripts/release-ui/`、`scripts/run_release_ui_regression.sh`、新增
  `scripts/run_rssr_web_auth_smoke.sh`；现有 `scripts/wasm_contract_runner.rs` 增加显式工具路径。
- 平台：本机 Windows MSVC + Git Bash 已验证；Unix 信号路径已实现，尚无运行证据。
- 额外影响：`docs/testing/release-ui-runner.md`、发布清单链接、拟新增的 CI CLI 测试矩阵。
- 没有修改 PR #19 目录行为、产品 Rust、存储、JS/CDP 断言或产品 workspace/锁文件。

## 关键变更

### 基线与结构

- 从 `origin/main` 的指定 merge SHA 建分支；旧 HEAD 与它的文件树相同，因此没有
  stash/reset/clean 或 worktree 切换。主目录仍是 `E:/gitclone/RSS-Reader`。
- 旧入口保存在忽略目录 `target/release-ui-baseline/run_release_ui_regression.sh`，完整行为表见
  [runner 文档](../testing/release-ui-runner.md)。不是新增需要长期维护的旧实现。
- 选择 `scripts/release-ui/Cargo.toml` 独立 package/lockfile，而不是产品 workspace xtask。
  clap 管参数；process-wrap 管 process group/Job；signal-hook/ctrlc 管平台信号；
  serde 报告。Windows process-lifetime Job 补强父进程强杀后的后代清理。
- 部署登录 smoke 从旧函数提取，HTTP 断言未改变。增加信号退出 trap；Rust 收尾保证
  native cargo/service/browser 后代不会只因 shell 父进程消失而残留。

### 行为与平台差异

- 保留阶段顺序及现有参数；新增无副作用 `--plan`、直接执行时的 `--repo-root`/`--bash`。
- `summary.json` 和 Markdown 由同一阶段列表生成；failed/interrupted 后续为 blocked；
  skipped 不升级 passed，skip-build 为 reused，SPA ready 为 serving 且仍需手工验收。
- 进程失败保留真实退出码；非法端口/缺值/未知参数为 1，帮助为 0。
- 实测修复 Windows append-only 日志句柄导致 Bash 静默退出 1 的问题。
- Windows 仅排除 MSYS 对 `curl --data-urlencode next=/feeds` 中 `next=` 的路径转换，
  保留其他转换与已有排除设置（包括 `*`）。
- CI 工作区新增 `release-ui-runner` Linux/Windows 矩阵并纳入总门禁；尚未推送/执行。
- 新增显式 `--skip-external-feed`，默认 full 行为不变；仅排除外部 proxy-feed。
  固定组为 partial，总结果为 completed-with-skips；原有本地 browser feed fixture 仍须真实通过。
- wasm 适配器仅增加 `RSSR_WASM_BINDGEN_TEST_RUNNER` 绝对路径选择，不改变契约或产品锁文件。
  单纯前置 PATH 在本机 Git Bash/Cargo 边界仍选中全局版本，因此采用显式路径。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：通过，2.41 秒（缓存）。
- `cargo test --workspace --locked`：通过；日志 `target/release-ui-validation/workspace-tests.log`。
- `cargo fmt --manifest-path scripts/release-ui/Cargo.toml --check`：通过。
- `cargo clippy --manifest-path scripts/release-ui/Cargo.toml --all-targets --locked --offline --target-dir target/release-ui-runner -- -D warnings`：通过。
- `cargo test --manifest-path scripts/release-ui/Cargo.toml --locked --offline --target-dir target/release-ui-runner`：3 个单元测试通过，覆盖参数错误、128 种开关组合和显式排除外部检查。
- `python -X utf8 scripts/release-ui/tests/acceptance.py --bash C:\Users\QQ\scoop\apps\git\current\bin\bash.exe`：续做后 12 项黑盒测试通过，23.20 秒。
  证据 `target/release-ui-acceptance/1791043941174940900/`；新增测试验证 skipped/partial 不误报 passed，实际阶段失败仍终止。
- 增补 Git Bash TERM 后，`Acceptance.test_cancellation_and_forced_termination` 单独复测通过，
  证据 `target/release-ui-acceptance/1791041850790036200/`。
- 覆盖成功/失败/取消/强杀后的三层原生进程树、PID 消失、端口释放，真实 Bash/Python SPA，
  服务提前退出、端口冲突、原始退出码 23、阶段阻断、日志 stdout/stderr、中文空格目录。
- Windows Ctrl+C / Ctrl+Break：130、interrupted、后代清零；Git Bash `kill -fW -TERM`
  测试 WinPID：143、failed、后代清零；TerminateProcess：1、后代清零，最后报告仍 running。
  强杀不能保证落盘终态，未完成报告不得作为通过凭据。没有把这些结果写作 Unix SIGTERM 实证。
- `actionlint -color`：通过；Shell 兼容入口/阶段脚本已由真实执行覆盖。
- `rustfmt --edition 2024 --check scripts/wasm_contract_runner.rs`、`clippy-driver --edition 2024 --test scripts/wasm_contract_runner.rs -o target/release-ui-validation/wasm-runner-tests.exe -D warnings`：通过；生成测试程序的 6 项测试通过。
- 使用现有 cargo-binstall 安装官方预编译 `wasm-bindgen-cli =0.2.126` 到
  `target/release-ui-tools/wasm-bindgen-0.2.126`，18.27 秒；禁用源码安装回退，未改全局 0.2.128。
  test runner SHA256：`977E6FEC176F2B43C0C2AF4D075BF6CF4994E82F922C49233D225A3D1B2EC64B`。
- 三个真实 wasm harness 共 51 项通过（config 3、refresh 28、subscription 20），独立轮次 29.82 秒。
  `wasm-matched/`、`wasm-pinned/` 保留 PATH 仍选错版本的失败，`wasm-explicit/` 保留通过证据。

### 实际 aggregate 与旧/新差异

- 旧 Bash `--release --skip-build --no-serve` 自动门禁通过，约 10.50 秒，四组可选检查 skipped。
  新 CLI 相同四条自动门禁也通过；没有将这些 skipped 记为通过。
- 第一轮新 aggregate 复用旧 release bundle，目录断言失败。旧 bundle 07:06 生成，
  早于 PR #19 最后修复 `282592f` 的 07:54；保留失败证据，随后从当前源码重新构建。
- 当前源码轮次：

  ```bash
  CHROME_BIN='C:/Program Files/Google/Chrome/Application/chrome.exe' \
  bash scripts/run_release_ui_regression.sh --release --no-serve \
    --with-rssr-web --with-fixed-smokes --port 18191 --web-port 19181 \
    --log-dir target/release-ui-validation/aggregate-fresh
  ```

  159.45 秒，最终 **exit 1 / failed**。自动门禁、一次 Web 构建、登录 smoke、五主题 Reader、
  小视口 278 条断言通过；proxy-feed failed；fixed-browser-feed blocked；browser contracts/SPA skipped。
- 真实阻塞：`/feed-proxy` 返回 HTTP 400，正文为“出于安全原因，禁止代理内网或本地地址。”
  本机 DNS 在只读权限核对后仍解析默认 `www.ruanyifeng.com` 为 `198.18.1.2` / `2001:2::f3`；
  对同一 URL 的 HTTPS HEAD 另返回 403。没有修改代理安全规则、DNS、feed URL、CDP 断言。
- `dx build` 0 退出，但日志包含 wasm-opt `0xc0000409` 后继续复制产物；只确认构建命令和后续
  UI 断言通过，不声称 wasm-opt 成功。
- 聚合退出后 18191/18201/18202/28202/19181/19191/19192 均可重绑。
- 原/新日志与报告：`target/release-ui-baseline/`、`target/release-ui-validation/aggregate/`、
  `target/release-ui-validation/aggregate-fresh/`。后者 summary 已正确记录失败和 blocked。

### 续做：本地 fixture 实际聚合

设置 `RSSR_WASM_BINDGEN_TEST_RUNNER` 为上述 0.2.126 绝对路径、`CHROME_BIN` 为已有 Chrome，执行：

```bash
bash scripts/run_release_ui_regression.sh --release --full --skip-external-feed --no-serve \
  --port 18191 --web-port 19181 --log-dir target/release-ui-validation/aggregate-local
```

- 167.87 秒，**exit 0 / completed-with-skips**。自动门禁、51 项浏览器 wasm 契约、一次同 profile
  Web 构建、登录 smoke、五主题、278 项小视口与添加/刷新/阅读同源 feed fixture 均通过。
- proxy-feed 为 skipped / exit null，固定组 partial；SPA 由 no-serve 明确跳过。
  外部网络限制仍是独立未通过项，不用本地 fixture 替代其结论。
- 所有阶段 PID 已退出；18191/18201/18202/28202/19181/19191/19192 均可重绑。
- dx 0.7.10 对 dioxus 0.7.9 报版本不一致，wasm-opt 仍为 `0xc0000409` 后复制产物；
  构建命令与下游断言通过，不等于优化器成功。未升级工具或产品依赖来掩盖此限制。

### 成本

- 离线依赖缓存、全新 target 的 CLI 编译：14.38 秒；debug exe 2,220,032 字节。
- 直接 help 122.69 ms、plan 18.63 ms、全部跳过 21.86 ms；缓存 Shell 入口 653.88 ms。
- 旧 Shell 全部跳过约 851 ms。单次、含进程启动与本机并行负载，不是统计性能结论。
- 不涉及产品运行速度提升。记录：`target/release-ui-validation/costs.txt`、`cold-build.log`。

## 结果

- Rust CLI 实现及本机直接测试可审阅；本地 fixture 聚合通过，完整外部发布 aggregate 尚未通过。
- 按用户续做授权推进独立 Draft PR；本记录写入时 commit/远端状态仍待更新。没有 merge / tag。
- 自审：产品边界和既有断言未改；唯一编排状态位于 Rust；子进程生命周期验证在本轮完成；
  现有用户改动与本任务内容保持分离。

## 风险与后续事项

- 默认外部 feed 的本机 DNS/403 限制仍存在；在具备合法可达网络的环境重跑默认 full 才能补齐。
  不通过改产品 SSRF 规则、网络设置或替换既有检查绕过此限制。
- wasm 匹配工具已在 target 下完成验证；全局工具与产品锁文件未改。
  WSL 枚举 E_ACCESSDENIED，未转用 WSL/另一台机器。
- Linux SIGINT/SIGTERM、Linux/Windows CI、最终远端完整 SHA 均待执行；当前没有 PR 编号。
- Unix SIGKILL、主动逃逸进程组的自守护进程、断电和日志磁盘故障的完整报告不可保证。
- `--skip-build` 保持旧含义，调用者需确认产物来源；Git HEAD 不证明 dirty 工作区或缓存来源。

## 给下一位 Agent 的备注

- 入口为 `scripts/release-ui/main.rs`（调度/报告）、`plan.rs`（参数/计划）、`process.rs`
  （信号/进程树）、`tests/acceptance.py`（黑盒 OS 验证）、`tests/fixture.rs`（故意遗留后代的原生夹具）。
- 保留的原有内容：22 项 `.agents/skills` / `.specify` 删除、`.workbuddy/`、
  `docs/handoffs/2026-10-03-directory-follow-rust.md` 与 `2026-10-03-directory-review-correction.md`
  的收尾补记；它们不属于本任务提交范围。
- 切换前快照为 `C:/Users/QQ/AppData/Local/Temp/rssr-release-runner-preserved.json`，记录各原有
  dirty/untracked 文件 SHA256 与 MISSING 项；切换后逐项一致，收尾再次核对 25 项全部一致。
  暂存区仍为空，产品目录、根 Cargo.toml/Cargo.lock 与 scripts/browser 没有本任务 diff。
- 继续时不要重建 C 盘 worktree、stash/reset/clean，不要混入上述用户内容。若完成后提交，
  只显式 stage 本任务路径，再按授权 push 新分支、建独立 Draft PR、核对远端 SHA 与 CI，不 merge。

# Release UI runner 行为基线与迁移

基线：`f1984c04f2a2875faaa9e175d746417ae80ebcf6` 的
`scripts/run_release_ui_regression.sh`，409 行。2026-10-03 在 Windows
LAPTOP-H6JEOCF0 / Git Bash 检查；下表来自源码，不冒充整套实测。

## 旧入口契约

| 项目 | 行为 |
| --- | --- |
| 默认 | debug，静态端口 8091，部署端口 18081；自动门禁后启动 SPA |
| profile | `--debug` / `--release` 最后一个生效 |
| 开关 | `--skip-automated`、`--skip-build`、`--no-serve`、`--with-rssr-web`、`--with-browser-contracts`、`--with-fixed-smokes` |
| full | 同时启用三个 with 开关，不覆盖 skip/no-serve/profile |
| 路径 | `--log-dir DIR`；默认 `target/release-ui-regression/<本地时间>`；相对路径以仓库为基准 |
| 自动门禁 | wasm check → app tests → 三个 infra host contracts → rssr-web tests |
| browser | 三个 wasm contracts，由 `run_wasm_contract_harness.sh` 执行 |
| bundle | 首个部署 smoke / fixed smoke / SPA 之前构建一次；后续同 profile 复用；skip-build 仍检查 public 目录 |
| 部署 smoke | 拒绝已有监听；启动 cargo rssr-web；健康检查、登录重定向、cookie 会话、feeds/settings、登出；失败时清理 |
| 部署 browser smoke | with-rssr-web 且无 fixed-smokes 时执行，端口 web-port + 1 |
| fixed smokes | reader 五主题 (port + 10) → 小视口 (port + 11，CDP 再 + 10000) → proxy feed (web-port + 10) → browser feed (web-port + 11) |
| SPA | 复用 bundle，调用 `run_web_spa_regression_server.sh`，终端常驻以便手工检查 |
| 产物 | summary.md、automated-gates.log、browser-contracts.log、rssr-web.log、rssr-web-browser-feed-smoke.log、fixed-smokes.log，四个固定 smoke 子目录 |
| 失败 | set -e / pipefail 传递命令失败；不会继续后续阶段；summary 可能仍为 pending，SPA 正常运行也保留 pending |
| 参数错误 | 未知参数/缺值退出 1；未统一校验端口；`--help` 也退出 1 |

本轮保持命令、阶段顺序、profile、产物位置和 skip 的含义。失败报告改为
failed/interrupted，后续未运行阶段为 blocked，显式未选择的阶段为 skipped；
不把进程退出 0 或 SPA 启动写作人工验收通过。

## 实现选择

独立 `scripts/release-ui/Cargo.toml` 不增加产品 workspace 成员，也不引入产品依赖。
与现有 std-only wasm runner 相比，本入口需要管理多个层级的服务/浏览器子进程；
使用 process-wrap 的 Unix process group / Windows Job Object 和平台信号库，
避免手写 Windows 挂起创建、附加 job、恢复线程的竞态处理。
参考 [process-wrap 文档](https://docs.rs/process-wrap/10.0.1/process_wrap/)。
Shell 兼容入口只构建和 exec；不保留另一套阶段状态，不建立 Shell/Rust 事件协议。

Windows 另有一个不继承句柄的进程生命周期 Job：即使 runner 被强制终止，
OS 关闭最后一个句柄时也回收其后代。每个阶段有自己的 Job / process group，
正常结束也清理遗留后代；Unix 中断先转发信号，留 1.5 秒给原脚本 trap，然后强制回收。
不依靠全局进程名查杀。此设计没有迁移 application/domain/infra 职责。

## 调用与依赖

```bash
# 原有接口继续使用；首次会构建独立 CLI，以后 Cargo 复用缓存。
bash scripts/run_release_ui_regression.sh --release --full --no-serve
bash scripts/run_release_ui_regression.sh --full --plan

# 环境限制外部 RSS 时，只验既有本地 fixture；报告明确是部分验收。
bash scripts/run_release_ui_regression.sh --release --full --skip-external-feed --no-serve

# 独立构建与验证，不改变根 Cargo.lock 或产品 workspace。
cargo build --manifest-path scripts/release-ui/Cargo.toml --locked --target-dir target/release-ui-runner
cargo fmt --manifest-path scripts/release-ui/Cargo.toml --check
cargo clippy --manifest-path scripts/release-ui/Cargo.toml --all-targets --locked --target-dir target/release-ui-runner -- -D warnings
cargo test --manifest-path scripts/release-ui/Cargo.toml --locked --target-dir target/release-ui-runner
python -X utf8 scripts/release-ui/tests/acceptance.py --bash "$BASH"
```

PowerShell 可直接运行已构建的 `.exe`：

```powershell
$env:RSSR_BASH = Join-Path (scoop prefix git) 'bin\bash.exe'
& .\target\release-ui-runner\debug\release-ui.exe --no-serve
& .\target\release-ui-runner\debug\release-ui.exe --full --plan
python -X utf8 scripts/release-ui/tests/acceptance.py --bash $env:RSSR_BASH
```

原生自动门禁直接调用 cargo；Bash 阶段仍需要 Git Bash/Unix Bash、Python、curl、
rg、Node/Chrome、dx，以及与产品锁文件匹配的 wasm-bindgen-test-runner/driver。
Windows 不会默认选中 system32/bash.exe（那是 WSL）；用 `--bash` 或 `RSSR_BASH`
明确选择 Git Bash。`CHROME_BIN`、`NODE_BIN` 等现有子脚本接口继续传递。
Shell launcher 在解析任何参数前先确保 CLI 编译成功，所以首次 `--help` 也有编译成本。
launcher 需要 Cargo 1.91+（当前 stable）以及 PATH 中的 Python 3（`python3` 或 `python`）。
Python 仅解析 Cargo 的 JSON artifact 输出；选项、阶段和状态仍由 Rust 管理。
通过 Cargo 的 `--target host-tuple` 选择所配置编译器的 host，遵循
`RUSTC`、`CARGO_BUILD_RUSTC`、`build.rustc` 的 Cargo 优先级，不另行调用 PATH 中的
`rustc`。构建成功后只 exec 本次 `compiler-artifact.executable` 指定的 CLI；
不猜测 host 目录、不扫描旧产物，构建或 artifact 解析失败时不会执行缓存二进制。
Python 缺失会在构建前给出明确错误。Cargo 编译诊断仍显示在 stderr，`--plan` 的 stdout
保留为纯 JSON。Windows artifact 路径转换为正斜杠以兼容 Git Bash，支持中文和空格。
参见 [Cargo 1.91 changelog](https://doc.rust-lang.org/cargo/CHANGELOG.html#cargo-191-2025-10-30)
与 [Cargo JSON artifact 格式](https://doc.rust-lang.org/cargo/reference/external-tools.html#compiler-artifact-messages)。
`CARGO_BUILD_TARGET` / `build.target` 仍传给后续产品命令，不决定验收工具本身的运行平台。
上面的手工 Cargo 构建命令未指定 target，故其直接二进制示例仍位于未带 triple 的目录。

Unix launcher 和阶段调用不改 PATH 优先级，阶段直接使用所选 Bash。
Windows 只把 Git Bash 基础工具目录追加到 PATH，启动阶段及其裸 `bash` 子调用继续使用该
Git Bash，避免选中 system32 的 WSL 入口。已有 `RSSR_BASH` 不被 launcher 覆盖。

若全局 wasm 工具与产品锁文件不同，可将匹配的预编译工具放在仓库 `target/` 下，
显式设置 `RSSR_WASM_BINDGEN_TEST_RUNNER` 为该可执行文件的绝对路径。适配器默认仍用 PATH；
显式路径会透传到 Cargo 启动的 wasm runner，避免 Git Bash/Cargo 边界改变 PATH 优先级。
例如本次 Windows 验证使用：

```powershell
$env:RSSR_WASM_BINDGEN_TEST_RUNNER = Join-Path (Get-Location) 'target/release-ui-tools/wasm-bindgen-0.2.126/bin/wasm-bindgen-test-runner.exe'
```

`--skip-external-feed` 只在 `--full` / `--with-fixed-smokes` 下有效，只排除既有外部
proxy-feed smoke。原有本地同源 feed fixture 的添加、刷新、阅读流程仍会执行。
这不覆盖外部代理可达性；代理安全逻辑仍由现有本地 HTTP fixture / SSRF 单元测试验证。
默认 full 保持执行外部检查，外部失败仍终止后续阶段并返回真实非零码。

`--repo-root` 默认是构建时工具源码所在仓库；复制二进制到别处时显式指定。
`--log-dir` 的相对路径以仓库为基准，支持空格和中文。新增默认目录名为
`target/release-ui-regression/<Unix毫秒>-<PID>`，避免秒级目录碰撞。
`--plan` 不启动子进程、不创建日志，输出所有阶段及 enabled/argv/log/port。

## 状态、退出码与已知差异

- 保留旧参数错误退出 1、命令失败的原始非零退出码；新增 `--help` 成功退出 0。
- 端口统一按十进制校验 1..65535；fixed 模式要求 port <= 55524、web-port <= 65524，
  包括小视口的 CDP 派生端口。旧脚本缺少这一统一检查。
- `summary.md` 保留原五组状态和人工补记栏目，增加 Web bundle 和逐阶段明细；
  `summary.json` schema_version=1 含命令、PID、状态、耗时、退出码。
  只保存一个 Rust 阶段列表，分组状态从该列表计算。
- `skipped` 只表示用户未选择；失败后后续阶段为 `blocked`；正常复用构建为 `reused`。
  `completed` 仅表示选中的命令执行完毕，不表示手工验收完成。
- 显式 `--skip-external-feed` 将 proxy-feed 标成 `skipped`，固定组为
  `partial (external feed skipped)`，总体为 `completed-with-skips`；其余阶段失败仍返回非零码。
- SPA 经 HTTP 就绪探测后为 `serving`，始终需要人工检查；提前退出（包括退出 0）失败。
  端口占用在启动阶段前被拒绝，不关闭占用者。
- 捕获的 Unix SIGINT/SIGTERM 分别计划返回 130/143；本机 Windows Ctrl+C/Ctrl+Break
  已验证返回 130、记录 interrupted，清理真实 Bash/Python 服务和三层原生进程树。
- Windows Git Bash `kill -fW -TERM <本次测试WinPID>` 实测返回 143、报告 failed，后代清零；
  这不是 Unix SIGTERM handler 的实证。Windows TerminateProcess 实测返回 1，后代清零，
  最后 summary 仍为 running。强制终止不可保证写终态；running/pending/无报告均是未完成验收，
  不可当作通过。Unix SIGKILL、逃离 process group 的自守护进程及系统断电未验证。
- 在 Windows 使用普通可写日志句柄定位到末尾，避免 append-only 句柄使 Git Bash 静默退出 1。
  仅排除 MSYS 对 curl `next=` 表单字段的路径转换，并保留调用者既有排除设置。
- `commit` 是运行时可读的 Git HEAD，不证明工作区 clean；`--skip-build` 不校验缓存产物来源。
  本轮真实发现旧 bundle 早于 PR #19 最后修复，重新构建后对应 278 项小视口断言通过。
- 版本由 `git rev-parse --verify HEAD` 读取，支持 `.git` 文件、linked worktree、分离 gitdir、
  packed refs 和 detached HEAD。该探测也受进程树清理、取消和 5 秒超时约束；失败记 unknown，
  原始输出保存在 `git-revision.log`，不退回读取猜测的 ref 路径。
- 阶段完整 stdout/stderr 保存在日志文件。控制台转发是有界的尽力输出：单个后台线程、
  最多 16 个 8 KiB 块，队列满时丢弃控制台副本；监督线程每次最多读取 128 KiB 后继续轮询。
  收尾最多等 100 ms，不 join 可能卡住的输出线程。状态通知同样经过该队列，报告不依赖管道读者。
  高频输出下应以日志文件和 summary 为验收依据，不依赖控制台内容完整。

## 2026-10-03 本机证据与阻塞

环境：Windows LAPTOP-H6JEOCF0，Rust 1.98.1，Git Bash 5.3.15 / MSYS 3.6.10，
Chrome 154.0.8037.93，Python 3.14.7。仓库根目录保持 `E:/gitclone/RSS-Reader`。

- 三个单元测试、128 种开关计划组合通过；12 项黑盒测试通过，包含 Git Bash TERM
  边界。真实子进程断言包括成功/失败/取消/强杀后 PID 消失及端口可重绑。
- 根 workspace fmt/clippy/tests 通过；独立包 fmt/clippy/tests 通过；actionlint 通过。
- 原 Bash 自动门禁实跑通过，其他组 skipped，10.50 秒；新 CLI 相同四条自动门禁也通过。
- 新 CLI 从当前源码构建的一轮 aggregate（自动门禁 + rssr-web + fixed，未选择 browser contracts
  和 SPA）耗时 159.45 秒，**最终退出 1**。Web bundle 只构建一次，登录 smoke、五主题 Reader、
  小视口 278 项通过；proxy-feed 失败，fixed-browser-feed 正确 blocked。
- 后续 `--release --full --skip-external-feed --no-serve` 的实际聚合耗时 167.87 秒，
  exit 0 / `completed-with-skips`。自动门禁、51 项 wasm 契约、一次 Web 构建、登录、五主题、
  278 项小视口与既有同源 browser feed 流程通过；proxy-feed 明确 skipped，固定组为 partial。
  日志 `target/release-ui-validation/aggregate-local/`。阶段 PID 均已退出，七个端口可重绑。
- 真实阻塞：既有默认 feed `www.ruanyifeng.com` 本机 DNS 返回 `198.18.1.2` / `2001:2::f3`，
  rssr-web 原有安全规则拒绝并返回 400。离开受限 token 的只读核对仍返回这些地址，HTTPS HEAD
  又得到 403。没有修改代理规则、DNS、feed URL 或断言来放行。
- 全局 wasm-bindgen-test-runner=0.2.128，产品 Cargo.lock=0.2.126。经用户授权续做，使用已有
  cargo-binstall 安装官方预编译 0.2.126 到仓库 `target/release-ui-tools/wasm-bindgen-0.2.126`，
  安装用时 18.27 秒；未改全局工具或产品锁文件。仅修改 PATH 仍选中全局工具的两次失败保留。
  显式路径后 config / refresh / subscription 契约分别 3 / 28 / 20 项通过，独立轮次 29.82 秒。
  WSL 枚举拒绝访问；未改用 WSL、另一台机器或 C 盘 worktree。
- dx 构建返回 0，但日志含 wasm-opt `0xc0000409`，随后使用生成产物完成构建。
  本机 dx=0.7.10 对 dioxus=0.7.9 还报告版本不一致。记录工具差异与降级，不宣称优化器成功。
- 聚合结束后 18191/18201/18202/28202/19181/19191/19192 端口均可重绑。
  日志位于 `target/release-ui-validation/aggregate-fresh/`；失败的旧缓存轮次保留在 `aggregate/`。

单次成本测量（不是统计基准）：离线依赖缓存 + 全新 target 编译 14.38 秒；debug exe 2,220,032
字节；直接 `--help` 122.69 ms、`--plan` 18.63 ms、全部跳过 21.86 ms；缓存 Shell 启动 653.88 ms。
旧 Shell 全部跳过约 851 ms。测量含进程启动和本机负载，不支持产品运行速度变快的结论。

外部 feed 的 DNS/HTTPS 限制仍保留为单独未通过项，不以本地 fixture 代替外部可达性证据。
Linux/Windows CI 及最终提交状态以交接记录和 PR 为准；不能把本机 Windows 结果写作
Unix 信号验证。完整交接见
[2026-10-03-release-ui-runner-rust.md](../handoffs/2026-10-03-release-ui-runner-rust.md)。
后续审阅修复及对应复现证据见
[2026-10-03-release-ui-runner-review-fixes.md](../handoffs/2026-10-03-release-ui-runner-review-fixes.md)。

## web-auth 生命周期迁移（第一小步，2026-10-05）

基线为 `f69087a61a4627cbd3ba88fa082d7722df811dc5`。Rust 现在在
`web-auth` 阶段预检端口，直接拥有 `cargo run --locked -p rssr-web`，
调度 readiness、取消和清理。继续复用 `OwnedProcess`、`Cancellation`、
`LogTail` 和既有报告；未新增状态机或依赖。服务的 Unix 清理仍使用既有
1.5 秒 grace 上限；2026-10-06 起，仅在回收 leader 并确认整个组已不存在时
提前结束等待，仍存活或无法确认的组继续获得原有宽限及强制清理。
探测使用已锁定的 libc（新增为 Unix 直接依赖，无新增依赖包）。
这不改变尚未迁移的 Chrome 5 秒 grace。

`--plan` 的该阶段现在显示 `release-ui --web-auth-only ...`；
阶段名称、原有聚合顺序、schema_version=1 及字段保持。该阶段 PID 现在是
受监管的 cargo/service leader，而不是旧 Bash leader。新增单阶段入口：

```bash
target/release-ui-runner/debug/release-ui --web-auth-only --debug --web-port 18081
# 原来的三参数入口也会调用上述 Rust 路径
bash scripts/run_rssr_web_auth_smoke.sh debug 18081 target/auth-smoke
```

`--web-auth-only` 使用已有静态 bundle，单独执行认证；常规聚合路径中的
`--skip-build` 仍仅跳过 dx bundle，服务仍然执行 Cargo。
`--release` 只选择 `target/dx/rssr-app/release/web/public`，不会为服务添加
`--release`。

readiness 保留 **30 次** curl，每次 connect timeout 2 秒、总 timeout 10 秒，
每次失败后等待 1 秒（含最后一次）；最坏可接近 330 秒加进程/调度成本。
取消会同时回收正在运行的请求及服务。Windows readiness 显式优先查找调用者
PATH 中的 `curl.exe`，防止系统目录搜索抢先选中另一份 curl。

HTTP 断言目前仅在 `scripts/run_rssr_web_auth_assertions.sh` 中保留一份。
它保持原有 curl/grep 合同且不自动跟随重定向；断言原始非零码继续向上传递
（例如服务在响应中退出时 curl 的 52/56）。服务日志 `rssr-web.log`、
readiness 日志 `rssr-web-readiness.log` 和断言日志
`rssr-web-auth-smoke.log` 分开，避免共享游标的并发写入。
为兼容不支持 Unicode 文件名参数的 Windows curl，断言先解析调用者 PATH
中的 curl，再在日志目录内传入 ASCII 相对 headers/cookie 文件名；日志仍保存
在用户指定的中文/空格路径，MSYS 的 next= 排除保持。

这只是阶段 1a：仍依赖 Bash/curl/grep，兼容构建入口仍依赖 Python 读取
Cargo JSON。后续 HTTP 迁移应在成本与语义验证后移除这一单份 adapter，
不得长期维护两套断言。新增测试使用 Python 标准库本地 HTTP peer，
它属于验收夹具，不进入产品运行时。迁移测量、平台边界和 CI 结果见
[交接记录](../handoffs/2026-10-05-web-auth-owned-process.md)。

2026-10-06 的清理等待及 Windows 文件路径阻塞修复、固定输入复测与最终 CI
状态见 [后续交接](../handoffs/2026-10-06-web-auth-blocker-fixes.md)。
2026-10-06 的退出观察延迟复测后，readiness 使用有限的 5/10/20 ms
早期 sleep，随后恢复 40 ms；Unix stop 使用 5/10 ms 后恢复 25 ms。
完整 1.5 秒进程组宽限与 ESRCH 提前结束条件保持。固定版本的分段归因、
30 组 A/B/C 耗时/CPU 结果和 Windows 补验见
[轮询优化交接](../handoffs/2026-10-06-web-auth-polling-latency.md)。

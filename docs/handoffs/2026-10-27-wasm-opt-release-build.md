# Windows Web release 的 wasm-opt 异常修复与完整构建验收

- 日期：2026-10-27
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 当前产品 HEAD：`1547425e`
- 相关 commit：本记录与构建入口修复同一提交；准确提交号见 `git log -1 -- docs/handoffs/2026-10-27-wasm-opt-release-build.md`。
- 相关 tag / release：N/A；未 push、打 tag 或发布。
- 状态：`validated`（本机 Windows Web release 构建及下述回归范围）

## 工作摘要

上一轮目录重构的 release 构建在 `wasm-opt` 报 `0xc0000409` 后仍返回 0。本轮复现并将
触发条件定位到优化后的 DWARF 调试信息处理，修正本地 release 入口遗漏的
`--debug-symbols false`，保留完整 `-Oz` 优化。使用匹配产品依赖的 dx 0.7.9 重新构建，
编译、wasm-bindgen、wasm-opt、资源打包全部完成，日志无 ERROR，最终产物通过浏览器回归。

## 影响范围

- 模块：独立 `scripts/release-ui` 验收工具、三个现有 Web smoke / SPA 启动脚本。
- 平台：本机 Windows x86_64 构建的 Web release；未修改产品 Rust、主题或跨端行为。
- 文档：目录维护入口、release runner 使用说明及上一轮交接的后续状态。
- workflow：未修改；GitHub Pages 与 CI 的 Web release 原本就显式传递 `--debug-symbols false`。

## 关键变更与原因

### 复现与故障边界

1. dx 0.7.10、Rust 1.99.1、缓存中的 Binaryen 127：原命令稳定记录 `wasm-opt` 退出
   `0xc0000409`，dx 却继续打包并退出 0。`BINARYEN_CORES=1`、仅省略 `--debuginfo`
   均未解决。
2. 缓存路径是 `~/.dx/tools/binaryen-129`，实际 `--version` 为 127。这与
   [dx 0.7.10 的源码](https://github.com/DioxusLabs/dioxus/blob/v0.7.10/packages/cli/src/wasm_opt.rs)
   一致：目录常量为 129，常见平台下载 URL 固定为 127；它还在优化失败后返回 `Ok(())`。
   这不是本轮推断的缓存损坏，未覆盖全局缓存。
3. 下载并校验官方 Binaryen 129 后，同样输入和参数仍崩溃。逐 pass 验证到最终校验完成，
   输出文件仍为 0 字节；不能把升级优化器作为修复结论。
4. 同一输入保持 `-Oz`，只添加 `--strip-dwarf` 并保留 `--debuginfo` 后成功；
   `--strip-debug` 也成功。这将触发条件缩小到 DWARF 处理，并非需要关闭优化。
   尚未定位上游具体 C++ 代码行，未声称修复 Binaryen 内部缺陷。
5. dx 的默认 `--debug-symbols` 为 true，且会覆盖 wasm-opt 的配置值，见
   [Web 构建源码](https://github.com/DioxusLabs/dioxus/blob/v0.7.10/packages/cli/src/build/web.rs)。
   因此按已有 CI 约定显式关闭 release 调试信息，而不是更改全局 Rust 参数或降低优化级别。

### 本地构建入口与失败判定

- `scripts/release-ui/plan.rs`：release 构建加入 `--debug-symbols false`；debug 参数不变。
- `run_web_spa_regression_server.sh`、`run_rssr_web_browser_smoke.sh`、
  `run_rssr_web_browser_feed_smoke.sh`：修正各自独立构建分支的相同遗漏。
- `scripts/release-ui/main.rs`：完成进程回收后逐行读取本轮 `web-build.log`；即使退出 0，
  若有 `wasm-opt failed` / `wasm_opt_failed`，仍将阶段判失败并阻止依赖阶段。
  此判断留在 bundle 验收处，不污染共用进程监督器；`--skip-build` 仍只记录复用。
- 新增黑盒用例：假的 dx 创建产物目录、打印优化失败及成功提示、退出 0，runner 必须失败；
  随后在同一日志目录成功重跑，不能被旧日志误判。fixture 仅用于验证判定规则，不代替真实构建。

### 与项目匹配的本地工具

全局 Scoop dx 仍为 0.7.10。本次从官方发布包提取独立 dx 0.7.9 到
`target/release-ui-tools/dioxus-0.7.9/dx.exe`，避免全局降级。下载经官方 SHA-256 核验：

| 文件 | SHA-256 |
| --- | --- |
| [dx 0.7.9 Windows x64 ZIP](https://github.com/DioxusLabs/dioxus/releases/tag/v0.7.9) | `0423b94dd36372d09936a9a288c4a6e7903a9f1bddd193b60bba9659890c87c4` |
| 提取的 dx.exe | `7395c0bbbe17e6aa86215634cf511387fd1ff8afba58661d0ac81fab28eb9fed` |
| [Binaryen 129 Windows x64 对照包](https://github.com/WebAssembly/binaryen/releases/tag/version_129) | `1405d2f51377859ccf5fcd2c59c0a8c5756373e691ca0eeb5219f646b743e3aa` |

Binaryen 129 仅用于临时对照，最终构建使用 dx 自带缓存的 127。该缓存中的 wasm-opt.exe
SHA-256 为 `51ea67fafe21b4f6ab247a1c4219779d6783a5e9bd5b1b223d8861be0a6c637d`。

## 验证与验收

### 完整 release 构建

本机可重复执行的明确入口：

```powershell
& .\target\release-ui-tools\dioxus-0.7.9\dx.exe build `
  --platform web --package rssr-app --release --locked --debug-symbols false `
  --verbose --log-to-file target/wasm-opt-fix-20261027/release-verified.log
```

- dx 0.7.9 (`3e43ffa`)、Rust 1.99.1、Binaryen 127。
- **64.47 秒完成，退出 0；完整日志没有 ERROR、`wasm-opt failed` 或 `wasm_opt_failed`。**
- 日志明确执行 `wasm-opt ... -Oz ... --strip-debug`，优化阶段耗时 19.653 秒。
- 之前用 dx 0.7.10 关闭调试信息的对照轮也完成了优化，但仍报告 CLI / 依赖版本不一致，
  不作为最终无错误构建凭据。
- 最终 JS：`rssr-app-dxhc250149c8de694ba.js`。
- 最终 Wasm：`rssr-app_bg-dxh5d5c1221f3ead334.wasm`，**4,723,575 字节**。
- Wasm SHA-256：`31f2f1fbf5f5e9aec4a3168746c7ddc9824c808475cc5ceb565f554460d90ac3`。
- 从当前 `index.html` 追踪 JS 再追踪 Wasm，`WebAssembly.validate` 为 true、模块可编译、
  36 个导出、无所检查的 DWARF 段；未按目录中旧文件的时间或名称猜测产物。

### 新产物浏览器验收

环境：`NODE_PATH=C:/Users/QQ/node_modules`，`PUBLIC_DIR=target/dx/rssr-app/release/web/public`，
两组顺序执行，使用隔离离线 fixture：

- `node scripts/browser/rssr_directory_offline_regression.cjs`：150 项通过，console errors 为空。
- `node scripts/browser/rssr_directory_window_acceptance.cjs`：22 项通过，page errors 为空。
- 覆盖目录分组、鼠标／键盘、手机浮层、600 篇虚拟窗口、全局分页、选择保持、滚动恢复、
  可变行高、坏消息后的继续使用。本轮未重新做 Android / WebView2 实机验收。

### 构建入口修复验证

- `cargo fmt --manifest-path scripts/release-ui/Cargo.toml --check`：通过。
- `cargo clippy --manifest-path scripts/release-ui/Cargo.toml --all-targets --locked --target-dir target/release-ui-runner -- -D warnings`：通过。
- `cargo test --manifest-path scripts/release-ui/Cargo.toml --locked --target-dir target/release-ui-runner`：2 项通过，含 128 种选项组合。
- `cargo build --manifest-path scripts/release-ui/Cargo.toml --locked --target-dir target/release-ui-runner`：通过。
- 既有 `acceptance.py` 定向执行 5 项黑盒用例：新增优化失败判定、完整顺序／只构建一次、
  所有阶段错误终止、skip／缺失产物、构建产物缺失／进程启动失败；全部通过（20.965 秒）。
- `bash -n` 检查三个修改的 Shell 脚本：通过。
- `release-ui --release --full --plan`：实际计划包含 `--release --debug-symbols false`。
- `git diff --check`：通过。

## 结果

当前源码的 Web release 已完成真实优化及资源打包，关闭上一轮 wasm-opt 未通过项。
这是本机 Web 构建与专项回归结论，不是完整发布、远端 CI 或所有平台验收。

## 风险与后续事项

- 上游带 DWARF 的 release 路径仍有本机可复现崩溃；需要此类调试包时应单独处理。
  本项目常规 release 已显式关闭 DWARF，debug 开发构建保留调试能力。
- 裸命令仍使用全局 dx 0.7.10；需要本次相同工具时使用上述路径，或在当前 shell 的 PATH
  前置该目录。没有替换全局工具、调整系统内存、清理其他任务或更改产品锁文件。
- runner 检查的是当前 dx 0.7 的已知优化失败标记，不能代替未来 CLI 升级后的工具验证。
- 未运行整个 release UI 聚合套件；守卫使用定向黑盒验证，实际产品使用上述两组浏览器回归。

## 给下一位 Agent 的备注

- 完整日志与最终产物回执：`target/wasm-opt-fix-20261027/release-verified.log`、
  `release-verified-console.log`、`release-artifact.json`。
- 原始失败、单线程／无 debug 参数、Binaryen 129／逐 pass、移除 DWARF 的对照日志均在同目录。
- 浏览器回执：该目录下 `release-regression/acceptance.json`、`release-window/acceptance.json`。
- 定向黑盒日志：`runner-acceptance.log`；fixture 证据目录为
  `target/release-ui-acceptance/1793110703641708200`。
- 官方工具下载保存在用户 Temp 的 `rssr-dioxus-0.7.9-20261027` 和
  `rssr-binaryen-129-20261027`；不把临时二进制提交到仓库。

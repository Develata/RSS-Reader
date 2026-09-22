# CI 模块并发、Rust 产物复用与 Actions 版本更新

- 日期：2026-09-22
- 作者 / Agent：Codex
- 分支：`main`
- 实现基线 HEAD：`1df684d`
- 相关 commit：`3bb0c8c`（CI / Rust 工具）、`0360591`（smoke 进程隔离）；本记录随 `docs: record CI modernization and measured validation` 提交
- 相关 tag / release：N/A；未 push、打 tag、发布或触发远端工作流
- 状态：`validated`（本地验证；真实 GitHub 调度与平台发布构建仍待验证）

## 工作摘要

重新核实当前工作流后保留已有 crate / UI 主题并发矩阵，消除三个 wasm job 的重复 Cargo 构建，更新官方 Actions 稳定 major，并补齐持续 actionlint、工具缓存隔离与 Release 并发身份。额外复现并修复静态 smoke 借用旧服务而误报通过的问题。开始时工作树干净；没有新增产品依赖、crate 或平台业务分叉。

## 影响范围

- `.github/workflows/ci.yml`、`release.yml`、`docker.yml`。
- `.github/actions/setup-{browser-tests,dioxus-cli,wasm-bindgen}/action.yml`；新增 `.github/dependabot.yml`。
- `scripts/wasm_contract_runner.rs`、`run_wasm_contract_harness.sh`、`run_static_web_small_viewport_smoke.sh`。
- 主线验证矩阵、UI 验收覆盖 / checklist、小视口 smoke 文档。
- 平台：CI 的 Linux / Web / Android，以及 Release 的 Windows / macOS / Linux / Web / Android；没有修改产品运行时。

## 关键变更

### 构建一次，分模块并发执行

- `workspace` 从 `cargo metadata --locked --no-deps` 取得全部 6 个 crate 及 `rssr-infra` 的 `wasm_*_contract_harness`；wasm-bindgen 工具版本直接取 `Cargo.lock`。没有路径选测或第二份手工 harness 列表。
- 原生 crate 并发上限 4、UI 五主题上限 3 不变；wasm 改成 `wasm-contract-build` 一次准备，三个 `wasm-browser-contract` job 并发上限 3。构建 job 先预热精确版本的工具缓存，执行 job 不再调用 Cargo。
- 交付目录位于各 job 的 `RUNNER_TEMP`，不落入 Rust target cache；避免恢复上次缓存的旧 bundle 后触发拒绝覆盖，也不将旧缓存当成本次产物。
- std-only Rust runner 新增 `--prepare DIR HARNESS...` / `--prebuilt DIR HARNESS`。Cargo 的 runner 参数提供本轮真实 artifact，避免 glob / mtime 猜测。只有集合完整才发布正式 manifest；已有输出目录、缺失 / 重复目标、歧义名称、符号链接、额外文件和无效 wasm 头都拒绝。失败清理本次未完成目录，已有目录不覆盖；中文、空格、引号路径和冷环境父目录均支持。
- 原有单 harness / 多 harness 接口保留。构建产物齐全与浏览器契约成功是两个独立结果。最终 `lint-and-test` gate 覆盖所有其他 jobs，失败、取消、意外跳过、空结果都不能通过。
- Python 只保留 Cargo JSON / lock TOML 事实解析与 GitHub needs 摘要；没有为了十几行解析增加通用手写 parser 或新 crate。Rust 负责产物校验与执行隔离，平台脚本保留启动和参数转发。

### Actions、缓存及发布流程

- 2026-09-22 联网核实所有 14 个外部 Action 的官方 release / ref、commit 和 runtime；checkout → v7、setup-node → v7、setup-java / cache → v6、Docker buildx / login → v4、metadata → v6、build-push → v7；原 artifact、Android、rust-cache、gh-release 已是最新 major。精确版本与官方链接见主线矩阵。
- 最新 JavaScript Actions 均原生 Node 24，删除强制 runtime 的过渡变量。Node 22、Java 21、Dioxus 0.7.9、wasm-bindgen 0.2.126 和 NDK 27.3.13750724 的项目兼容选择保留。
- Dioxus 缓存采用独立安装根及 OS / 架构 / 版本 key，不再覆盖全局 Cargo 安装登记；命中后仍通过绝对路径核对版本。原生 crate、Web、Android、wasm 构建 cache 分开，macOS Release 按 target 区分。
- CI `test-tools` 下载固定 actionlint 1.7.12，验证官方 SHA256 清单后执行；新增每周 grouped Dependabot Actions 更新，不自动合并或发布。
- Release 的 push tag / workflow_dispatch 同 tag 现在使用同一个 concurrency key，不取消正在发布的 run；Docker tag 同样不中断。各 workflow job 明确有限超时，Release Cargo / dx 使用 `--locked`；精确选择指定 NDK，不再排序选目录。Android licenses 消除 `yes` 的 SIGPIPE 误失败，下载 / Docker HTTP 探测增加单次超时，必需发布产物缺失时直接失败。
- permissions、签名要求、上传 / 发布条件未扩大。

### 小视口验收的进程归属

- HTTP / CDP 双端口预检；ready 与断言前后都检查本次 server / Chrome PID。探测请求有限超时，INT / TERM 走统一清理，自有子进程终止等待有限。
- 真实复现：原脚本启动的新 SPA 已报 Address already in use，却通过旧 SPA 完成 103 条断言并 exit 0。新脚本相同输入 exit 1；外部旧 SPA 仍存活且 HTTP 200。
- 此处只修验收可靠性，不改变 UI 或业务行为。Windows `.exe` 入口未在本轮实机运行。

## 验证与验收

- `/home/deve/go/bin/actionlint -verbose`：修改前后全部 3 个 workflow 通过；最终独立审查再验通过。实际执行新 CI 下载 / checksum / actionlint 步骤也通过。可选 pyflakes 不在本机 PATH，不计为通过。
- 全部 composite inline Bash、两个修改的 Shell 入口：`bash -n` / ShellCheck 通过。
- `cargo fmt --all --check`、`rustfmt --edition 2024 --check scripts/wasm_contract_runner.rs`、`clippy-driver --edition 2024 --test scripts/wasm_contract_runner.rs ... -D warnings`：通过；standalone 工具 **10 项测试通过**。
- 执行真实 workspace 元数据步骤：输出 6 crates、3 harnesses、wasm-bindgen 0.2.126；检查 gate 包含所有其余 jobs，所有 workflow job 均有 timeout。
- 将最终 CI prepare 步骤原文放入新的中文 / 空格 `RUNNER_TEMP` 执行，通过且三份产物与已验 bundle 字节一致；上传 / 下载目录与 producer / consumer 参数一致，位于 Cargo cache 之外。
- 执行工作流真实 gate 脚本：全部 success → 0；failure / cancelled / skipped / 空结果 → 1。
- 执行真实 NDK 配置脚本：指定版本与另一个更大版本同时存在时仍选指定目录；缺失则非零。licenses 控制变量：旧 pipefail 管道 exit 141，新写法 exit 0。
- 真实 `--prepare` 使用锁定 Cargo 构建三个 harness；产物 SHA256 与 Cargo 路径文件一致。实际 CLI：未知 Cargo harness → 101 且清理；重复 harness / 已有输出目录 / 未知 prebuilt → 1。prebuilt 在 Cargo 故意失败的 stub 环境仍进入 browser stub，保留退出码 7 并清理 profile，证明不依赖重新编译。
- 使用隔离下载并核验官方 digest 的 wasm-bindgen 0.2.126、匹配的 Chrome / ChromeDriver 151.0.7922.34，串行运行三个真实 prebuilt 浏览器契约：**refresh 19、subscription 3、config exchange 3，合计 25 项通过**。
- 本机驱动自动启动初试失败：TCP 探针成功后正式连接 reset / refused；trace 和官方源码定位到自动启动就绪窗口。SIGKILL 是上游失败清理，不能据此断言驱动崩溃。控制变量验证先启动相同 driver 并等待 `/status` ready，再通过官方 `CHROMEDRIVER_REMOTE` 接口执行上述 25 项；不是 stub，也未修改正式执行器掩盖失败。驱动与浏览器均已清理。该结果不验证 GitHub runner 上的自动驱动启动。
- 小视口真实浏览器：默认主题 **103 项通过**，360×800 / 1280×800，console errors 0；本次复用既有 release bundle，仅用于验证脚本改动，未将它当作新产品构建证据。
- 真实 HTTP / CDP 占用、Chrome 启动失败、断言失败、TERM 中断均验证拒绝 / 清理；外部 listener 保留。Dioxus 冷 / 热 cache、错误 PATH 同名工具、错版 / 版本前缀碰撞、版本命令失败、缺失缓存二进制均符合预期，全局 Cargo 登记未变。
- `git diff --check` 通过；已独立复核 workflow DAG、Actions 输入兼容、权限与文档一致性。

### 性能证据与适用范围

- 构建拓扑：三个独立 Cargo 契约构建改为一个构建，共享精确 artifact；运行仍分模块并发。一次工具缓存预热替代三个冷 job 同时安装，缓存失败仍可各自安装兜底。
- 本机热缓存三轮对照：三个单独 prepare 的**串行累计**中位 4.742 秒，合并 prepare 中位 1.589 秒。只测 Rust 工具编译、Cargo 启动 / 热检查和文件收集，不含浏览器，不等于远端三个 runner 并发墙钟提速。
- 三份 wasm 原始约 160 MiB；本机 ZIP/deflate 采样 level 1 为 1.864 秒 / 42,549,400 bytes，level 6 为 5.348 秒 / 37,211,518 bytes。保留 artifact 默认压缩，避免未测远端带宽时只优化几秒打包 CPU。没有声称这是 GitHub artifact 服务实测。

本地证据目录（未入 Git）：`target/ci-matrix-review/`、`target/ci-concurrency-review/`、`target/ci-action-audit/`。

## 结果、风险与后续事项

- 未运行完整 workspace tests / workspace Clippy / 全平台 release 构建：本轮没有产品源码或依赖变化，执行了受影响的工具单测、真实 wasm 契约、工作流脚本和 UI smoke；不沿用旧全量测试结果作为本轮新执行证据。
- 未运行真实 GitHub CI / artifact 跨 job 传输 / 远端缓存 / runner 并发调度，未修改 branch protection。新配置需后续用户推送后观察首次 run，尤其冷缓存和自动 driver 启动。
- Windows / macOS / Android 发布构建与原生 smoke 未运行；Action 版本兼容静态核对不等于实际打包验收。
- Actions 使用稳定 major ref，精确版本表仅为核实日期快照；后续 Dependabot PR 仍需验收。

## 给下一位 Agent 的备注

从 `docs/testing/mainline-validation-matrix.md` 查看 DAG、版本表和本地入口。不要将 `--prepare` 成功当作测试通过，也不要将本机累计准备耗时当作远端 CI 总耗时。GitHub 缓存 / artifact 的收益需真实 run 证据；当前所有变更仅本地提交，不包含发布授权。

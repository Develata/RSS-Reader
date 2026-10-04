# PR #20 审阅修复：工具选择、host 产物、背压与 Git provenance

- 日期：2026-10-03
- 作者 / Agent：Codex
- 分支：`refactor/release-ui-runner-rust`
- 本轮基线 HEAD：`6acbf7ace5fdfbfd3b9914e67028a425ef8b38ea`
- 相关 commit：本记录随审阅修复提交归档，完整 SHA 与结果见 PR #20 最终验收段
- 相关 tag / release：N/A
- 状态：`draft`；更新既有 [Draft PR #20](https://github.com/Develata/RSS-Reader/pull/20)，不 merge

## 工作摘要

用户明确批准续做独立审阅发现的三个条件性 P2 和一个 P3：先复现，再最小修复，
验证真实 Shell 入口、stdout 背压与取消、worktree provenance，推送同一 Draft PR 并核对最终 SHA CI。
仍在 Windows LAPTOP-H6JEOCF0 的 E:/gitclone/RSS-Reader 主目录，无环境切换或主仓库 worktree。

## 影响范围

- 模块：`scripts/run_release_ui_regression.sh`、`scripts/release-ui/main.rs` / `process.rs`、直接黑盒与原生夹具。
- 平台：本机 Windows MSVC + Git Bash；Unix 特定 Bash 身份及 SIGINT/SIGTERM 由已有 Linux CI 验证。
- 文档：runner 使用说明、本交接；无产品模块、根依赖、存储、安全规则、JS/CDP 断言改动。
- CI：沿用原 Linux/Windows job，直接测试随同执行；artifact 排除夹具编译缓存和二进制，保留报告/日志。
  没有新增工作流或另启任务。

## 关键变更

### 复现结果与修复

| 审阅项 | 修复前证据 | 修复 |
| --- | --- | --- |
| PATH / Bash P2 | 调用者选择的工具被 `/usr/bin` 同名工具替代；受控 Bash 启动 hook 的同一用例在旧二进制失败、新二进制通过 | Unix 不改 PATH、直接调用所选 Bash；Windows 仅追加基础工具目录，exec 当前 Bash 并将其用于裸 Bash 子调用；launcher 保留已设 RSSR_BASH |
| host target / stale binary P2 | 真 Cargo + 真 Shell：环境 target 和 config target 两种方式均运行旧夹具而 exit 101；删除旧夹具后 exit 127 | 用所选 rustc 的 host triple 显式构建及 exec 对应产物；Windows 反斜杠 script 路径也正确定位根目录 |
| stdout 背压 P2 | 4 MiB 阶段日志 + 无读者的父 stdout 管道，Ctrl+Break 后超过 5 秒仍不退出 | 一个有界控制台转发线程，try_send 不阻塞监督线程；文件读取也限量；运行状态通知同样排队；退出最多等 100 ms |
| `.git` 文件 P3 | linked worktree 的报告为 unknown，正常 packed-ref checkout 的完整 SHA 可读 | 使用 Git 支持的 rev-parse；Git 探测同样由 OwnedProcess 管理，支持取消、超时和后代回收 |

- 四个问题均确认；未将 Windows 单机运行伪装成 Unix 专用测试的本机证据。
- Windows 原生 PATH 经本机 MSYS 初始化可能被重新导入；工具优先级测试在 Bash 启动 hook 中建立
  调用者 PATH 并只执行一次，随后交给被测 bootstrap。旧/新 A/B 结果独立保留。
- 未应用已撤回的 Windows 外层 Job RAII 关闭建议；该 Job 包含 runner，维持进程生命周期句柄。
- 保留 TIME_WAIT 修正、PID 消失及活跃 listener 拒绝断言；没有放宽为仅检查退出码。
- 日志文件保持完整；堵塞或高负载时允许丢弃控制台副本。队列最多 128 KiB + 单个处理中块，
  不逐阶段生成输出线程，不建立 Shell/Rust 事件协议。
- stdout worker 保持 stdout lock，避免 Rust 退出清理在主线程重新 flush 阻塞；
  对照 [Rust stdio cleanup 源码](https://doc.rust-lang.org/src/std/io/stdio.rs.html) 的 try_lock 行为，
  并以双平台真实无人读取管道用例验证，不仅靠源码推断。

## 验证与验收

### 修复前

- `review-reproduction.log`：4 个 focused tests 在审阅基线失败，包含三个 target 子场景和 5 秒取消超时。
- `review-path-original.log`：更新后的受控 Bash PATH 用例对保留的旧二进制仍失败。
- 旧源码/二进制位于忽略目录 `target/release-ui-acceptance/1791048841357396100/`，未新增第二套受版本控制的实现。

### 修复后本机

- 根 `cargo fmt --all --check`：通过。
- 独立 CLI `cargo fmt --manifest-path scripts/release-ui/Cargo.toml --check`：通过。
- `cargo clippy --manifest-path scripts/release-ui/Cargo.toml --all-targets --locked --offline --target-dir target/release-ui-runner -- -D warnings`：通过。
- 对应 `cargo test`：3 项单元测试通过，原 128 组选项组合仍覆盖；`actionlint -color` 通过。
- `python -X utf8 scripts/release-ui/tests/acceptance.py --bash C:/Users/QQ/scoop/apps/git/current/bin/bash.exe`：
  18 项中 17 通过、1 项 Unix 专用测试明确 skipped，61.28 秒（与实际聚合并行，非性能基准）。
  日志 `target/release-ui-validation/review-acceptance-final.log`。
- 真实 Shell target/config/旧产物测试还验证调用者 Cargo wrapper 三次均被选用；测试项目路径含中文和空格。
- 新 Git 探测取消测试验证其三层后代回收；linked worktree、separate gitdir、packed refs、detached HEAD 通过。
  测试自建临时 Git 仓库在本仓库 target 下，不改主仓库工作树配置或用户全局 Git 配置。
- stdout 背压用例不读取父 stdout，仍在 5 秒界内退出，终态 interrupted、后续 blocked、完整文件日志超过 4 MiB，
  子孙 PID 消失且端口不能连接并可重绑。

### 实际聚合与远端

- 实际 Shell `--release --full --skip-external-feed --no-serve --port 18191 --web-port 19181` 聚合完成，
  208.04 秒（含首次 host 目录 CLI 构建，并行直接测试），exit 0 / completed-with-skips。
  自动门禁、51 项 wasm 契约、一次 Web bundle、登录、五主题、278 项小视口与本地 feed 流程通过。
  外部 proxy-feed 仍明确 skipped，SPA 按 no-serve 跳过；日志 `target/release-ui-validation/aggregate-review-fixes/`。
- 收尾所有阶段 PID 已退出，7 个端口可重绑；只读进程查询未发现本轮服务/浏览器残留。
- 最终完整提交 SHA 与 Linux/Windows CI 结果将在 PR 正文和 Checks 记录；只接受匹配该 SHA 的结果。

## 结果

- 直接测试与实际聚合已通过本机覆盖范围；本轮修复与本记录一起提交到既有 Draft PR，
  最终远端 SHA 与 CI 结果见 PR 正文及 Checks，避免把提交前证据写成后续 SHA 已验收。
- 本轮没有改用户原有 22 项删除、`.workbuddy/` 或两份目录任务收尾补记；25 项哈希/MISSING 与启动快照一致。

## 风险与后续事项

- 外部 feed DNS/HTTPS 与既有 wasm-opt 降级仍单独保留，未修改网络设置、SSRF 或断言。
- `--skip-build` 来源校验仍是原有语义，本轮不扩展；Git HEAD 不证明工作区 clean 或旧 bundle 来源。
- 无人读取管道时控制台不是完整日志；日志磁盘故障、Unix SIGKILL、逃逸进程组、断电的边界保持原记录。
- host triple 对 CLI 显式生效，产品命令仍保留调用者 target 设置；没有新增依赖或 workspace 成员。

## 给下一位 Agent 的备注

- 先读本记录及 [初轮交接](2026-10-03-release-ui-runner-rust.md)，勿把历史 CI 通过当作后续 SHA 已验收。
- 只暂存本任务显式路径，保留原有 dirty 内容；不 stash/reset/clean，不切主目录，不 merge。

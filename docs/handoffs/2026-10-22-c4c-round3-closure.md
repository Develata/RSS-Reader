# C4c 第三轮复核、修复与 C6 准入

- 日期：2026-10-22
- 作者 / Agent：Codex
- 分支：`v0.2-architecture`
- 当前 HEAD（本轮提交前）：`e8853b62c93551cd720705b6e235eb852b2f395a`
- 相关 commit：`a13a1e5`（复核修复）；远端失败诊断与结果补记随本记录提交；原固定审阅快照 `28859c2`。
- 相关 tag / release：N/A
- 状态：`validated`（本轮复核与修复复验通过，不等于独立评审或发布）

## 工作摘要

用户明确选择“由你逐项复核，通过后进入 C6”。没有收到第三轮外部报告，本 agent 按既定提示词审查固定增量与证据，确认两项测试驱动缺口和一项文档遗漏，修复后通过定向反例与完整浏览器长验收。详细逐项判断见 [第三轮复核报告](../reviews/2026-10-22-c4c-round3-review.md)。

## 影响范围

- 模块：浏览器验收驱动、驱动反例测试。
- 平台：Windows Chrome / Node；CI `test-tools`。
- 文档：阶段 C 计划、release UI 覆盖矩阵、复核报告与本交接。
- 工作树：`target/audit-20261009/worktree`；主检出与 `b2-baseline` 未修改。没有产品 Rust、依赖或数据格式变更。

## 关键变更

- HTTP 网络例外比较完整 origin；热重载严格对应 `ws/http` 或 `wss/https`，畸形 URL 不抛出分类器异常，也不放行；故障注入日志有 URL 时同样核对 origin。
- 场景级异步清理错误纳入最终运行错误；仍尝试剩余清理，避免留下总报告 pass。
- 新增实际驱动函数与运行入口的反例回放测试，CI 安装 Node 24 后执行。
- 覆盖矩阵明确常规 CI 短场景与手动长场景的区别。

## 验证与验收

- `node --test scripts/browser/server_acceptance.test.mjs`：修复前 1 passed / 4 failed；修复后 5 passed / 0 failed，覆盖 origin、socket、故障注入、初始化/最终收尾、场景异步清理。
- `node --check scripts/browser/rssr_server_acceptance.mjs`、`actionlint .github/workflows/ci.yml`、`git diff --check`：通过。
- 文档审计：36 份现行文档、226 个本地链接，0 错误，旧证据输出未覆盖。一次可选 PyYAML 接线检查因当前 Python 没有 `yaml` 模块未执行，随后使用实际已安装的 actionlint 完成 workflow 检查，未安装新依赖。
- `RSSR_BUILD_ID=c4c-round3-e8853b6`，Cargo 上限 2 jobs：`dx build --platform web --package rssr-app --locked --no-default-features`；`cargo build --locked -j 2 -p rssr-web -p rssr-infra --example browser_fixture_library`；`cargo build --locked -j 2 -p rssr-web`：均通过。app 没有默认 feature，未启用 `pages-demo`。
- Git Bash：`bash scripts/run_server_web_regression.sh --suite acceptance --long --debug --skip-build --server-bin target/debug/rssr-web.exe --fixture-bin target/debug/examples/browser_fixture_library.exe --static-dir target/dx/rssr-app/debug/web/public --port 8147 --log-dir target/handoff/evidence-c4c-round3/browser`：退出 0，27/27 场景、108/108 检查、0 skipped、0 意外控制台错误、无清理错误。
- 本轮证据在 `target/handoff/evidence-c4c-round3/`。原 24/24 CI 仍指 `509ea35` 的 run `39785231034`，不冒充包含本轮驱动修改的远端结果。
- 未重跑产品全量 Rust / WSL / 变异压力测试；代码未变，原结果及其受测版本已在复核报告中区分。

### 后续远端复验（C6 期间）

- source `129996af85798c07c623255be88e18a5706426d1` 的 [CI run 39816994053](https://github.com/Develata/RSS-Reader/actions/runs/39816994053)，显式 `acceptance_long=true`，最终 24/24 jobs 成功。
- 首次默认主题长验收有 3 项失败：`reader-recovers-without-localizing`、`settings-read-fails-at-start`、`http-backend-only` 均在 `ApiTraffic.settled()` 等待 `GET /api/v1/settings` 完成时超时。该次报告 24/27，意外控制台错误为零；没有将它解释为产品已修复或放宽等待条件。
- 三个场景在本机定向回放全部通过；远端只重跑失败 job 及汇总 gate，第二次完整长验收 27/27，通过日志逐项核对。超时根因尚未确定，留作驱动稳定性跟进。
- 新增失败时的 `*-traffic.json`：包含请求 ID、loader ID、完成状态和当前 frame；原文本摘要仍保留，断言与静默窗口不变。5 组驱动反例测试再次通过。
- 首次失败 artifact 已保存并校验 SHA-256：`4570f41b6b570e79ef350c66566dec68c1ec98e716bdc8a3f1873cc32e0cb573`，位置 `target/handoff/evidence-c6/ci-default-39816994053.zip`。
- 第二次 artifact ID `12444401389`，GitHub 报告 SHA-256 `f5eff726bcf4c94acff19e18f424cdb335928ebcb6e520df48023f7e119755fa`；本机保存命令被自动审批拦截（需批准但当前策略不允许申请），未绕过。通过 GitHub job `119965198186` 的完整日志核对 27 项 pass 与退出成功，未声称本机重新校验该 ZIP。

## 结果

原快照有两项应修；本轮修复后没有剩余阻断或应修项。按用户明确授权进入 C6；C6 的实现、验证与交接另行记录。

## 风险与后续事项

- 外部独立复审、真实 bfcache、人工视觉/辅助技术、Android/macOS 实机和真实公网部署仍没有本轮证据。
- 远端首次三个网络收尾超时未确定根因；重跑通过不等于稳定性问题已经修复。失败时增加的 JSON 诊断是后续定位入口。
- C6 完成前不得把发布包、镜像或迁移验收描述为已交付；阶段 C 最终独立评审仍是独立事项。
- 未合并、未打 tag、未发布；本地旧复审 ZIP 仍固定于 `28859c2`，不包含本轮修复。

## 给下一位 Agent 的备注

本轮授权取代此前“停在 C6 前”的工作停止点；它没有把本 agent 自检改称外部独立评审，也不扩展为发布授权。以本记录的命令与新的修复提交为准。

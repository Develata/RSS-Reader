# C4c 第三轮复核与修复

- 审阅者：Codex（本工作 agent；不是 DeepSeek / Claude 或另一 agent 的独立评审）。
- 固定原快照：`28859c2fea970467030b07d65e08feeca53eb276`，主要增量 `53f6c9d..28859c2`。
- 开始工作时 HEAD：`e8853b62c93551cd720705b6e235eb852b2f395a`，相对原快照只增加两份复审材料文档。
- 用户本轮明确选择“由你逐项复核，通过后进入 C6”；这取代本轮此前的停止点，不授权合并、打 tag 或发布。
- 方式：读取固定差异、沿当前相同源码调用链检查、复核原报告和原始证据；实际执行下面列出的反例与验收。未启动其他模型。
- 结论：原快照 **FAIL**（2 项 SHOULD_FIX、1 项 NIT）；上述修复落地并复验后 **PASS / 无剩余阻断或应修项**，可按用户本轮授权进入 C6。本轮不是独立外部评审或发布验收。

## 本轮发现

### C4C2-R3-01 / SHOULD_FIX：origin 比较遗漏协议

原快照 `scripts/browser/rssr_server_acceptance.mjs:1042` 的 `sameOrigin` 仅比较 `URL.host`。页面 `http://localhost:8091` 与错误 `https://localhost:8091/api/v1/settings` 被当成同源，预期 401 被忽略；热重载同样接受不对应页面协议的 socket。`provoked` 分支也没有对有 URL 的日志检查 origin。

本轮直接执行驱动的原函数，反例失败已保留。修复比较 HTTP(S) 的完整 origin；仅热重载规则把 `ws/http`、`wss/https` 成对对应，非法 URL 按意外错误处理；有 URL 的故障注入日志同样检查 origin。这是验收门禁漏报，未发现历史成功报告混入该类条目。

### C4C2-R3-02 / SHOULD_FIX：场景级异步清理失败被吞掉

原快照 `scripts/browser/rssr_server_acceptance.mjs:1138` 对场景的 cleanup Promise 使用空 catch。全局启动/收尾已修正，但异步场景清理（例如删除新文档脚本、移除 Fetch 规则）拒绝时仍可能输出总报告 `pass`。本轮对实际 `run()` 注入拒绝的清理回调，得到该反例；不是声称历史 CI 发生过此失败。

修复把场景回调、额外页面、网络规则复位和焦点复位逐项尝试；任何失败记入运行错误，其他清理仍继续，最终报告与退出状态都失败。回放也覆盖初始化与最终清理失败，确认原已修复分支仍成立。

### C4C2-R3-03 / NIT：CI 长验收说明遗漏手动选项

`docs/testing/release-ui-coverage-matrix.md` 原来写“CI 不含三个长场景”，与已加入的 `acceptance_long` 手动输入不符。改为常规 CI 跳过、手动 CI 可启用。

## 前轮逐项结果

| 项目 | 状态 | 本轮核对 |
| --- | --- | --- |
| C4C2-R2-01 | RESOLVED | `drafts::follow` 同步停止导航、清除过时副本并返回错误；shell 保留内存修改并设置全局未保留提示。核对了实际表单发布链、两个原生测试、n5/m9 原变异补丁及行为失败日志。 |
| C4C2-R2-02 | 原快照 PARTIALLY，本轮已修复 | 类型/来源检查已正确；协议遗漏由 R3-01 修复。新增回放覆盖异源、协议、端口、异常来源、畸形 socket 和故障注入。 |
| C4C2-R2-03 | 原快照 PARTIALLY，本轮已修复 | 初始化和最终收尾的失败报告正确；场景级异步清理由 R3-02 补齐。 |
| C4C2-R2-04 | RESOLVED | 两种启动提示与按钮使用 `VISIBLE`（计算可见性与非空盒），重试/关闭通过 CDP 鼠标事件点击。 |
| C4C2-R2-05 | RESOLVED | 计划 C4b 第 4 条已写入离开期间跟随修改、保存/关闭清除、用户自行重载的例外；实现没有恢复旧的 30 秒计时器。 |
| C4C1-10 | RESOLVED | 现行命令与正式 Web / 演示边界已改写；本轮文档审计核对 36 份现行文档、226 个本地链接，退役文件引用与链接错误均为零，历史记录有明确范围。 |

第一轮 C4C2-01～11 的回归检查：关闭设置页清除草稿、后续修改跟随导航、零写入读取、可见提示与按钮、请求起止静默窗口、控制台分类、旧任务不修改新表单、按新 loader 记录重载前后轮询、traffic artifact 上传、运行失败报告和 1280×800 实际视口均仍有对应实现与断言。控制台及清理的残留由上述修复补齐。原变异资料的构建成功与行为断言失败分别核对，没有把编译失败当作变异成功。

## CI、资料库关闭与时间规范化

- CI：四组 wasm feature、Android app / CLI、长验收参数转发与失败 artifact 保存均有实际接线；普通 CI 不等于长验收。apt 修正保持原 suites/包，只切换官方源并设超时重试。新增驱动反例测试接入 `test-tools`。
- release UI runner：三组目标与当前 Cargo metadata 一致，`planned_cargo_tests_exist_in_workspace` 读取真实 workspace，而非只用假 Cargo 通过编排测试。
- 资料库关闭：`Queue::stop` 的 sender 在锁下取走，后续准入被拒；reader 的 join 持有 workers 锁，重复 close 必须等首个 join 完成；持有 writer sender 的 close 调用只有在 reader 全部结束后才释放它。其他 close 即使先取得 writer 的 join 锁，也会等待该 sender 释放。read/write 任务仍能排空；reader 出错也执行 writer join。外部进程持有连接不在本 store 的清理保证内；重复 close 不重复传播首个关闭错误是既有明确契约。未发现新增死锁或提前返回路径。
- 原 4/500 → 0/500 CLI 证据、Windows / WSL 定向测试和远端测试支持关闭修复，但不构成并发正确性证明；本轮没有重复这轮压力测试。
- UTC：补充裸 `UTC` 只影响测试 normalizer，产品显示和基准 transcript 未变；非法日期和 `UTC+invalid` 仍拒绝，其他文本保留，因此时间字段之外的差异不会被整体吞掉。原三时区验证作为作者证据引用，未在本轮重跑。

## 本轮亲自执行与边界

- `node --test scripts/browser/server_acceptance.test.mjs`：修复前 1 passed / 4 failed；修复后 5 passed / 0 failed。测试执行驱动原函数和原 `run()`，替身仅用于控制故障，不能称为浏览器验收。
- `node --check scripts/browser/rssr_server_acceptance.mjs`、`actionlint .github/workflows/ci.yml`、`git diff --check`：通过。
- 现行文档审计：36 份、226 个链接、0 错误；副本输出到本轮 evidence，未覆盖 C4c-3 原记录。
- 当前源码重新构建页面、服务与夹具，统一 `RSSR_BUILD_ID=c4c-round3-e8853b6`；完整浏览器 `acceptance --long` 退出 0，27/27 场景、108/108 检查、0 skipped、0 意外控制台错误，最终报告没有运行/清理错误。
- 本轮证据：`target/handoff/evidence-c4c-round3/`。原远端 CI 为 `509ea35` 的 run `39785231034`（24/24），不能称为包含本轮驱动修复的 CI。
- 未验证：独立外部评审、真实 bfcache 命中、人工视觉/辅助技术、Android/macOS 实机、真实 HTTPS 公网部署及 C6。当前检查不宣称这些完成，也不授权发布。

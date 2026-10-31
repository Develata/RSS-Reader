# main 补充回归与 Windows 实际宿主检查

- 日期：2026-10-31
- 作者 / Agent：Codex
- 分支：`codex/main-regression-20261031`，复用 E 盘接续工作树
- 验证基线：main `60b4c6f175fc3a0cc166818307dd213c9c67c852`，开工时与远端一致
- 相关 commit：本记录所在提交仅保存验证和交接文档；产品源码未改
- 相关 tag / release：无
- 状态：`validated`

## 工作摘要与审查结论

按用户要求重新测试当前整合结果，重点检查 Reader 切文与回执归属、图片浮层、快捷键、返回和位置恢复，以及工作区、来源编辑和 Settings 的异步保存。

**本轮所测范围内未复现新的产品缺陷，没有修改产品实现。** Reader 的读取与标记写入仍按访问归属处理，正文保持不替换；工作区偏好最终持久化、导航期间写入和设置逐字段回填的回归均通过。这是本轮代码自审和实际执行结果，不是第三方独立 review，也不代表所有平台或输入组合均无缺陷。

## 影响范围与关键变更

- 仅新增本交接、精简回执并更新文档入口；没有改动 Rust、产品 JS、CSS、测试脚本、依赖、workflow 或资料库格式。
- 新建本地分支承载文档，沿用 `E:/gitclone/RSS-Reader-worktrees/native-back-policy`；未新建或清理工作树。
- 主 checkout 的 HEAD、分支、status、index 与 41 个变更/缺失路径均与本轮开工快照相同。

## 验证与验收

原始日志、报告、截图、隔离资料库、固定测试输入和本地启动脚本：`target/main-regression-20261031/`。[精简回执](../testing/evidence/2026-10-31-main-regression/receipt.json)保存精确源码、包身份、报告散列和覆盖范围。

| 命令 / 场景 | 本轮结果 |
| --- | --- |
| `cargo test -p rssr-app --locked` | 440 项通过，1 项既有性能测量忽略；含独立 Reader 规则 35 项 |
| `cargo test --workspace --locked` | 1149 项通过、0 失败、4 项既有忽略；包含上面的 app 用例，不重复相加 |
| `node --test scripts/browser/api_traffic.test.mjs scripts/browser/server_acceptance.test.mjs scripts/browser/workspace_response_probe.test.cjs` | 16/16 |
| `node scripts/browser/workspace_response_probe.browser.cjs` | 13/13；两种 EOF 前取消均拒绝，实际 EOF 后取消通过，页面异常 0 |
| `node scripts/browser/rssr_reader_host_acceptance.cjs` | 1280 / 390 两种宽度，共 6/6；旧图片消息、浮层清理、快捷键、标记门禁及正文保持，页面/控制台异常 0 |
| `node scripts/browser/rssr_workspace_acceptance.cjs` | 固定输入后完整 18/18，页面异常 0；包括偏好保存顺序、来源编辑、跨页写入和重载后的持久化 |
| `node scripts/browser/rssr_native_preparation_acceptance.cjs` | 正式 Web 1536 / 390，共 8/8；逐字段保存、保存中继续编辑、下载合并、UTF-8/BOM 与过期文件读取，页面/控制台异常 0；脚本名称中的 native 不代表原生验收 |
| `cargo build -p rssr-app --locked`；`run-windows.ps1 -RunName windows-current-01` | 当前源码新构建的 Windows EXE、实际 WebView2 `156.0.4314.50`，1280 / 390，共 8/8；返回、阅读位置、图片优先关闭、快捷键和监听卸载，页面/控制台异常 0 |
| `python target/main-regression-20261031/collect-receipt.py` | 重新核对源码等价、产物与报告身份、计数、取消断言及主 checkout 保全，生成精简回执 |
| `python target/main-regression-20261031/check_docs.py`、`git diff --check` | 文档本地链接、锚点、JSON 与 diff 检查通过 |

Rust/Cargo `1.99.1`，Node `24.22.1`，Chrome `155.0.8059.40`。Web 复用已验明的 `dev-801f6337e81f-dirty-8f2160d303f9` 产物，产品输入与 `97d75438` / 当前 main 相同，29 个静态文件及服务器散列重新核对；未冒称 Web 包本轮重建。Windows 则由本轮 main 源码重新构建并复制到独立运行目录，100 篇合成文章、6 个 loopback 来源，测试进程结束后退出。

### 测试执行中断及处理

首次工作区测试在 `queryStability` 重置夹具时遇到 Windows `spawn EBUSY`，此前 5 个场景通过。当时 `cargo test --workspace` 正在重建同一个 `target/debug/examples/browser_fixture_library.exe`；该 EXE 的修改时间和散列随构建变化。这是本轮并行执行共用构建输出造成的测试输入冲突，不能算作产品保存或页面故障。

将服务器、已编译的夹具和已核验的 Web 静态文件复制到本轮 `inputs/`，通过 `RSSR_WEB_BIN`、`RSSR_FIXTURE_BIN`、`PUBLIC_DIR` 指向固定副本，换用全新隔离资料库后，完整 18 场景通过。保留首次失败日志与通过日志，未放宽断言、增加等待上限或改动产品代码。后续同时运行 Cargo 与浏览器验收时，应使用固定输入副本，避免共享可执行文件。

## 风险与未覆盖范围

- 4 项忽略分别为 Entries 投影测量、refresh ticket 测量、外部正文语料校准和 HTML 压力计时；本轮没有将其记为通过。
- Android 真机可连接，但本轮未安装测试包、操作设备或重跑实际宿主；不能用 Windows / Web 通过代替 Android 系统返回验收。
- 未覆盖 macOS/Linux GUI、物理 IME、辅助技术、正式签名升级和真实用户数据迁移。隔离 SQLite 不使用日常资料库。
- 本轮没有重新触发远端 CI、Docker 或 Pages；上一批精确提交的结果仍归[合并交接](./2026-10-31-workspace-cancel-merge.md)。

## 给下一位 Agent 的备注

本批是验证交付，不是新的原生模块实现。继续从[维护状态](../current-status.md)和[正文渲染边界](../design/reader-rendering-boundary.md)选择有具体复用阻碍的下一项；不要为“找到 bug”而改变既有语义。证据和原工作树均保留。本地文档提交后不自动合并、push 或发布。

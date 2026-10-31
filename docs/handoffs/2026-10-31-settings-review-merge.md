# Settings 规则边界复审与整合

- 日期：2026-10-31
- 作者 / Agent：Codex
- 分支：`main`，复用 E 盘接续工作树
- 复审 HEAD：`51a4fa827a7f26ad825a84913780962485f25d92`
- 合并 HEAD：`2571f56a58e96a567f0fc20ef53641dd1874f661`
- 相关 commit：实现 `ec4a605b`，原批回执 `51a4fa82`，复审 `950c9665`；本记录后续提交仅补整合回执
- 相关 PR：[PR #60](https://github.com/Develata/RSS-Reader/pull/60)
- 基线：`8dcf710c21ebfd191c3662790c9354184f436138`（本轮 fetch 后 main 仍为此提交）
- 相关 tag / release：无新 tag 或发布
- 状态：`validated`（已合并；PR 与合并提交的 main 新检查均通过）

## 工作摘要与影响范围

用户授权重新 review、测试这批 Settings 改动，有问题先修复，再 push 并尝试合并。本轮没有发现可操作缺陷或行为变化，没有继续改产品代码；新增正式 Web 补验和整合记录。原实现、Windows 宿主和编译结果仍按当时范围见[规则边界交接](./2026-10-31-settings-rule-boundary.md)。

## Review 结论

- 固定上述 base/head，逐段核对 Signal 发布顺序与订阅、stored/submitted/pending 字段归属、保存成功与失败恢复、暂存读取释放时的 App 版本判定、预设/CSS 预览/文件请求身份和数值提示代次。
- 回填策略只计算结果；session 保持原状态来源、门禁、预览和任务作用域。等值回填更新未编辑数值提示、只在 CSS 真正变化时使导入失效、预设相等判定等行为保持原样。
- 直接引用的 CSS 校验仍是原 domain 函数；独立编译入口使用生产模块及真实 API/application/domain 类型，没有替身实现或新 crate。现有 CI job 的步骤条件和 actionlint 结果已复核。
- review 无可操作问题，未为没有复现的推测添加产品修复。这里是 agent 自审，不表述为外部独立审批。

## 验证与验收

证据目录：`target/settings-review-20261031/`。

| 命令 / 场景 | 本轮结果 | 证据 |
| --- | --- | --- |
| `cargo test -p rssr-app --locked settings_page` | 78/78 通过（76 项 Settings 页面、2 项 Shell） | `settings-tests.log` |
| `python scripts/check_settings_rule_boundary.py` | 18/18 通过，无 UI 框架依赖 | `independent-rules.log` |
| dx 0.7.9 `build --platform web --package rssr-app --release --locked --debug-symbols false --verbose` | 通过；普通输出与详细日志均没有优化器错误 | `web-build.log`、`web-detail.log` |
| `cargo build -p rssr-web --locked` 与既有 fixture example | 通过；同一 `RSSR_BUILD_ID=settings-review-51a4fa82` | `server-build.log` |
| `rssr_server_acceptance.mjs --only <下述 10 个场景>` | 正式 Web 10/10 通过，无未预期控制台异常 | `web/assertions.json`、`web-acceptance.log` |
| `cargo fmt --all --check`、文档链接和差异检查 | 通过 | `format.log`、`docs-check.log` |
| 原批完整源码散列复核 | 与原实现验收源码逐项相同；这次没有产品变更 | `review.json` |

Web 使用真实 `rssr-web`、SQLite 合成资料库、单独 Chrome profile；每个场景创建新资料库，未打开用户日常数据。测试产物先复制到本批 `inputs/`，身份见 `web-identity.json`，启动与完整参数见 `run-web.ps1`。实际运行的场景为：

- `retired-density-layout-and-drafts`
- `settings-preview-explicit-save`
- `build-mismatch-keeps-edits`
- `draft-storage-unavailable`
- `restored-draft-then-sign-in`
- `stopped-departure-restores-once`
- `stopped-departure-then-saved`
- `later-edit-not-kept-stays`
- `settings-read-fails-at-start`
- `failures-and-unknowns-visible`

网络失败、丢失回包、版本不匹配等由既有 CDP 拦截器注入，其预期错误与真实异常分别记录。原批 app 467 项及 Windows 实际宿主 8 项不是本轮重跑结果；本轮没有 Android 实际宿主、macOS/Linux GUI、系统 IME 或物理键盘验收。

## 当前状态与后续

- PR #60 的 3 个提交、15 个文件已合并。合并前 [CI 41217318062](https://github.com/Develata/RSS-Reader/actions/runs/41217318062) 为 **36/36 成功**；[Pages PR 构建 41217318067](https://github.com/Develata/RSS-Reader/actions/runs/41217318067)成功，deploy 按 PR 规则跳过。新增无框架编译步骤在 Linux CI 中 18/18 通过，日志为 `pr-api-job.log`。
- 合并前 review submissions / threads 均为空，main 无额外 rules；自审无待处理问题，没有伪造外部审批。通过 `expected_head_sha=950c9665db8b43b6b968e3c5b1fd75dc0eb9b6a9` 采用 merge commit 合并，未绕过失败检查。
- PR 测试合并引用 `b6705e2d`、head 和最终合并提交的 tree 都为 `ad72f883bb65c471900b537da94720d9239831cc`；实际合并父提交为上述 base 和 head，无冲突改写。
- 下载 PR UI artifact `13048679199` 并核对 GitHub SHA-256 `d4a307563a2d953a8357e40ed74768941bdafbdeb89c3012a0f961f30074af7f`。正式 Web 完整验收 **26 个短场景通过、3 个长场景按配置跳过**，未预期控制台异常为 0；证据 `pr-ci-ui-default.zip`、`pr-acceptance.json`。这与本轮本地 10 场景分开记录。
- 合并提交的新 [main CI 41218144589](https://github.com/Develata/RSS-Reader/actions/runs/41218144589) 为 **36/36 成功**；下载 UI artifact `13048819892` 并核对 SHA-256 `70142649d4456b8c49080bd86ef995cf71770160f553de9f7cc901e813704832`，完整正式 Web **26 个短场景通过、3 个长场景跳过、未预期控制台异常 0**。证据 `main-workflows.json`、`main-ci-ui-default.zip`、`main-acceptance.json`，与 PR 结果分别保存。
- [Docker 41218144723](https://github.com/Develata/RSS-Reader/actions/runs/41218144723) 成功，4 个 job 成功、扩展原生包 job 按条件跳过。容器 **9 项**、Linux 包 **6 项**均通过；两个验收 ZIP 均按 GitHub SHA-256 校验，JSON 中 build id 均为 `git-2571f56a58e96a567f0fc20ef53641dd1874f661`。GHCR 登录、标签与镜像推送步骤全部跳过，不是镜像发布。
- [Pages 41218144773](https://github.com/Develata/RSS-Reader/actions/runs/41218144773) 的 build / deploy 均成功。
- E 盘接续工作树已切回既有 main 并快进；主 checkout `feat/reading-workspace-20261024` / `020f9d40` 的 HEAD、状态、index 与 41 个改动/缺失路径仍与 `primary-before.json` 一致，回执为 `primary-preservation.json`。
- 不清理工作树、旧分支或历史证据，不发布新版本。后续功能批次仍按真实复用阻碍安排，主题导入/预览宿主边界尚未实施。

本节后续文档提交只记录已核验的事实；最终 main 的新增工作流按其精确 SHA 再检查，回执保存在 `final-workflows.json`、`final-state.json`，不为回写自身 SHA 循环修改文档。`SHA256SUMS` 索引本轮本地与远端证据。Git 切换时 3 个 Rust 文件正常从 LF 检出为 CRLF；规范化内容与 Git blob 不变，最终核对分别记录内容和换行，不把原始字节变化误写成产品改动。

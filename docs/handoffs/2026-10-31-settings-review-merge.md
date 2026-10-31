# Settings 规则边界复审与整合

- 日期：2026-10-31
- 作者 / Agent：Codex
- 分支：`codex/settings-rule-boundary`，复用 E 盘接续工作树
- 复审 HEAD：`51a4fa827a7f26ad825a84913780962485f25d92`
- 相关 commit：实现 `ec4a605b`，原批回执 `51a4fa82`；本记录 commit: pending
- 基线：`8dcf710c21ebfd191c3662790c9354184f436138`（本轮 fetch 后 main 仍为此提交）
- 相关 tag / release：无新 tag 或发布
- 状态：`validated`（本地复审与补验通过，远端状态见后续回执）

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

- 本地复审通过，随后推送分支并创建 PR；只在精确 head 的检查通过且无未解决 review 后尝试合并。
- 合并后核对 main 的新 CI，更新本记录和维护状态。暂不将待运行的远端检查写成已通过。
- 主 checkout `feat/reading-workspace-20261024` / `020f9d40` 的 HEAD、状态、index 与 41 个改动/缺失路径独立留存于 `primary-before.json`，后续以 `primary-preservation.json` 核对。
- 不清理工作树、旧分支或历史证据，不发布新版本。后续功能批次仍按真实复用阻碍安排，主题导入/预览宿主边界尚未实施。

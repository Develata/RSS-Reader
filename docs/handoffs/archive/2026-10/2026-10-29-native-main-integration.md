# 原生优化有效改动接入当前主线

> 历史分支记录，2026-10-30 从 `2312dd0e` 补收归档。下文只描述该来源的代码、测试和当时判断；PR #40 后续已经合并。Settings 的三项差异以当前[适配契约](../../../design/native-control-adapter-contracts.md)及[逐项复核](../../2026-10-30-branch-consolidation.md)为准，不能将旧分支的行为建议或验收结果视为当前主线结论。本次仅增加此说明并修复引用，原始 Git blob 留在恢复 bundle 中。

- 日期：2026-10-29
- 作者 / Agent：Codex
- 分支：`codex/native-main-integration`
- 初始基线 HEAD：`6e99706ce90026c70c9538eb35faf572f02781fd`（PR #38）；随后接入 `9b17a4d5053f5de2678533b8b514ce1db3c1e96c`（包含 PR #39、#41）
- 固定来源：`58157a1a90796f8a17624bf9958b0be75bb15bf4`
- 相关 commit：`7cd533bc`（原生有效改动与冲突决策）、`bb72bde0`（接入最新 main）；最后检查修正及回执见本记录所在的 `test: validate native main integration and clean merged branches` 提交
- 相关 tag / release：N/A；本轮不发布
- 状态：`validated`（本地整合、所列自动化验收及分支清理；同日后续已获授权并发送 PR 交接）

## 工作摘要

用户授权先把原生优化的有效改动接到最新 main，保留当前架构和进行中的工作，解决冲突、测试后再清理已完成旧分支。使用独立受管工作树合并来源的 17 个提交；保留双方祖先关系，按当前契约逐项解决冲突。

## 影响范围

- UI：Entries、Feeds、Reader、Settings 页面规则、Dialog 和主题文件适配。
- 平台：共享 Rust、正式 Web、Windows 原生；Android 编译与实机范围分别记录。
- 不改变七 crate / Backend 架构、单库、解析预算与后台图片调度。
- 原生工作树 `E:\gitclone\RSS-Reader` 的既有未提交内容，以及目录 UI 工作树 `E:\gitclone\RSS-Reader-directory-ui-20261029` 均不改写。

## 关键变更

| 来源提交范围 | 接入决策 |
| --- | --- |
| f76b15c4、f239620d | 提取行/批量写入规则与同步偏好准入；保留 main 的 app 写入任务、偏好队列、未知结果恢复 |
| ed90aa53 | Reader 访问身份纯规则；保留 main 的只读刷新和离页标记写入 |
| dda9aefa、8ea613cd | 保存准备/结果纯策略改为 SettingsPatch；拒绝旧的整份草稿比较与旧 runtime |
| 2b551537、ee5582c1、7f1014a0 | Feeds 请求/提示策略、编辑器状态、Dialog cancel；未知结果继续保留 6 秒提示 |
| f25dbdbb | WebDAV 参数捕获与确认纯规则；保留 main 的 app 任务和全局结果处理 |
| 49860dd4、b1b1d4a7 | 文件 I/O 错误、严格 UTF-8、导入请求身份和导出快照 |
| ea70ff19、ca0861cb、58157a1a | 原有字段补丁保留语义上增加预设代次和逐字段数字反馈；保留 main 的暂存加载与写失败恢复 |
| 1d27eae4、075569b3 | 当前适配契约重写；17 份原分支 handoff 归档、旧 BrowserStore 验收保留在源提交历史 |
| 906035ef | 原生解释器 BOM 保护；保留 main 退出时关闭资料库的事件处理 |

源码入口见[控件适配契约](../../../design/native-control-adapter-contracts.md)。没有合并其他任务的未提交文件，也没有恢复 rssr-host、sqlx 或 BrowserStore。

## 验证

本地验证已完成。结构化结果见 [result.json](../../../testing/evidence/2026-10-29-native-main-integration/result.json)；完整日志和截图目录：`target/native-main-integration-20261029/`。

- 第一轮构建找到旧测试接口不兼容并修正。
- 默认全工作区并发构建遇到 `rustc-LLVM ERROR: out of memory`，其后产生的依赖缺失/编译器内部错误不计通过；改为 2 个构建任务且关闭调试符号重跑。
- `cargo test --workspace --locked -j 2`：首次整合 1059 通过、0 失败、4 既有忽略项；接入 `9b17a4d5` 后重新全量执行，**1060 通过、0 失败、4 既有忽略项**（63 个测试套件，`workspace-final.log`）。关闭 dev/test 调试符号。
- 减少无意义草稿复制后，`cargo test --workspace --locked -j 2 settings_page::`：59 通过。
- `cargo fmt --all --check`：通过。`cargo clippy --workspace --all-targets --locked -j 2 -- -D warnings`：发现并移除测试中 `&str` 的多余 `.into()`，重跑通过（`clippy-final.log`）。
- 新浏览器运行器使用真实服务器与当前 schema，不复用旧分支的通过数或截图。
- 同源码正式 Web debug 构建通过；真实服务器与单库运行的 `rssr_native_preparation_acceptance.cjs` 在 1536 / 390 两种宽度共 **10 组通过**。页面异常 0；2 次预期的范围校验 HTTP 400，以及 debug 产物的 14 次 Dioxus 开发工具 WebSocket 404 分别记录，CI 默认仍要求无此开发工具错误。
- `rssr_workspace_acceptance.cjs` **18 组通过、页面异常 0**，覆盖文章选择、来源编辑、偏好顺序、离页后的文章/Reader 写入和主题工具栏。首次运行在偏好快速切回后重载失败；检查发现测试可能在第一笔写入提交前读到初始 All/Time，过早判定保存完成。现在先等待最终 PUT 响应成功，再读取和重载；完整重跑通过。未修改 main 的保存队列实现。
- `cargo build --locked -p rssr-app -j 2` 原生构建通过。使用复制的 EXE、独立 WebView profile、回环 RSS 和 72 篇文章 / 6 个订阅的独立库，`rssr_native_transport_acceptance.cjs` **13 项通过**：BOM/中文/emoji 字符串表、所有相邻字段、CSS、保存、页面重新挂载、1280 / 390 宽度和数据保留。页面/控制台异常均为 0。
- 原生 EXE SHA256：`08f63753e14bbd4884b9ce84198b1326565f8109691f9fb23e3ca77b5932ab29`；WebView `156.0.4314.50`。本次原生进程、回环服务和测试端口均已关闭。
- 本机 `dx 0.7.10` 与锁定 `dioxus 0.7.9` 提示版本不兼容，`dx build --platform desktop` 另遇重复 VERSION 资源的 `CVT1100 / LNK1123`；该命令不计通过。正式 Windows workflow 使用 Cargo 构建，本轮实际运行的是通过的 Cargo 产物，没有为绕过本机 CLI 问题改动产品构建契约。
- JavaScript 语法、文档链接、`git diff --check`：通过。浏览器运行器改用当前订阅修改接口的 PATCH，并允许通过 `RSSR_BROWSER_PORT` 隔离并行工作树。

未覆盖：Android 本轮编译/设备验收、系统文件选择器、真实 IME/触摸、真实 WebDAV、release 产物与本地整合分支的远端 CI。以上结果不能移用于另一对话独立推送的 PR #40 版本。

## 当前状态与后续

- 原生代码已形成本地合并提交，包含 main `9b17a4d5`，保留当前架构和最新已合并的目录/设置层级；该本地版本已完成上述验证。本对话没有推送产品分支或修改 PR。
- 原主工作树在固定来源后新增的 `418801f8` 是推送/CI 说明文档，其未提交文件仍保留。目录任务继续在自己的工作树开发，本轮没有复制这些未提交内容。
- 已合并旧分支清理完成，详见下节。保留本地整合分支及工作树，待 PR 负责方吸收仍有价值的差异后再归档，不能因清理数量而丢弃本轮成果。

## 已合并分支清理

逐个重新读取 GitHub PR 回执，验证远端当前 SHA 与 PR 最终 head 相同、合并提交属于 `9b17a4d5` 的历史，并排除关联工作树的名称。按用户本轮授权，用原子推送与每条引用的预期 SHA 约束删除下列 16 个远端分支：

| 已删除远端分支 | 已合并 PR |
| --- | --- |
| `chore/pages-actions-hardening` | #14 |
| `fix/feed-guid-identity` | #5 |
| `fix/import-validation-migration-lock` | #6 |
| `fix/large-feed-filter-bind-limit` | #11 |
| `fix/native-outbound-request-safety` | #26 |
| `fix/native-persistence-atomicity` | #3 |
| `fix/pages-smoke-profile-cleanup` | #18 |
| `fix/post-audit-repair` | #10 |
| `fix/post-generation-review` | #13 |
| `fix/refresh-generation-fence` | #12 |
| `fix/rust-1.99-async-trait-clippy` | #9 |
| `fix/sqlite-large-feed-batching` | #4 |
| `fix/storage-gc-query-contract` | #15 |
| `hotfix/mobile-entry-scroll-snapback` | #2 |
| `perf/batch-content-upserts` | #17 |
| `perf/sqlite-large-feed-scope-json` | #16 |

同时删除未被工作树使用且对应同一 PR head 的本地 `fix/native-outbound-request-safety`，并移除其跟踪配置。本地 `main` 从 `d9ebe354` 快进到 `9b17a4d5`，已确认没有独有提交且不在任何工作树中检出。

保留：所有关联工作树的分支（包括 `fix/feed-proxy-content-isolation`）、当前原生/设计/文档工作、四条未证明完整吸收的历史、两条历史保险备份，以及近期目录任务的引用。本轮没有删除工作树、用户文件、dirty 内容或重写提交历史。清理后快照为 18 个本地 / 15 个远端分支，其中包含本轮两个并行整合工作树和新设计任务，不都是待合并功能。

恢复包位于 `C:\Users\QQ\.codex\backups\RSS-Reader\20261029-124212-merged-branches\before-cleanup.bundle`，已执行 `git bundle verify`；SHA256：`2bdd359e7490897607d36569124fa9da43e917648cb52bb0a90fcbfe95c5af46`。同目录保留前后引用、PR 证明、删除回执和结构化结果。该目录在工作树之外，后续归档本工作树不会丢失备份。恢复时从 bundle 中精确 fetch 所需引用到新恢复分支，先核对再考虑恢复远端。

## 同时进行的 PR #40 整合

检查时发现用户在“规划原生 Rust 化前的模块优化”（`01a17c4b-67c0-73d2-96aa-00f7aaf09fc7`）也授权了 PR #40 的冲突解决、提交、推送与 CI。该任务使用独立工作树 `C:\Users\QQ\.codex\worktrees\pr40-main-integration\RSS-Reader`，已将 PR 更新为 `72e0516b`；本轮末读取到 draft、未合并、无合并冲突。本轮不修改其文件，也不竞争更新同一 PR；已向用户提出分工与发送交接的请求，仍未收到答复，故尚未向该对话发消息。

对照其 Settings 代码时，发现下面三项值得接手者复核；最后核对 `72e0516b` 仍保留相应旧路径，后续提交应再次检查：

- `preferences.rs` 的无效数字输入只调用 `note_draft_edit()`，需要同时将对应字段的最后有效值标为 pending，才不会被迟到读取/保存回填覆盖。本实现的 `retain_numeric_input` 与 `invalid_numeric_input_survives_a_load_without_hiding_other_stored_fields` 覆盖该路径。
- 预设下拉框只比较保存前后字符串会遗漏 A → B → A 的后续选择，加载也会重置未应用的选择。本实现使用预设修改代次，测试 `a_preset_choice_away_and_back_survives_load_and_save_results` 同时覆盖加载和保存。
- 文件导入在保存准入、无效保存尝试与等值存储回填时也应失效，不能只在 CSS 字符串实际变更时失效。本实现保留 main 的字段叠加规则，同时用独立的导入代次约束迟到文件结果。

本地新浏览器脚本与该任务同名文件不能直接互相覆盖；应逐项选择当前真实服务器验收场景。原生 transport 脚本的夹具已改为当前单库的 72 篇文章，不能继续沿用旧 5000 篇的验收文字。

### 2026-10-29 同日后续：分工确认与交接发送

用户回复“它负责 PR，允许发送交接（推荐）”，确认由“规划原生 Rust 化前的模块优化”统一负责 PR #40 的复核、接入、推送与 CI，本对话继续旧分支整理。此前未获授权的状态到此结束。已通过 `send_message_to_thread` 将代码 HEAD `9a6700fc99b1bce9fd58fbdb6ea7200120709478`、提交链、上述三项差异、[设置会话职责整理](./2026-10-29-settings-cohesion.md)、验证范围与备份路径发给该对话；工具返回目标 thread ID，确认发送成功。发送成功不等于差异已吸收或 PR 已合并。

发送前重新读取 GitHub 与目标已提交源码：PR head 已为 `50af7c99c9c47f045df8ccd7cf39c5dbba6524ed`，base 为 `8932a5ac317bfcb1af9b2d966e16ce3d989a9911`，仍为草稿、未合并且无合并冲突。对方已独立完成 `draft_edits` / `request_lifecycle` 封装，并接入 PR #42 的主题预览、明确保存和离页撤销行为；本来源仅基于 main `9b17a4d5`，不能整目录覆盖新版本。上述三项设置交错路径已对照 `50af7c99` 再读后交给负责方复核，不把本地通过数当作其当前验收。

同日分支整理复核：`git for-each-ref` 与 `git ls-remote --heads origin` 显示 18 个本地、15 个远端分支；前述 16 个已删除远端分支均未重现，本地 `fix/native-outbound-request-safety` 仍不存在。恢复 bundle 的 SHA256 再次与本记录一致。本次仅复核，未新增删除、重写引用或改动其他工作树；保留本地整合来源供 PR 负责方接入。

本次跟进仅更新交接文档，产品代码仍为 `9a6700fc`，不重复运行已通过的产品验收。文档相对链接与 `git diff --check` 通过；文档提交见本段所在的 `docs: record PR 40 ownership handoff` 提交。

## 备份与接手备注

`target/native-main-integration-20261029/before-integration.bundle` 已创建并验证，覆盖合并前 Git 引用及对象；`source-snapshot.json` 固定工作树、分支和原生未提交状态。bundle 不包含未提交/未跟踪内容，原目录中的这些内容未改动。不得据此删除活动工作树或原生任务的 dirty 文件。

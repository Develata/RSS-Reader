# Reader 正文选择归属与行为保持验证

- 日期：2026-10-31
- 作者 / Agent：Codex
- 分支：`codex/reader-body-boundary`
- 基线 HEAD：`801f6337e81f498be16a3dab9ea7db3733dbbb82`（本轮重新核对远端 main）
- 相关 commit：实现 `97d75438c8ba54045f28c559cb01b2bc0de4f8a1`；本文件所在核验文档提交（`docs(reader): record PR 54 verification`）
- 相关 PR：[PR #54](https://github.com/Develata/RSS-Reader/pull/54)
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

按正文渲染边界中的下一批范围，将正文候选选择从 Reader 页的 `support.rs` 归到唯一调用方 `ui/runtime/reader`，消除这一条反向依赖。选择函数体与命令执行逻辑保持原样，直接复用现有 infra 清洗；没有发现需混入本批修复的运行缺陷。

## 影响范围

- 模块：`rssr-app` 的 Reader 正文选择、runtime 导入和规则测试；现有独立规则检查脚本、CI 检查归属及对应设计/验收/状态文档。
- 平台：普通 Rust 规则、当前 Windows 编译环境和正式 Web；没有修改宿主脚本或平台执行逻辑。
- 工作树：继续使用 `E:/gitclone/RSS-Reader-worktrees/native-back-policy`。主 checkout `E:/gitclone/RSS-Reader` 的其他任务改动保留。
- 七个 crate、Backend/LocalBackend、单库与正文 HTML 输出保持不变；无新依赖、crate、trait、IR 或状态副本。

## 关键变更

### 模块归属

- `pages/reader_page/support.rs` 移至 `ui/runtime/reader/body.rs`；父模块私有声明 `mod body`，类型和函数仅 `pub(super)`。
- 页面删除旧声明，runtime 只从自身子模块导入正文选择。runtime 仍使用页面 intent 和加载结果类型装配回执，本批不声称其全部脱离页面/Dioxus。
- 8 项正文规则覆盖候选优先级、缺失/空字符串、清洗失败、图片地址、相对链接、MathML 和超预算降级。原有 4 项保留，新增 4 项先在旧位置验证，再移动同一源码。
- `reader_rules.rs` 直接加载生产 `body.rs`；独立脚本改为构建既有 infra 依赖并显式链接 API/domain/infra/serde/url。真实清洗器参与测试，本机编译图拒绝 Dioxus/web-sys/Wry/Tao。
- 原 CI 步骤继续执行同一脚本，从 API job 移到 infra job 复用依赖缓存，不增加作业。infra 整体包含数据库与网络能力，不能将“无 UI 框架”写成整个依赖图无 I/O。

### 行为核对与 review

- 从基线提取 `select_reader_body`，与当前文件的签名、函数体逐字比较（只排除可见性、模块注释和行尾格式），结果相同。`execute` 起的 runtime 代码也相同。
- 首次新增基线测试错误地预期普通链接也被补全，7/8 通过；实际旧实现保留 `href="../next"`，仅图片 `src` 补全。修正测试预期后旧位置 8/8 通过，没有修改产品 URL 策略。
- 已审查单一调用方、可见性、测试直接引用、CI 脚本真实依赖与原清洗路径；本轮未发现新的产品缺陷。

## 验证与验收

证据目录：`target/reader-body-boundary-20261031/`。该目录保存本轮日志、等价核对脚本和回执，历史证据未覆盖。可追踪的[精简回执](../testing/evidence/2026-10-31-reader-body-boundary/receipt.json)保存基线、源码散列、日志散列和产物身份；本轮产物来自提交前工作树，提交包含同一份已验证源码。

| 命令 / 检查 | 本轮结果 | 证据 |
| --- | --- | --- |
| `cargo test -p rssr-app --locked pages::reader_page::support::tests`（移动前） | 8/8；首次预期修订另存 | `body-baseline.log`、`body-baseline-attempt-01.log` |
| `cargo test -p rssr-app --locked` | 440 passed，1 ignored（已有手动性能测量）；含真实命令回执 transcript | `app-tests.log` |
| `python scripts/check_reader_rule_boundary.py --target-dir target/reader-body-boundary-20261031/rules-build` | 35/35；176 个编译目标无上述 UI 框架 | `rules.log`、`rules-build/reader-rule-boundary.json` |
| `cargo clippy -p rssr-app --all-targets --locked -- -D warnings` | 通过 | `clippy.log` |
| `cargo fmt --all --check` | 通过 | `fmt.log` |
| `cargo check -p rssr-app --target wasm32-unknown-unknown --locked` | 通过 | `wasm-check.log` |
| 同一函数 / runtime 执行逻辑核对 | 通过 | `source-equivalence.json` |
| dx 0.7.9 `build --platform web --package rssr-app --locked --release --debug-symbols false`，构建本次 `rssr-web` / `browser_fixture_library` | 通过 | `web-build.log`、`server-build.log`、`fixture-build.log` |
| `node scripts/browser/rssr_reader_host_acceptance.cjs` | 6/6；1280/390 两视口，页面/控制台错误均为 0 | `web-candidate-01/acceptance.json`、截图及 `web-acceptance.log` |
| `actionlint .github/workflows/ci.yml` | 通过 | `actionlint.log` |
| 文档链接、主 checkout 保全及最终 diff review | 通过；无新增断链，HEAD/status/index 与 41 个文件/缺失路径未变 | `documentation-check.json`、`primary-preservation.json` |

Web 使用实际同源服务器和新建隔离 SQLite，构建标识为 `dev-801f6337e81f-dirty-8f2160d303f9`，浏览器 Chrome `155.0.8059.40`。`RSSR_WEB_BIN` / `RSSR_FIXTURE_BIN` 指向本工作树 `target/debug/rssr-web.exe` / `target/debug/examples/browser_fixture_library.exe`，`PUBLIC_DIR` 为 `target/dx/rssr-app/release/web/public`，`ARTIFACT_DIR` 为本轮 `web-candidate-01`。固定 dx 位于 `target/reader-bugcheck-20261030/dx-0.7.9/bin/dx.exe`；包的 29 份静态文件及两个 EXE 均已记录 SHA-256。回归复用现有真实图片/正文、快捷键、标记写入场景，没有新增浏览器替身。

## 结果与未覆盖范围

实现阶段完成 review、相称测试与本地提交 `97d75438`；随后按用户“按建议继续”推进 PR 核验，已推送同名分支并创建 PR #54，尚未合并或发布。远端 main 仍为 `801f6337`。上表是实现阶段的本地验证，远端结果另见下节；既有主线 CI 和此前 Windows/Android 真机结果不冒充本轮验收。

没有改动 JS、HTML 插入、焦点、位置捕获、监听或 Android 返回。本轮不安装测试 APK，不重复 Windows WebView2 / Android 实际宿主验收；Windows Rust 测试和 Web 编译不替代原生交互。物理键盘、系统 IME、辅助技术及候选原生正文渲染器仍未覆盖。

## PR #54 核验（2026-10-31）

- 再次核对主线 `801f6337` 与实现提交 `97d75438`：只领先一个提交，无主线差异待整合。已验证源码、日志、Web 包及 EXE 散列仍与实现回执一致。
- 推送 `codex/reader-body-boundary`，创建 PR #54。GitHub 确认为可合并，当前尚未合并；没有向旧 PR 追加内容或发布版本。
- PR 的 [CI 运行](https://github.com/Develata/RSS-Reader/actions/runs/41187780112)与 [Pages PR 构建](https://github.com/Develata/RSS-Reader/actions/runs/41187780013)绑定实现 SHA；Pages 的部署步骤按 PR 条件跳过，不作为网站发布。
- 完整 CI 结果：实现 SHA `97d75438` 在第二次 attempt 达到 **36/36 成功**；只重跑首轮失败的工作区作业及汇总门禁。Linux 独立规则 **35/35**、下载产物中的 Reader 正式 Web **6/6**，Chrome `156.0.8078.80`，页面和控制台错误均为 0。PR 测试合并提交 `c9213de5cfd32e1b64d972676e974b5d01d50b0e` 的 tree 与实现提交相同，Web/server 构建标识 `dev-c9213de5cfd3`；不把它误当成 main 已合并。
- [PR 精简回执](../testing/evidence/2026-10-31-reader-body-boundary/pr-ci.json)记录实际 SHA、36 个作业、首轮失败、诊断缺口与重跑产物。原始分页作业列表为 `target/reader-body-pr-20261031/implementation-ci.json`；本次核验提交只改文档，推送后的最终 head 另行核对，结果保存在同目录 `final-ci.json`，不冒用实现 SHA 的检查结果。
- 本轮保全核对使用新建 `primary-before.json`，继续只在 E 盘接续工作树修改；主 checkout 的 41 个文件/缺失路径、HEAD、status 与 index 不纳入操作。

首轮 CI 是 34 成功 / 2 失败：`reading-workspace` 在原有 `preferenceOrdering` 第一组等待最终 PUT 的 `requestfinished` 超时，汇总门禁随之失败；尚未执行 Reader 宿主场景。两笔 workspace PUT 依次收到 HTTP 200，随后分别在 195 ms、200 ms 记录 `net::ERR_ABORTED`。与 PR #50 / #52 曾出现的签名一致，本轮未改偏好实现或测试。失败 artifact `13038065828` 已下载并校验 SHA-256 `52595c577a4ecce76c9b72724e98ac4ba42ab350d1e63741ca5824a9805f8ea8`，日志、HTML、截图与完整首轮 CI 元数据保留在本轮 target。

隔离诊断按原顺序运行前置 9 场景与偏好排序：Windows Chrome `149.0.7827.55` 通过；Chrome `155.0.8059.40` 加 150 ms CDP 受控延迟与响应读取观察也通过，workspace 请求失败与页面异常均为 0。后一观察在页面重载后被清空，不能用空记录证明 CI 中应用已完整收取响应。`reqwest 0.12.28` 的 AbortGuard 在释放时调用 abort 仅是排查线索，尚未证明它导致本次 CI 失败。没有扩大超时、放宽断言或修改产品代码。

首次本地启动因缺少 Playwright 指定的 bundled browser revision 失败，随后使用已有浏览器；补下载与 CI 相同的 Chrome `156.0.8078.80` 被自动审批策略拦截（需要批准，但当前模式不能弹出批准），未变更审批规则。版本与 Windows/Linux 差异仍未消除。该问题已列入[维护待办](../maintenance-backlog.md#偏好保存顺序的-ci-不稳定点)；本轮对同一实现 SHA 仅重跑失败作业，重跑成功不代表根因已修复。

## 给下一位 Agent 的备注

从[正文渲染边界](../design/reader-rendering-boundary.md)与本交接继续，核对 PR #54 最新 head 的检查后再整合。偏好顺序验收的不稳定点已有多次相同签名，应优先单列定位，避免继续靠重跑收口；后续模块提取仍由实际复用阻碍驱动。无第二种实现前不扩展通用 Host 接口或正文模型。正文选择返回 HTML/Text 的现有语义继续保留，不能因文件迁移推断整个 Reader 已可直接换原生控件。

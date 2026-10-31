# Reader 正文选择归属与行为保持验证

- 日期：2026-10-31
- 作者 / Agent：Codex
- 分支：`codex/reader-body-boundary`
- 基线 HEAD：`801f6337e81f498be16a3dab9ea7db3733dbbb82`（本轮重新核对远端 main）
- 相关 commit：本文件所在本地提交（`refactor(reader): colocate body selection with runtime`）
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

本批已完成实现、review、相称测试和本地提交，工作树收口。未 push、未合并、未发布；远端 main 仍为 `801f6337`。本轮只做本地验证，远端 CI 尚未运行；既有主线 CI 和此前 Windows/Android 真机结果不冒充本轮验收。

没有改动 JS、HTML 插入、焦点、位置捕获、监听或 Android 返回。本轮不安装测试 APK，不重复 Windows WebView2 / Android 实际宿主验收；Windows Rust 测试和 Web 编译不替代原生交互。物理键盘、系统 IME、辅助技术及候选原生正文渲染器仍未覆盖。

## 给下一位 Agent 的备注

从[正文渲染边界](../design/reader-rendering-boundary.md)与本交接继续，先 review 并整合此有界样本，再按实际复用阻碍选择后续工作。无第二种实现前不扩展通用 Host 接口或正文模型。正文选择返回 HTML/Text 的现有语义继续保留，不能因文件迁移推断整个 Reader 已可直接换原生控件。

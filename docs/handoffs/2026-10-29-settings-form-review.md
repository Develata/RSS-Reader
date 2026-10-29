# Settings 数字提示回填审查

- 日期：2026-10-29
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 开始 HEAD / 被审查提交：`ca0861cb1f531df4a91c990fbcdb781bd308d68f`
- 相关 commit：本次仅提交审查交接；生成时 `commit: pending`。提交后由 `git log -1 -- docs/handoffs/2026-10-29-settings-form-review.md` 定位，最终 SHA 记入本机 `final-receipt.json`。
- 相关 tag / release：无
- 状态：`validated`（限定于下列审查与验证范围）

## 工作摘要

审查上一批数字解析提示的草稿回填代次、四字段接线、保存/下载结果准入及正式验收脚本。未发现需要修复的代码问题；补查保存尚未完成时继续输入无效数字的实际 Web 行为，没有修改生产代码或正式验收脚本。

## 影响范围

- 审查模块：`settings_page/{request_lifecycle,session,facade,preferences}.rs`，并追踪 `save/session.rs`、`sync/session.rs`、`save_policy.rs` 与主题导入失效入口。
- 审查工具：`scripts/browser/rssr_settings_sync_acceptance.cjs`、`settings_form_scenarios.cjs`。
- 实际执行：Windows 上 Rust 测试、静态 Web/WASM、运行器启动失败检查。全程 CLI / Playwright；没有操作用户数据或系统 GUI。
- 提交内容：本交接记录；不涉及配置格式、架构、依赖、存储或发布流程变更。

## 关键检查与结论

- 没有发现需按严重级别列出的缺陷。回填代次只在已获准的整份草稿替换中推进，不改变用户编辑代次、写入 gate、旧读取作废或保存的值相等规则。
- 同值回填能清除旧提示，数值往返不能使提示复活；后续新错误仍只属于对应字段。比较代次发生在渲染期，未引入渲染期写入或页面 effect 循环。
- 回填同值不会额外取消主题导入；实际值替换仍走既有导入失效条件。保存失败没有设置结果，不推进回填代次。
- 正式验收运行器仍在编辑前核对原生 fixture；启动失败会覆盖旧成功回执、返回非零并释放自身连接。连接的调用方浏览器保持可用。

## 验证与验收

证据目录：`target/settings-form-review-20261029/`，属于本机 ignored 证据；缺失时应注明不可用。

### 本轮新执行

| 命令 / 检查 | 实际结果 | 证据 |
| --- | --- | --- |
| `cargo test -p rssr-app --locked settings_page` | 61 通过、0 失败、0 忽略；无编译警告 | `settings-tests.log` |
| `node target/settings-form-review-20261029/save-feedback-probe.cjs` | 1536×844 / 390×844，四数字 × 三交错各 12 项，共 24 项通过 | `save-feedback-probe.json` |
| `node scripts/browser/rssr_feeds_lifecycle_startup_test.cjs target/settings-form-review-20261029/startup settings-sync` | 缺页面、缺数据库、CDP 返回 503 共 3 项通过；旧成功回执被替换、标记数据库哈希不变、调用方浏览器仍可用 | `startup/results.json` |
| `node --check scripts/browser/settings_form_scenarios.cjs` 与 `node --check scripts/browser/rssr_settings_sync_acceptance.cjs` | 通过 | CLI 返回码 |
| 源码/运行器及实际 Web 入口身份 | 8 个文件与上一批验证哈希一致；候选 index/JS/WASM 哈希一致 | `review-identity.json` |
| `git diff --check`、交接链接和提交范围核对 | 通过 | 提交前后检查 |

探索脚本使用既有离线 fixture、Storage helper 和真实 Web Lock，先确认 Rust 保存请求已排队且按钮禁用，再逐字段清空输入。三种交错为：

1. 后续无效数字保留的整份草稿仍等于提交值：保存完成后回填获准，提示清除。
2. 同时更改主题使后续草稿不同：保存完成后整份后续草稿及数字提示保留；再次明确保存后清除提示。
3. 注入 Storage 写失败：旧持久化值、后续草稿和数字提示保留；恢复存储后明确重试，成功清除提示。

两视口均完整核对 72 篇文章标记不变，页面/控制台错误为零。脚本在 `finally` 中释放锁、恢复 Storage 方法并关闭自身浏览器。它是本轮保留于证据目录的探索脚本，**不是新增的仓库长期回归入口**。

运行环境：`NODE_PATH=C:/Users/QQ/node_modules`，`PUBLIC_DIR=E:/gitclone/RSS-Reader/target/settings-form-20261029/web-candidate-public`；Chrome `155.0.8059.40`。复用上一批候选 WASM，SHA-256 为 `f831cdb00761bcd68a65bc44ef9a63c78189bee642070ac8d0a66f5decae9b10`，本轮已重新核对实际引用资产与源码一致。

### 既有证据和未执行项

- 上一批的 574 项工作区测试、Clippy、WASM/Android 编译、Web/Windows 各 90 项表单场景与原生重启结果见[原交接](./2026-10-29-settings-form-feedback.md)。它们不是本轮重新执行的结果；生产代码未改，因此没有再次完整构建和广泛重跑。
- 本轮未新跑 Windows EXE、前台原生视觉、系统文件窗口、真实 IME、Android 设备、Linux/macOS 或托管 `rssr-web`。没有把 Web Lock / Storage 注入当作 SQLite 故障或原生宿主验证。
- 手工验收未执行。没有新的视觉布局改动，沿用上一批已记录的四对 Web 图像比较，未把它计为本轮新截图。

## 结果

审查通过，无需代码修复。仅提交交接文档，原修改的适用范围和发布边界不扩大；没有 push、tag 或远端操作。

## 风险与后续事项

- 保存值相等与下载编辑代次是不同契约。将来调整字段缓冲、原生控件或异步接线时，需要继续验证两者对无效输入提示的不同处理。
- 真实 IME、新 OS 控件及前台原生视觉的既有缺口仍然存在。

## 给下一位 Agent 的备注

- 当前实现入口与正式复验命令见原交接；本轮探索脚本和输入/结果身份见上述本机证据目录。
- 初始 staged 为空。任务外技能/spec 删除、两个 10/03 handoff 修改和已有 untracked 内容全部保留，只暂存本交接文件。

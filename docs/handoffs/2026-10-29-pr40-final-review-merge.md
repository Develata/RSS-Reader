# PR #40 最终审查与合并回执

- 日期：2026-10-29
- 作者 / Agent：Codex
- 分支：本地 `codex/pr40-main-integration`；PR `codex/native-ui-preparation-20261029` → `main`
- 审查 HEAD：`6117a353e398a96c7bf1488df221d7e5ff01f924`
- 最新主线：`85b679a5afb12df9de53d40903cfb99745874b1e`（PR #44）
- 相关 commit：pending；主线文档整合及本记录待提交，最终 PR 合并回执待补充
- 相关 tag / release：无
- 状态：`draft`（代码审查及补核通过；等待最新 head CI 和最终合并）

## 工作摘要

按用户授权再次审查 [PR #40](https://github.com/Develata/RSS-Reader/pull/40)，没有发现需要修复的阻塞问题。合并前 main 新增 PR #44，仅有文档及协作规则变化；本次普通合并保留双方交接入口和新增的自动化验收要求，不改产品代码。最终合并结果将在本记录收口。

## 影响范围

- 审查模块：Entries、Reader、Feeds、Settings 的私有规则模块及 session 接线；dialog 关闭门禁；原生字符串传输和主题文件适配；新增验收脚本及 CI 接入。
- 本次改动：接入主线文档、解决交接索引冲突，新增最终审查记录与数值投影补核回执。
- 平台：共享 Rust / Web / Windows 原生代码审查；本轮额外运行的是正式 Web 自动化。没有把浏览器视口当作 Android 设备。
- 工作树：仅操作 `C:/Users/QQ/.codex/worktrees/pr40-main-integration/RSS-Reader`，其他工作树及用户资料库保持原状。

## 关键审查结论

- facade/session → UiCommand → runtime → Backend 层次保留；没有迁回 SQLx、双库、BrowserStore 或新增业务状态副本。私有规则集中在各页面内部，I/O 与任务作用域仍由原适配层负责。
- Settings 保留主线逐字段补丁和 `stored → submitted → pending` 顺序；失败恢复位于后续编辑之下。初次读取、下载与保存的迟到回执经过请求身份检查，主题导入身份在选择器打开时建立，并在后续编辑、预览、保存等操作时失效。
- Entries 保留 App 所有的写入、完整 Reread、偏好保存队列；Reader 保留访问身份和独立只读刷新代次；Feeds 保留编辑门禁、配置指纹、写入排序及未知结果提示时长。
- 原生 BOM 修正只作用于锁定版本解释器的解码器；文件 UTF-8 错误保持可报告、可重试。来源保存期间的 dialog 关闭仍经过 Rust gate。
- 验收等待边界修正有有限超时及阶段日志，没有把先前 CI 超时的未复现触发条件描述为已经定位。
- GitHub reviews 与 review threads 均为空；这表示没有远端待处理意见，不代替本轮源码审查。

## 验证与验收

### 本轮执行

- 相对 main 的产品差异审查，以及 PR #44 的五个文档文件差异审查：通过。
- `git diff --check origin/main...HEAD`：通过（接入 PR #44 前）。
- `git diff --exit-code d9c7c698cf02d69eca7cce55ec4dcdf84f5718ab HEAD -- crates assets scripts .github build-support Cargo.toml Cargo.lock Dioxus.toml`：无差异，确认已有本地构建验收对应当前产品内容。
- `node target/pr40-final-numeric-review.cjs`：1536 / 390 两视口通过；四个数值框清空后，等值保存清除解析提示并恢复真实存储值，全部存储设置不变，页面异常为 0。[紧凑回执](../testing/evidence/2026-10-29-pr40-final-review/numeric-projection.json)。复用当前正式 Web 构建和真实服务器夹具，隔离端口 9325，结束时关闭服务器和浏览器。
- 重新读取 `6117a353` 的检查：37 项成功，Pages deploy 1 项预期跳过，无失败或未完成项。[CI 40916567382](https://github.com/Develata/RSS-Reader/actions/runs/40916567382)、[Pages 40916567417](https://github.com/Develata/RSS-Reader/actions/runs/40916567417)。
- 重新核对已下载的 `ci-reading-workspace` artifact `12917096276`：ZIP SHA-256 `d6bd03b6f597393cf6301e448b8cf500b842f9ec77e74baa878378534475aa0f`；workspace 18、Entries 8、Settings 8、目录顺序 26、目录窗口 26、窗口边界 11、目录复核 8，transport 通过。

### 已有同产品内容证据

- [上一轮整合记录](./2026-10-29-pr40-directory-order-integration.md)：工作区 Rust 1070 通过、4 项既有忽略；fmt、Clippy、Windows/Web 构建通过。
- 同记录的 Windows 实际 EXE / WebView2 13/13 通过，5000 篇全部标志和 6 个订阅地址不变，关闭后单独主库完整性及内容核对通过。它属于之前同产品源码的原生验收，本轮没有重新运行 Windows GUI。

### 手工验收

- 本轮没有 Computer Use 或手工 GUI 验收。OS 文件选择器、真实 IME、Android 真机、macOS/Linux GUI、正式安装包仍未覆盖，既有证据范围不扩大。

## 结果与合并回执

- 产品代码无需修复；主线文档冲突保留双方内容。
- 最新文档整合 commit、对应 CI、PR 合并 SHA 及后续交接 commit：pending，完成后按实际回执补充。
- 不打 tag、不发布新版本。

## 风险与后续事项

- 先前 CI 超时的具体触发条件仍未复现；有界等待和后续成功不能证明原触发条件已经消除，继续保留原始回执供再次出现时定位。
- Dioxus / sledgehammer 升级时必须复核原生解释器初始化顺序及 BOM 字符串表传输。
- 后续按明确平台场景补 OS 选择器、IME 和设备验收，不以本次合并或 CI 代替全平台发布就绪。

## 给下一位 Agent 的备注

- 先读本记录、[原生控件契约](../design/native-control-adapter-contracts.md)和[验收入口](../testing/native-control-acceptance.md)。
- 本次仅接入 PR #44 文档，PR #43 的共享目录分组模型及主线逐字段保存规则继续保持。
- 新的 head 或 main CI 必须与 `6117a353` 的既有 CI 分开记录；不能沿用旧 SHA 的成功标记。

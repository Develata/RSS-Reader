# 原生控件适配契约与平台验收清单

- 日期：2026-10-29
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 开始时 HEAD：`ea70ff19f7713220e8c1dec93aa53fa6fd679bc9`
- 相关 commit：与本记录同批本地提交；生成时 `commit: pending`。提交后以 `git log -1 -- docs/handoffs/2026-10-29-native-control-contracts.md` 定位，最终 SHA 另记本地 `final-receipt.json`。
- 相关 tag / release：无；不 push、不打 tag、不发布
- 状态：`validated`（文档与已有证据核对；不代表新平台验收）

## 工作摘要

按上一批 Settings 回填交接的建议，整理控件输入、回填、取消和宿主能力的现行契约，建立按证据等级区分的平台验收表，并把下一批限定为 Windows 系统 CSS 文件选择/保存的实际宿主验收。此次只改文档，不继续拆控制器、不建立状态镜像，也未更换渲染器。

## 影响范围

- 新增：[原生控件适配契约](../design/native-control-adapter-contracts.md)、[原生控件验收清单](../testing/native-control-acceptance.md)。
- 更新：[原生界面前置边界](../design/native-ui-preparation.md)、[设计索引](../design/README.md)、[测试索引](../testing/README.md)。
- 平台：文档区分静态 Web、Windows WebView2/CDP、系统 GUI、Android 编译/设备以及 Linux/macOS、托管 `rssr-web`；本次没有新增平台执行结果。
- 无生产源码、依赖、构建配置、CSS、配置格式或数据迁移变更。

## 关键变更

### 契约与权威状态

- 按 Settings 表单、保存/下载/上传、文件、Feeds 编辑、Entries、快捷键、手势、目录和位置恢复列出输入事实、捕获时点和源码入口。
- 显式保留 Settings 保存的值相等回填与初次加载/下载的编辑代次回填；程序回填不能伪装用户输入。草稿、预设及已保存主题沿用现有职责。
- 将页面卸载、请求失效、取消确认、弹窗关闭、系统文件取消、滚动恢复和全局刷新分开说明；不承诺离页关闭 OS 对话框或撤销已经发生的写入。
- 标明 Dioxus session 仍绑定框架、HTML Dialog 与 OS 控件不同、Android 文件导入未接入；自定义 CSS/HTML/主题选择器仍是当前产品契约。

### 可执行的下一批范围

- 验收表提供 10 组通用适配场景和 7 组 Windows W-FILE 场景：有效导入、取消/重选、真实错误、导出/覆盖、失败重试、离页与重开、焦点/IME。
- Windows 使用已 checkpoint 的复制 fixture、独立进程/profile/测试文件，先只读核对身份；页面 CDP 操作与 OS 选择器实际操作分别记录。
- 现有主题文件脚本仅覆盖 Web 文件路径，没有 Windows OS 选择器驱动。下一批应先实测现有实现，发现可复现问题后才修对应适配层。
- 新控件实现的最小表单候选是 Settings 三个枚举和四个数字字段；它只是后续范围建议，尚未选择宿主/框架或批准 CSS 契约变更。

## 验证与验收

证据根目录：`target/native-control-contracts-20261029/`（本地 ignored）。

### 自动化核对

- `node target/native-control-contracts-20261029/audit.cjs`：通过。12 项检查、110 处本地链接、23 项哈希比对，记录 19 个引用源码文件的身份，核对 17 个唯一场景 ID；上一批六份最终回执均为通过且无非预期 page/console 错误。结果写入 `audit.json`，属于证据审计而非新运行应用。
- `git diff --check`：通过；最终提交前按本批六个文档路径再核对。
- 任务外状态对照：开始状态保存在 `initial-status.txt` / `initial-staged.txt`；提交前及提交后分别检查，仅本批六个文档入提交，最终结果写入 `final-receipt.json`。

### 本次未执行及已有证据的使用边界

- 未重跑 Cargo、Clippy、Web/Android 构建、浏览器或设备：本轮只修改文档，未改生产代码或运行器。
- 文档引用的 571 个工作区测试、36 个 Web 回填场景及 Windows 正常同步是 `ea70ff19` 上一批已有结果，本轮只读复核日志、回执和产物身份，不写成新执行结果。
- 10/26–28 Entries、Reader、Feeds、目录交接仅作为历史场景索引；本轮未重新核验这些旧产物，不将其升级为当前版本的全平台验收。
- Windows OS 选择器 GUI、真实 IME、Android 系统导出/返回、Linux/macOS 与托管 `rssr-web` 未执行。W-FILE 表是待执行清单，无完成勾选。

## 结果

- 交付范围是文档契约、证据索引与下一批验收步骤；运行行为保持不变。
- 不声明原生渲染器已实现或系统文件 GUI 已通过；本次不构成发布验收。

## 风险与后续事项

- 首先执行 W-FILE 的实际系统窗口路径，再决定是否需要改适配代码；不要继续为统一接口添加控制器或业务状态副本。
- 旧证据保存在 ignored 目录，其他机器缺失时须明确标未找到或重跑，不能依据文档补造回执。
- 上一批 Web 窄屏 loaded 图 R 标识有 108 像素差异，原因未确定；本次保留这一限制，未把它记成全像素一致。

## 给下一位 Agent 的备注

- 从两个新增文档进入；代码入口仍是 `theme_file_io.rs`、`theme_io.rs` 和 `SettingsPageSession`。不把页面写入 gate 当作跨页面/进程事务锁。
- 本批只暂存六个文档路径。任务外 `.agents/skills` / `.specify` 删除、两份 10/03 handoff 修改和既有未跟踪目录/文档全部保留。
- 本次未访问用户数据库、未启动应用或系统对话框、未联系外部服务；本地提交授权沿用仓库约定。

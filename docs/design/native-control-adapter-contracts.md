# 原生控件适配契约

核对日期：2026-10-30。页面规则对应 PR #40 合入 main 后的实现，见[最终合并回执](../handoffs/2026-10-29-pr40-final-review-merge.md)；App 设置读取约束已通过 PR #47 合入 main，见[实现交接](../handoffs/2026-10-30-shell-settings-lifecycle.md)。旧分支的整份 Settings 草稿相等规则已退役。本文不新增公开 API 或原生渲染器；适配须遵守[功能边界](./functional-design-philosophy.md)、[前端命令](./frontend-command-reference.md)与[验收入口](../testing/native-control-acceptance.md)。

## 架构与状态所有权

- facade/session → UiCommand → runtime → Arc<dyn Backend>；application/domain 保持统一用例语义。平台差异留在 infra 和控件适配器。
- Windows/Android 仍用 WebView；HTML dialog 不等同于 OS 窗口。HTML select 的弹出实现须逐宿主核对：本轮 Android 已观察到系统原生列表弹窗，Windows 为 Chromium 控件，见[主题选择评估](./native-theme-selector-assessment.md)。系统文件选择器是单独的宿主能力。
- 原生 LocalBackend 使用单个 library.sqlite3；正式 Web 使用同源 HTTP Backend 与服务器 SQLite。不得恢复 rssr-host、SQLx、双库或 BrowserStore。
- 私有纯 Rust 规则仍由 Dioxus session 装配，不是稳定的跨渲染器 SDK，也不建立第二份业务状态。Settings 的 draft_edits 独占补丁转换，request_lifecycle 独占读写身份与暂存回执；session 不直接修改其内部字段，主题/Shell 投影仍留在 session。

## 输入和回填

| 输入/结果 | 当前规则 |
| --- | --- |
| Settings 枚举、数值、CSS | 按主线 SettingsPatch::between 记录实际改变的字段，再合并到 pending。A→B→A 后该字段仍可保留在 pending；无变化的通知不构造补丁。无法解析的数字保留上次有效值并显示字段提示，不构造补丁、不自动保存。 |
| Settings 初次加载/重新读取 | 只接纳最新读取；成功写入使更早读取失效。在途写入期间的读取回执暂存：成功写入丢弃；写入失败且没有更新读取则应用。若全局已应用更新的存储结果，使用当前全局值。 |
| App 级设置读取 | `ui/shell/settings_read_lifecycle.rs` 决定启动/只读命令、在途合并和错误归属。页面打开时自动读取不排队，显式重试仍可进入；完成时页面已打开则丢弃合并的后续读取。ThemeController 的存储 revision（含等值存储）使旧成功失效，重读错误绑定完成时 revision；App 任务、命令和主题发布留给 Shell。 |
| Settings 草稿投影 | 唯一规则为 stored → submitted → pending。加载和下载更新 stored，任何尚未保存的逐字段编辑都继续覆盖它；未编辑字段立即反映新存储。保留主线跨刷新/登录的草稿恢复。 |
| Settings 保存 | 仅发送 pending 字段；仅校验编辑过的 CSS，仅把补丁内页大小 0 规范化为默认值。成功清除 submitted，再覆盖 pending；失败把 submitted 放回 pending 下方。后续编辑即使等于已存储值，仍须明确保存。 |
| 数值解析提示 | 每个字段有独立投影代次。存储结果真正供应该字段时，等值回填也永久作废旧提示；有 submitted/pending 覆盖的字段保留提示。A→B→A 不复活旧提示。投影不触发输入事件。 |
| 预设选择 | 卡片选择、恢复默认和文件导入仅更新草稿及本页预览，明确保存才入库；输入/恢复 CSS 草稿不自动预览。加载/下载在 CSS 未编辑时跟随 stored CSS。保存回执仅在当前选择仍等于提交选择时重新识别预设，保持主线规则。 |
| WebDAV | 保存和实际下载共用同步写入 gate；下载确认、目标字段变化和上传清除确认沿用主线。保存中被拒绝的下载保留已有确认，不改变为重新确认。上传/下载属于 App，离页仍收回执、重新读取；存储主题仍更新，提示投向原页或 Shell。 |
| 主题导入 | 打开选择器即取得身份；新选择器、草稿/预设输入、预览尝试（包括校验失败）、明确保存、下载准入作废旧身份。存储投影实际改变 CSS 时同样作废旧身份。验证、提示、草稿与预览更新之前消费身份；旧成功/失败不得复活，导入本身不保存。 |
| 主题文件 | UTF-8 严格解码。Web 沿用去开头 BOM 的行为；原生保留文件原文，Dioxus 传输解码器保留 BOM。导出捕获点击时 CSS，不保存设置；真实文件错误可重试。 |
| 来源编辑 | ID + 打开代次归属，纯 FeedEditState 管理草稿、忙碌和错误。保存中拒绝编辑、关闭及重开；错误类别先由主线 UiFailure 投影为文案。App 写入、配置指纹和 URL/偏好写入顺序不变。 |
| Entries | 查询身份先于 flags revision；写入规则只输出有序发布和 Reread。补读必须走主线完整恢复流程，偏好队列、读失败期间的选择和离页回执保持。 |
| Reader/Feeds | Reader 的身份为文章 ID + 访问代次，保留单独的只读刷新代次和 App 标记写入。Feeds 只有快照使用查询代次；结果未知的提示时长与错误同为 6 秒，其他为 3 秒。 |

上传与 CSS 导出没有导入那样的“最新请求”规则，不在本次接入时擅自添加。页面任务取消、App 作业继续等待与存储已提交是不同层次；取消等待不能当成事务回滚。

## 控件与宿主边界

- 程序回填不能回送成用户输入；同步表单准入不能只依赖 disabled 外观。
- 来源保存时，即使焦点因控件禁用离开 dialog，Esc/cancel/背景点击也必须由 Rust 关闭 gate 决定。组词期间先同步阻止默认关闭。
- 快捷键、下拉刷新、目录/阅读位置继续使用既有 Rust 控制器；DOM 仅提供输入、几何、真实滚动、帧提交和焦点事实。
- 原生 BOM 兼容点依赖锁定的 Dioxus 0.7.9/sledgehammer 0.6.5 解释器 c 字段；升级依赖必须复核初始化时序与原生字符串表验收。
- 不改变自定义 CSS、正文 HTML、语义选择器、选择复制、IME 或系统返回能力。新的原生控件必须另留实际宿主证据；浏览器小视口不替代 Android。

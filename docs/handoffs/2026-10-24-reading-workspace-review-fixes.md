# 阅读工作区实现审查与修复

- 日期：2026-10-24
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 审查基线 / 开始时 HEAD：`f6d7d154b0591037cc4393468c5a92c6466b0644`
- 相关 commit：与本记录同一笔本地提交；用 `git log -1 -- docs/handoffs/2026-10-24-reading-workspace-review-fixes.md` 定位
- 相关 tag / release：N/A；没有 push、tag 或发布
- 状态：`validated`（本地自动化；实机覆盖限制见下文）

## 工作摘要

用户在模板落地后要求 review and fix。本轮复核来源编辑、多选与筛选、目录交互、持久化修改及响应式布局，修复以下五处已定位的问题。此前已确定的配色、页面宽度、正文阅读页和批量操作范围继续保持。

## 影响范围

- 模块：`rssr-app` 的 entries / feeds 页面、来源筛选组件、内部 UI commands/runtime，`reading-workspace.css`。
- 平台：共享 Web / desktop / Android UI；Android 返回路径复用既有 hook。
- 额外影响：浏览器回归脚本与前端交互契约。没有新增 crate、依赖、迁移、公开 host API 或存储格式变更。

## 审查发现与关键变更

### P2：旧编辑请求可以覆盖新草稿

打开来源 A、加载中关闭、再次打开 A，第二次加载完成并编辑后，第一次访问的迟到结果只按 feed ID 校验，能够覆盖草稿并解除保存中的 busy 状态。修复前新增 reducer 用例实际失败。

- 每次编辑访问携带 `edit_generation`，加载和保存结果同时校验 ID 与访问代次；关闭会使旧代次失效。
- 旧成功、旧失败都不改变新草稿或保存锁；保存失败仍保留当前输入供重试。

### P2：来源编辑没有取消之前的删除确认

先点击一次删除，再打开并关闭编辑，旧 `pending_delete_feed` 仍在，之后一次删除点击就会执行。覆盖导入确认同样残留。修复前新增 reducer 用例实际失败。

- 打开编辑同时取消这些确认及对应提示。关闭编辑后，删除必须重新经过两次点击。
- 浏览器用例在隔离测试数据上实际检查该流程，不删除用户订阅。

### P2：新增浮层未接入 Android 返回优先关闭逻辑

静态审查确认 entries / feeds 页面仍直接使用无 dismiss 的返回 hook，新增来源筛选、目录浮层及编辑表单没有获得返回优先权。

- 来源筛选打开状态上移到页面 Rust signal，页面先关闭来源筛选或目录浮层；不改变列表选择。
- 订阅页通过轻量 session 处理关闭，不为返回回调复制整份订阅快照；编辑保存中消耗返回操作，等待结果，避免离开表单。
- 单元测试覆盖加载中关闭、保存中阻止离开、空闲时关闭、随后允许导航。Android 编译验证与实机事件验证分开记录。

### P2：长原名把编辑操作挤出弹窗

360px 浏览器视口下，连续英文长来源名令宽 320px 的表单滚动宽度达到 3969px，“使用原名”按钮在屏幕外。

- 表单列可收缩，原名允许断行，重置按钮保留自身宽度。修复后同一数据的滚动宽度等于 320px；超长内容只需纵向滚动。

### P3：来源全选框缺少半选状态

实际浏览器确认部分来源被选中时，native checkbox 的 `indeterminate=false` 且没有 `aria-checked`，看起来与全部未选相同。

- Rust 计算三态与无障碍属性，DOM 只同步原生复选框的 `indeterminate` 属性；部分状态点击或空格选择全部。
- 回归覆盖半选、全选、全不选、重新打开半选状态及取消后保留文章勾选。

## 验证与验收

### 自动化验证

- 修复前：`cargo test -p rssr-app --locked -j2 pages::feeds_page::reducer::tests -- --nocapture`：两个新增用例按预期失败，分别复现迟到结果覆盖和确认残留。
- `cargo test -p rssr-app --locked -j2`：107 passed / 1 ignored；包括新增编辑代次、确认重置、返回关闭状态测试。
- `cargo clippy -p rssr-app --all-targets --locked -j2 -- -D warnings`：通过。
- `cargo fmt --all --check`：通过。
- `dx build --platform web --package rssr-app --locked`：通过。已安装 dx 0.7.10 与 Dioxus 0.7.9 版本提示依旧存在，最终构建成功；没有更换工具或依赖。
- `cargo check -p rssr-app --target aarch64-linux-android --locked -j2`：通过；使用现有 NDK 28.2.13676358 / API 24。
- `node scripts/browser/rssr_workspace_acceptance.cjs`：selection / sources / editing / responsive / editEdges 全部通过，页面错误为 0。覆盖 1920、1536、760、390、360px 视口，新增长原名用例在 1536 / 360px 通过。
- `node scripts/browser/rssr_small_viewport_assertions.mjs --cdp-base http://127.0.0.1:9229 --static-base http://127.0.0.1:8098 --artifact-dir target/reading-workspace-review-directory --directory-only true`：125 项断言通过，console errors 为 0；覆盖桌面 / 移动尺寸的时间与来源目录跟随边界。
- `git diff --check`（限定本次文件）：通过。

### 视觉复查与证据

- 独立 Playwright / Edge 浏览器使用隔离的 SPA seed 和 BrowserStore 数据，不接触用户订阅。
- 实际查看 360px 长来源名编辑弹窗截图，横向没有溢出；名称、重置按钮与 URL 可见，纵向可滚动。截图：`target/reading-workspace-review-long-name.png`。
- 目录结果及截图：`target/reading-workspace-review-directory/`；日志：`%TEMP%/rssr-review-{repro,app,clippy,web,android,browser,directory}.log`。
- 用户随后要求查看主页：使用 `542de173` 对应的实际 Web 构建，截取 1920×1080、device scale 1 的默认主页。隔离数据为 72 篇示例文章、每页 8 篇，配中文标题与已读／收藏示例；没有修改 DOM 样式或用户数据。页面错误为 0，截图已目视检查。文件：`C:/Users/QQ/.codex/visualizations/2026/10/22/01a16568-e398-7fb0-9847-e636e6fdecb2/rss-reader-home-542de173-1920x1080.png`。此次仅补截图记录，没有应用代码变更。

## 结果

- 五处问题已修复；本地自动化覆盖上述路径。
- 没有把浏览器、编译或纯 Rust 状态测试当成 Android / WebView2 实机验收。

## 风险与后续事项

- 未实际运行 Android 系统返回手势、软键盘或 Windows WebView2；macOS / Linux 原生宿主也未运行。
- 本轮没有重跑之前实现记录中的完整主题 / Reader / host / 数据库矩阵；此次修复没有修改这些后端语义。静态审查不等于穷尽所有并发及跨端路径。
- 未测量 CPU / 内存性能，没有作性能收益承诺。

## 给下一位 Agent 的备注

- 实现基线见 `2026-10-24-reading-workspace-implementation.md`；本轮重现与修复入口为 feeds reducer / session 和 `rssr_workspace_acceptance.cjs`。
- 工作树原有 `.agents/skills/speckit-*` / `.specify` 删除、两份 2026-10-03 交接修改、未跟踪 Penpot 交接和附件均属于任务外，保留且不纳入提交。
- 本次启动的浏览器与本地测试服务器在收尾时关闭；没有改变 Penpot 或用户浏览器。

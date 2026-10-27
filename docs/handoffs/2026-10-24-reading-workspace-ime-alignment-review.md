# 阅读工作区第五轮审查：输入法 Esc 与分组文字对齐

- 日期：2026-10-24
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 审查基线 / 开始时 HEAD：`9d3428b5`
- 相关 commit：与本记录同一笔本地提交；用 `git log -1 -- docs/handoffs/2026-10-24-reading-workspace-ime-alignment-review.md` 定位
- 相关 tag / release：N/A；没有 push、tag 或发布
- 状态：`validated`（本地自动化；实机覆盖限制见下文）

## 工作摘要

用户要求继续 review and fix。本轮检查来源编辑、归档与阅读流程衔接、选择和分页，以及实际浏览器中的弹窗和键盘操作。修复一处可丢弃来源编辑／筛选草稿的输入法 Esc 问题，并使分组文字对齐已确认的草模约定。

## 影响范围

- 模块：`rssr-app` 共用 `Dialog`、阅读工作区 CSS、工作区浏览器回归脚本。
- 平台：共享 UI 影响 Web / Windows / Linux / macOS / Android；本轮运行验收使用隔离 Edge 浏览器。
- 额外影响：前端交互契约、交接记录。没有增加依赖、crate、数据库迁移或公开接口；单篇 Reader 布局未改变。

## 审查发现与关键变更

### P2：输入法组词期间的 Esc 会丢弃弹窗草稿

- 在来源编辑中先输入自定义名称和新 URL，通过 Chromium CDP 开始真实 composition，再发送普通键盘 Esc。浏览器记录 `KeyboardEvent.isComposing=true`，编辑弹窗却被卸载，未保存的名称与地址丢失。
- 新增 `dialogComposition` 回归用例，修复前实际失败：期望弹窗数量 1，实际 0。失败日志为 `%TEMP%/rssr-review5-ime-before.log`。
- 共用 Dialog 原来对所有 Esc 都执行 Rust `on_close`；来源筛选使用相同组件，同样会丢失筛选草稿。
- 初次修复仅跳过组词时的 Rust 关闭动作；回归发现原生 dialog 仍默认关闭，Rust 保留的节点 `open=false`。没有把节点仍存在误认为修复成功。
- 最终处理同时阻止原生默认关闭与 Rust 关闭动作。组词结束后的 Esc 继续按取消处理，普通关闭、保存和焦点恢复沿用原逻辑。
- 回归检查两个弹窗的节点存在及原生 `open=true`，编辑草稿的名称／URL 保留并可保存，来源勾选草稿保留；普通 Esc 仍关闭弹窗，取消来源筛选不应用草稿、不清空文章多选。

### P3：分组标题未与文章文字对齐

- 已确认的 Penpot v7 约定要求分组文字、文章标题和副标题共用左对齐线，现有 CSS 仍给分组标题 10px 的左内边距。
- 修复前在 1536px 视口测得分组标题 x=254，文章标题／副标题 x=300，相差 46px。
- 将分组标题左内边距改为 56px，与文章行的 4px 内边距、44px 选择区域及 8px 间距之和一致。只调整对齐，不缩小选择点击范围或改变页面外围留白。
- 修复后 1920×1080 时间分组的三处 x 均为 492；760×864 来源分组的三处 x 均为 88。两种视口均无横向溢出，窄窗口目录仍默认收起；分页保持视口底部集中排列。

## 验证与验收

### 自动化验证

- `cargo test --workspace --locked -j2`：394 passed / 2 ignored / 0 failed；随后最终 Esc 默认行为修改另行复跑 app 测试，见下项。
- `cargo test -p rssr-app --locked -j2`：最终代码 111 passed / 1 ignored / 0 failed；日志为 `%TEMP%/rssr-review5-app-corrected.log`。忽略项为既有性能探针。
- `cargo clippy --workspace --all-targets --locked -j2 -- -D warnings`：最终代码通过，日志为 `%TEMP%/rssr-review5-clippy.log`。
- `cargo fmt --all --check`、`node --check scripts/browser/rssr_workspace_acceptance.cjs`：最终代码通过。
- `dx build --platform web --package rssr-app --locked`：最终代码通过，退出码 0。dx 0.7.10 / Dioxus 0.7.9 的既有版本提示仍存在，没有调整依赖或工具版本；最终日志为 `%TEMP%/rssr-review5-web-final.log`。
- `node scripts/browser/rssr_workspace_acceptance.cjs`：最终十二组全部通过，页面错误为 0；包含 selection / sources / editing / responsive / editEdges / queryStability / importNames / sourceIdentity / pendingWrites / preferenceOrdering / shortSourceDialog / dialogComposition。最终日志为 `%TEMP%/rssr-review5-browser.log`。
- `cargo check -p rssr-app --target aarch64-linux-android --locked -j2`：最终代码通过，沿用 NDK 28.2.13676358 / API 24，日志为 `%TEMP%/rssr-review5-android.log`。
- `git diff --check`（限定本轮路径）：通过。
- 中途误用 `cargo test -p rssr-app --lib`，因该包没有 library target 而退出 101；已改用不带 `--lib` 的 app 测试命令，不把这次命令选择错误列为测试通过。

### 浏览器检查与证据

- 使用独立浏览器 profile 与 `directory-contract` 数据，不访问用户订阅数据库。
- 已通过真实控件补查：空来源搜索 → Esc 取消、Tab／空格选择文章、窄窗口目录展开／收起与同页定位保留多选、分页往返清空多选。
- 已目视检查 `target/reading-workspace-review5/home-1920x1080.png` 与 `home-760x864-source.png`；测量结果在同目录 `layout.json`。截图采用测试文章和测试来源别名，不代表用户的真实订阅。
- 输入法回归用 CDP 设置 Chromium composition 状态，再由 Playwright 发送键盘 Esc；这是浏览器级验收，不是 Windows 中文输入法候选窗、WebView2 或 Android 设备的手工验收。
- 本轮未运行原生 WebView2、Android 设备、macOS / Linux 宿主，也没有重新执行未改动目录状态逻辑的 125 项专用套件；工作区套件和补查覆盖本轮相关的目录／选择行为。

## 结果

- 两处修复已通过上述测试、编译和浏览器验收，可供合并审查。没有执行 push、发布或远端工作流。

## 风险与后续事项

- 系统输入法及不同原生宿主的实际键盘行为仍需实机验收；本轮不作跨平台手工通过或性能收益声明。
- 来源 URL 变更的 generation 保护、批量写入范围、归档不改变已读／收藏的语义均沿用现有实现。本轮没有更改 Reader 导航范围或归档定义。

## 给下一位 Agent 的备注

- 入口：`components/dialog.rs` 的 Esc 处理，以及 `reading-workspace.css` 的 `entry-group-header`。
- 工作树已有的 `.agents/skills/speckit-*` / `.specify` 删除、两份 2026-10-03 交接修改、未跟踪 Penpot 交接和附件均保留，不纳入本轮提交。
- 本轮隔离浏览器与本地测试服务器已关闭；任务外浏览器与数据保持原样。

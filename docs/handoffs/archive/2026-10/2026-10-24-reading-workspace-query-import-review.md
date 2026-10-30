# 阅读工作区第二轮审查：查询生命周期与导入原名

- 日期：2026-10-24
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 审查基线 / 开始时 HEAD：`c8e5c3d8`
- 相关 commit：与本记录同一笔本地提交；用 `git log -1 -- docs/handoffs/2026-10-24-reading-workspace-query-import-review.md` 定位
- 相关 tag / release：N/A；没有 push、tag 或发布
- 状态：`validated`（本地自动化；实机覆盖限制见下文）

## 工作摘要

用户要求继续 review and fix。本轮针对文章查询、多选／目录状态隔离和来源名称编辑的导入边界，复现并修复三处 P2 问题。沿用已确定的布局、配色和正文阅读页，没有追加视觉改版。

## 影响范围

- 模块：`rssr-app` entries page 的查询、reducer、session；`rssr-infra` SQLite / BrowserStore 配置导入与 OPML 合并。
- 平台：共享 Web / desktop / Android 页面逻辑；SQLite 影响原生端与 `rssr-web`，BrowserStore 影响独立静态 Web。
- 额外影响：浏览器回归脚本、前端交互契约；没有新 crate、依赖、迁移、配置字段或 host API。

## 审查发现与关键变更

### P2：普通重绘改变归档截止时间，意外重复查询文章

- 修复前，`entry_query` 在每次页面 render 时使用当前时间。跨过整秒后，勾选一篇文章即可改变查询参数并重新加载整份列表，目录和页内分页也受相同依赖影响。
- 在隔离浏览器中改变存储层标题但不主动刷新，再勾选文章，页面出现新标题；新增 `queryStability` 用例在旧构建上实际失败。
- 查询范围与时间分开：只有查询范围、归档设置或刷新代次变化才重新计算归档截止时间。选择、目录和同一结果集内分页继续使用当前查询。

### P2：迟到查询可以覆盖已成功写入的已读／收藏状态

- 原有 generation 仅处理查询之间的竞争，无法识别查询等待期间完成的文章标记或批量写入。
- 新增受控异步用例在修复前实际失败：先保存已读／收藏，再返回旧列表，旧标记覆盖新标记；仅未读列表还会重新出现已读文章。
- 增加页面内写入代次校验。遇到过期结果时丢弃其成功或错误，并确保按当前查询范围重载；批量操作已经安排刷新时不重复追加。没有并发查询时，单条标记仍直接更新当前列表。
- 回归同时检查丢弃旧错误、保留仍可见的选择，以及下一次新查询正常发布。

### P2：导入的自定义名覆盖远端原名，“使用原名”无法恢复

- 修复前，SQLite 与 BrowserStore 在 JSON 配置替换和 OPML 合并时，把导入名称同时写入 `title` 与 `custom_title`。
- 浏览器复现：Alpha 改名 → 导出 → 原样导入 → 编辑，原始名称变成自定义名称。新增 `importNames` 用例在旧构建上实际失败；两条 SQLite 回归用例也实际失败。
- 导入名称只更新自定义名称，保留已有远端原名；新导入来源在实际抓取前不伪造远端原名。BrowserStore 批量 upsert 返回应用自定义名称后的有效名称。
- 不改变现有 JSON / OPML 格式、订阅 ID、文章标记或导入覆盖／合并规则。

## 验证与验收

### 自动化验证

- `cargo test -p rssr-app --locked -j2 query_cannot_restore_flags_or_rows_from_before_a_completed_write`：新增用例修复前失败，修复后通过。
- `cargo test -p rssr-infra --test test_entry_state_and_search --locked -j2 imported_names_preserve_original_title`：两条新增用例修复前失败，修复后通过。
- 第一轮 workspace 测试发现 `config_replacement_derives_removals_from_locked_current_state` 仍断言导入 `title: None` 清空远端标题。按已确定的名称分离语义，改为先设置别名、导入空名称，再验证只清空别名且保留原名；原有删除、计数断言保留。第一轮日志另存为 `%TEMP%/rssr-review2-workspace-first.log`。
- `cargo test --workspace --locked -j2`：390 passed / 2 ignored，0 failed；忽略项为既有性能探针。
- `cargo clippy --workspace --all-targets --locked -j2 -- -D warnings`：通过。
- `cargo fmt --all --check`、`node --check scripts/browser/rssr_workspace_acceptance.cjs`：通过。
- `dx build --platform web --package rssr-app --locked`：通过。dx 0.7.10 对 Dioxus 0.7.9 的既有版本提示仍存在，最终构建成功；没有更新工具或依赖。
- `cargo check -p rssr-app --target aarch64-linux-android --locked -j2`：通过，使用现有 NDK 28.2.13676358 / API 24。
- `cargo check -p rssr-infra --target wasm32-unknown-unknown --locked --test wasm_config_exchange_contract_harness -j2`：通过；这项只做 harness 编译，不宣称执行了 wasm-bindgen-test runner。
- `node scripts/browser/rssr_workspace_acceptance.cjs`：selection / sources / editing / responsive / editEdges / queryStability / importNames 七组全部通过，页面错误为 0；覆盖 1920、1536、760、390、360px 视口。
- `node scripts/browser/rssr_small_viewport_assertions.mjs --cdp-base http://127.0.0.1:9229 --static-base http://127.0.0.1:8098 --artifact-dir target/reading-workspace-review2-directory --directory-only true`：125 项断言通过，console errors 为 0；包括时间／来源目录、手动浏览与重新跟随的边界。
- `git diff --check`（限定本轮路径）：通过。

### 手工验收与证据

- 浏览器使用隔离 Playwright / Edge profile 与 `directory-contract` fixture；没有访问用户订阅数据库。
- 浏览器基线复现覆盖选择引起重查与 JSON 名称往返；运行的是旧实际 Web 构建，不是修改 DOM 的模拟界面。
- 修复后的目录断言及截图保存在 `target/reading-workspace-review2-directory/`。已目视检查 `directory-time-boundary.png`：列表、独立目录与底部分页保持现有样式，目录对应滚动位置。
- 本轮尚未进行 Windows WebView2 / Android 设备手工验收；浏览器与编译结果不替代实机验收。

## 结果

- 三处已定位问题已修复，并通过上述本地测试与浏览器回归。变更可供合并审查；没有执行 push、发布或远端工作流。

## 风险与后续事项

- 没有运行 macOS / Linux 原生宿主，也没有测量 CPU / 内存收益。
- 已被旧导入逻辑覆盖的原名不会凭空恢复；后续成功刷新可重新获得远端标题。未增加数据回填或迁移。
- 审查与回归覆盖列出的路径，不代表穷尽所有跨设备与并发场景。

## 给下一位 Agent 的备注

- 入口：entries page 的 query memo 与 session 写入代次；两套存储适配器的配置替换与批量 upsert。
- 工作树原有 `.agents/skills/speckit-*` / `.specify` 删除、两份 2026-10-03 交接修改、未跟踪 Penpot 交接和附件均保留，不纳入提交。
- 本次启动的隔离浏览器与本地测试服务器已关闭；验证日志使用 `%TEMP%/rssr-review2-*`。

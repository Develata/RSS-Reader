# 阅读工作区第三轮审查：来源身份与写入互斥

- 日期：2026-10-24
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 审查基线 / 开始时 HEAD：`99c29599`
- 相关 commit：与本记录同一笔本地提交；用 `git log -1 -- docs/handoffs/2026-10-24-reading-workspace-source-write-review.md` 定位
- 相关 tag / release：N/A；没有 push、tag 或发布
- 状态：`validated`（本地自动化；实机覆盖限制见下文）

## 工作摘要

用户要求继续 review and fix。本轮复现并修复两处 P2 问题：自定义名称相同的来源被合并为一个目录分组，以及单篇已读／收藏写入与重复点击、批量操作可以重叠。保持当前布局、配色和正文阅读页。

## 影响范围

- 模块：`rssr-app` entries page 的分组投影、列表标识、状态、session、facade 与操作按钮。
- 平台：共享 Web / desktop / Android 页面逻辑；本轮没有修改 application、domain、infra 或 host。
- 额外影响：浏览器回归脚本、前端交互契约。没有新增 crate、依赖、迁移、配置字段或接口。

## 审查发现与关键变更

### P2：自定义名称被误用为来源身份

- 修复前，来源分组按 `feed_title` 建立，目录锚点由显示名生成，列表分组的组件 key 也使用显示名。两个来源改为相同名称后会合并；`A/B` 和 `A B` 等不同名称会生成相同锚点。
- 新 Rust 用例在旧实现上实际失败：不同 feed ID 应得到 2 个来源分组，实际只有 1 个。
- 隔离浏览器中通过真实来源编辑表单将 Alpha、Bravo 均改为 `Same name`，目录从 6 个分组变为 5 个。新增 `sourceIdentity` 用例在旧 Web 构建上实际失败。
- 来源身份改用 `feed_id`，名称继续用于显示。顶级来源锚点为 `entry-source-{feed_id}`，时间视图内来源锚点包含 feed ID 与首篇文章 ID，组件 key 使用唯一锚点。现有年月／日期锚点保持不变。
- 回归覆盖同名、名称标点碰撞、改名后身份不变，以及在同名来源中按 URL 找到并独立筛选第二个来源。目录继续使用完整查询结果，选择仍限制在当前页。

### P2：待完成的单篇写入没有阻止重复或批量操作

- 修复前，单篇已读／收藏没有待完成状态；可在旧标记仍显示时重复提交，也可同时发起批量已读或归档。
- 在隔离 BrowserStore 页面中临时持有现有 Web Lock，使真实写入等待。旧构建仍允许再次点击同一行状态按钮和批量按钮，新增 `pendingWrites` 用例实际失败。没有把这次按钮准入复现夸大为已观察到生产环境数据损坏。
- Rust session 同步记录待完成文章 ID；同一行状态写入互斥，批量操作与所有待完成的单篇写入互斥。按钮同步禁用，Rust guard 同时拒绝重绘前已经排队的重复事件；其他行在单篇写入时仍可用。
- 成功或失败均清除待完成状态。待完成集合不参与分组投影，不因按钮忙碌状态重新查询或重建目录。
- 受控异步单元测试覆盖重复事件、单篇／批量互斥、成功／失败后可重试；浏览器覆盖真实存储锁等待和配额异常后的恢复。

## 验证与验收

### 自动化验证

- `cargo test -p rssr-app --locked -j2 source_identity_does_not_depend_on_custom_display_names`：新增用例修复前失败，修复后通过。
- `cargo test -p rssr-app --locked -j2`：110 passed / 1 ignored，0 failed；忽略项为既有性能探针。
- `cargo clippy -p rssr-app --all-targets --locked -j2 -- -D warnings`：通过。
- `cargo fmt --all --check`：通过。
- `dx build --platform web --package rssr-app --locked`：通过。dx 0.7.10 / Dioxus 0.7.9 的既有版本提示仍存在，最终退出码为 0；未改工具或依赖版本。
- `cargo check -p rssr-app --target aarch64-linux-android --locked -j2`：通过，使用现有 NDK 28.2.13676358 / API 24。
- `node scripts/browser/rssr_workspace_acceptance.cjs`：selection / sources / editing / responsive / editEdges / queryStability / importNames / sourceIdentity / pendingWrites 九组全部通过，页面错误为 0。
- 第一轮目录回归发现三处旧 `entry-group-foxtrot` 测试定位值未同步：实际页面已使用正确的 `entry-source-6`。修正定位值并保留原有行为断言；首次失败日志保存在 `%TEMP%/rssr-review3-directory-first.log`。
- `node scripts/browser/rssr_small_viewport_assertions.mjs --cdp-base http://127.0.0.1:9229 --static-base http://127.0.0.1:8098 --artifact-dir target/reading-workspace-review3-directory --directory-only true`：重跑后 125 / 125 项通过，未忽略的 console errors 为 0；16 条静态测试服务器缺少开发热更新端点的已知连接错误由现有脚本单独记录，不属于应用错误。
- `node --check scripts/browser/rssr_workspace_acceptance.cjs`、`node --check scripts/browser/directory_contract.mjs`、`git diff --check`（限定本轮路径）：通过。

### 浏览器验收与证据

- 使用隔离 Playwright / Edge profile 与 `directory-contract` fixture，没有访问用户订阅数据库。失败注入与延迟只用于测试存储行为，没有修改页面 DOM 来模拟产品结果。
- 目视检查实际 Web 构建的 1536×864 首页、760×900 目录和来源弹窗；取消筛选与目录浏览保留勾选，切换分组清空勾选。来源弹窗没有横向溢出。
- 首页截图：`target/reading-workspace-review3-directory/home-1536x864.png`，采用测试文章数据；目录断言和相关截图也保存在该目录。
- 新增两组回归在旧构建中失败、修复后通过；日志位于 `%TEMP%/rssr-review3-*`。
- Windows WebView2、Android 设备以及 macOS / Linux 原生宿主未做手工验收；浏览器与交叉编译不能替代实机验收。

## 结果

- 两处问题已修复，通过上述本地测试、编译与浏览器回归，可供合并审查。没有执行 push、发布或远端工作流。

## 风险与后续事项

- 写入互斥属于当前页面 session，不增加跨窗口或跨客户端事务能力；底层存储沿用已有一致性机制。
- 没有测量 CPU / 内存收益，没有运行本轮范围外的完整工作区测试；上轮工作区测试数字不作为本轮新证据。
- 审查与测试覆盖列出的路径，不代表穷尽所有并发与跨设备场景。

## 给下一位 Agent 的备注

- 入口：`entries_page/groups.rs`、`presenter.rs` 的来源身份，以及 `session.rs` 的 `spawn_entry_write` 与 `spawn_bulk`。
- 工作树原有 `.agents/skills/speckit-*` / `.specify` 删除、两份 2026-10-03 交接修改、未跟踪 Penpot 交接和附件均保留，不纳入本轮提交。
- 本轮隔离浏览器与本地测试服务器已关闭；目录首次失败证据与最终通过结果均保留在 `target/reading-workspace-review3-directory/`。

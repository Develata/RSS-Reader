# 阅读工作区第四轮审查：偏好保存顺序与来源弹窗高度

- 日期：2026-10-24
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 审查基线 / 开始时 HEAD：`53205db6`
- 相关 commit：与本记录同一笔本地提交；用 `git log -1 -- docs/handoffs/2026-10-24-reading-workspace-preference-review.md` 定位
- 相关 tag / release：N/A；没有 push、tag 或发布
- 状态：`validated`（本地自动化；实机覆盖限制见下文）

## 工作摘要

用户要求继续 review and fix。本轮检查筛选、分组偏好、多选、来源编辑及弹窗边界，复现一处偏好异步保存竞争和一处低高度窗口布局问题。沿用已有布局、配色与正文阅读页。

## 影响范围

- 模块：`rssr-application` 的 `EntriesWorkspaceService`、`rssr-app` entries session 的偏好任务、来源筛选弹窗 CSS、浏览器回归脚本。
- 平台：共享 application 逻辑影响 desktop / Android / Web / `rssr-web`；CSS 影响各 WebView / 浏览器界面。
- 额外影响：前端交互契约与交接文档。未增加 crate、依赖、数据库迁移、公开接口或配置字段。

## 审查发现与关键变更

### P2：快速恢复原筛选时，最后一次选择可能没有保存

- 复现：阻塞隔离 BrowserStore 的现有 Web Lock，点击“仅未读”再点击取消。界面回到全部文章；释放锁、等待写入结束并重新加载后，却又变成“仅未读”。新增浏览器 `preferenceOrdering` 在旧构建上实际失败。
- 根因：`save_workspace_if_changed` 的比较和保存可重叠。后一次请求读取到尚未变化的旧值，误判为无需写入；前一次较慢的保存随后覆盖最终选择。
- 新增可控延迟的 Rust 回归用例，在旧实现上实际失败：期望 `ReadFilter::All`，实际为 `UnreadOnly`。
- 在 `EntriesWorkspaceService` 内使用共享异步锁串行执行整个比较／写入过程；克隆的服务共享同一把锁，前一次成功或失败均释放等待。保留“相同值不写入”的行为。
- 仅增加保存端互斥后，独立用例与首次十组回归通过，但后续完整回归仍复现问题。诊断确认点击前为全部、两次点击后也为全部，重新加载却为仅未读；没有把这次失败视作按钮未点击成功而跳过。
- 补充 entries session 执行前校验：尚未开始的异步任务须与页面当前的分组、归档显示、状态筛选和来源集合一致，才允许进入保存流程。已过时的捕获值直接丢弃，避免较晚执行的旧任务重新保存旧偏好。新增 Rust 用例检查同一轮排队后只执行最终选择。
- 锁只约束工作区偏好保存，不阻塞文章查询，不新增前后端职责划分或 DOM 状态规则。

### P3：较矮窗口的来源弹窗将底部按钮挤出可见区域

- 760×400 视口中，来源列表与其他内容叠加超过弹窗高度，“取消／应用”需滚动整个弹窗才能看到。已目视确认并用新增 `shortSourceDialog` 用例复现失败。
- 将锚定弹窗内容改为受视口高度约束的纵向布局；来源列表可以收缩并独立滚动，其他控件保持点击区域。未改变配色、按钮样式或正常窗口的列表最大高度。
- 回归检查 760×400 和 360×480 的底部按钮完整可见；滚动选择最后一个来源后，仍能直接应用并得到该来源的文章。

## 验证与验收

### 自动化验证

- `cargo test -p rssr-application --locked -j2 overlapping_preferences_preserve_the_latest_choice_after_success_or_failure`：新增用例修复前失败。
- `cargo test -p rssr-application --locked -j2`：修复后 54 passed / 0 failed；包括前一次保存失败后，后一请求仍可完成。
- `cargo fmt --all --check`、`node --check scripts/browser/rssr_workspace_acceptance.cjs`：通过。
- 首次 `dx build --platform web --package rssr-app --locked`：通过；修复保存顺序后，原有九组浏览器用例与新增 `preferenceOrdering` 十组全部通过，页面错误为 0。
- `cargo test --workspace --locked -j2`：最终代码 394 passed / 2 ignored / 0 failed，包括新增的排队偏好任务校验。忽略项为既有性能探针，最终日志为 `rssr-review4-workspace-final.log`。
- `cargo clippy --workspace --all-targets --locked -j2 -- -D warnings`：最终代码通过，日志为 `rssr-review4-clippy-final.log`。
- `dx build --platform web --package rssr-app --locked`：最终代码通过，退出码为 0，日志为 `rssr-review4-web-guard.log`。dx 0.7.10 / Dioxus 0.7.9 的既有版本提示仍存在；没有调整工具或依赖版本。
- `cargo check -p rssr-app --target aarch64-linux-android --locked -j2`：最终代码通过，现有 NDK 28.2.13676358 / API 24，日志为 `rssr-review4-android-final.log`。
- `node scripts/browser/rssr_workspace_acceptance.cjs`：最终十一组全部通过，页面错误为 0。包括 selection / sources / editing / responsive / editEdges / queryStability / importNames / sourceIdentity / pendingWrites / preferenceOrdering / shortSourceDialog，最终日志为 `rssr-review4-browser-guard.log`。
- 中途完整浏览器回归的两次失败保留在 `rssr-review4-browser-final-first.log` 和 `rssr-review4-browser-diagnostic.log`；后者包含点击前后均为全部、重载后为仅未读的诊断。补充 UI 执行前校验后重新运行整组通过，没有删除或放宽持久化断言。
- `git diff --check`（限定本轮路径）：通过。

### 浏览器验收与证据

- 使用隔离 Edge / Playwright profile 与 `directory-contract` 测试数据，没有访问用户订阅数据库。
- 已通过真实控件检查：已读筛选得到空结果 → 清除筛选恢复 72 篇；来源搜索无匹配 → Esc 取消后保留原有文章选择；快速状态筛选与分组往返后重新加载仍保持最终选择。
- 低高度弹窗修复前截图：`target/reading-workspace-review4/source-short-before.png`；修复后为 `source-short-after.png`，正常 1536×864 弹窗为 `source-normal-after.png`，均在同一目录。已目视确认低高度按钮完整可见、正常尺寸来源列表保持原有高度；Esc 关闭后焦点返回入口。
- 日志：`%TEMP%/rssr-review4-*`。
- Windows WebView2 / Android 设备以及 macOS / Linux 原生宿主未做手工验收；没有执行远端发布或 Docker 部署验收。

## 结果

- 两处问题已修复，通过上述最终代码的测试、编译和浏览器回归，可供合并审查。没有执行 push、发布或远端工作流。

## 风险与后续事项

- 偏好锁的范围是共享服务实例及其克隆，不是跨浏览器／跨进程的配置冲突合并；存储仍使用原有一致性机制。
- 没有改变来源 URL 编辑、文章状态或批量事务语义，没有测量性能或内存收益。
- 本轮不重复运行未改动的 125 项目录回归；上一轮数字不作为本轮新证据，多选／目录保留行为仍由工作区浏览器用例覆盖。

## 给下一位 Agent 的备注

- 入口：`entries_workspace_service.rs` 的 `save_workspace_if_changed`，以及 `reading-workspace.css` 的锚定弹窗内容布局。
- 工作树已有的 `.agents/skills/speckit-*` / `.specify` 删除、两份 2026-10-03 交接修改、未跟踪 Penpot 交接和附件均保留，不纳入本轮提交。
- 本轮隔离浏览器与本地测试服务器已关闭；任务外浏览器与数据保持原样。

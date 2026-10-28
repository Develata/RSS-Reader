# 历史交接索引

核对日期：2026-10-28。本目录归档 v0.2.0 发布前（2026-10-23 之前）的 147 份执行记录，按月份保留原文件名。近期记录见[交接入口](../README.md)，现行行为与维护重点见[文档索引](../../README.md)。

归档只改变检索位置，不代表每一项历史风险都已关闭。正文中的日期、源码身份、测试结果和当时待办保持历史语义；失效源码链接固定到主线删除前仍包含该文件的提交，这仅便于追溯，不证明其就是当时的测试产物。

## 按月份查找

### 2026-10（55 份）

| 日期 | 记录 |
| --- | --- |
| 2026-10-22 | [发布候选：真实 RSS 数学公式保真](./2026-10/2026-10-22-reader-mathml-integration.md) |
| 2026-10-22 | [阶段 D：Pages 内存 SQLite 与发布候选验收](./2026-10/2026-10-22-phase-d-pages-sqlite.md) |
| 2026-10-22 | [Pages 预抓取真实 RSS 文章](./2026-10/2026-10-22-pages-rss-snapshot.md) |
| 2026-10-22 | [CI 三个请求收尾超时的调查与驱动修复](./2026-10/2026-10-22-ci-acceptance-timeouts.md) |
| 2026-10-22 | [C6 Web 服务发布包、镜像与迁移](./2026-10/2026-10-22-c6-web-distribution.md) |
| 2026-10-22 | [C4c-2 第二轮修复复验与 C4c-3 文档](./2026-10/2026-10-22-c4c2-verification-c4c3.md) |
| 2026-10-22 | [C4c 第三轮独立复审材料](./2026-10/2026-10-22-c4c-round3-review-package.md) |
| 2026-10-22 | [C4c 第三轮复核、修复与 C6 准入](./2026-10/2026-10-22-c4c-round3-closure.md) |
| 2026-10-22 | [C4c 远端推送与 CI 验证](./2026-10/2026-10-22-c4c-remote-ci.md) |
| 2026-10-22 | [C4c CI 覆盖补齐](./2026-10/2026-10-22-c4c-ci-coverage.md) |
| 2026-10-22 | [v0.2 与主干、公式防护分支的整合](./2026-10/2026-10-22-branch-integration.md) |
| 2026-10-21 | [不受信 URL 的解析上限：`url` 根上修复、feed 结构预检与自有入口预检](./2026-10/2026-10-21-untrusted-url-guard.md) |
| 2026-10-21 | [资料库：程序退出前关闭并等待连接；身份探测认出检查点中途的主文件](./2026-10/2026-10-21-store-close-before-exit.md) |
| 2026-10-21 | [CLI 输出契约测试：工作区目录名在进程内唯一](./2026-10/2026-10-21-cli-test-workspace-names.md) |
| 2026-10-20 | [资料库文件层：检查点探测测试受负载影响](./2026-10/2026-10-20-store-checkpoint-test-load.md) |
| 2026-10-20 | [rssr-web 服务端模式设计草案](./2026-10/2026-10-20-rssr-web-server-mode-design.md) |
| 2026-10-20 | [P1：rssr-web 服务端宿主与 `/api/v1`](./2026-10/2026-10-20-rssr-web-server-api.md) |
| 2026-10-20 | [P2：浏览器界面接入 rssr-web 服务端](./2026-10/2026-10-20-rssr-web-remote-ui.md) |
| 2026-10-20 | [P3：rssr-web 的 Linux 二进制发布与文档修订](./2026-10/2026-10-20-rssr-web-release.md) |
| 2026-10-20 | [P0：抽出原生宿主 crate `rssr-host`](./2026-10/2026-10-20-rssr-host-extraction.md) |
| 2026-10-20 | [退役 `/feed-proxy`](./2026-10/2026-10-20-retire-feed-proxy.md) |
| 2026-10-20 | [阅读页正文解析预算：超出预算整篇降级为纯文本](./2026-10/2026-10-20-reader-parse-budget.md) |
| 2026-10-20 | [阅读页正文深层嵌套的超线性消毒成本：调查与方案](./2026-10/2026-10-20-reader-deep-nesting-investigation.md) |
| 2026-10-20 | [正文图片地址归一化的超线性路径：URL 展开预算与实体解码窗口](./2026-10/2026-10-20-live-display-normalize-bounds.md) |
| 2026-10-20 | [订阅抓取改用空闲读超时 + 整体上限](./2026-10/2026-10-20-feed-fetch-idle-timeout.md) |
| 2026-10-20 | [订阅抓取改用空闲读超时 + 整体上限](./2026-10/2026-10-20-feed-fetch-idle-timeout-reader-mathml.md) |
| 2026-10-19 | [v0.2 阶段 C4：网页端改用服务器](./2026-10/2026-10-19-v0.2-phase-c4.md) |
| 2026-10-19 | [阅读页 MathML 公式保留、预算与 KaTeX 去重](./2026-10/2026-10-19-reader-mathml.md) |
| 2026-10-18 | [wasm-bindgen 升到 0.2.128，harness 脚本核对运行器版本](./2026-10/2026-10-18-wasm-bindgen-0.2.128.md) |
| 2026-10-18 | [阅读页底栏固定与快捷键；Android 系统返回键](./2026-10/2026-10-18-reader-bar-and-android-back.md) |
| 2026-10-16 | [资料库执行器：坏连接替换测试的偶发超时](./2026-10/2026-10-16-store-broken-connection-test-flake.md) |
| 2026-10-15 | [v0.2 阶段 C：服务器端 Web](./2026-10/2026-10-15-v0.2-phase-c.md) |
| 2026-10-14 | [v0.2 阶段 B：后端接缝、共享 composition、公开契约调整](./2026-10/2026-10-14-v0.2-phase-b.md) |
| 2026-10-13 | [v0.2 架构决策：服务器端 Web、统一 SQLite 存储与 Pages 演示](./2026-10/2026-10-13-v0.2-architecture.md) |
| 2026-10-12 | [B6 与 D5 指定提交范围的 Astra 评审](./2026-10/2026-10-12-b6-astra-review.md) |
| 2026-10-09 | [工程审计（2026-10）、三项最小修复与独立评审跟进](./2026-10/2026-10-09-engineering-audit.md) |
| 2026-10-09 | [2026-10 工程审计发现的修复](./2026-10/2026-10-09-audit-fixes.md) |
| 2026-10-07 | [原生图片 Referer 与 WebDAV 出站边界](./2026-10/2026-10-07-native-outbound-request-safety.md) |
| 2026-10-07 | [PR #25 合入当前 main 的组合验收](./2026-10/2026-10-07-feed-proxy-main-integration.md) |
| 2026-10-07 | [/feed-proxy 不可信文档隔离](./2026-10/2026-10-07-feed-proxy-content-isolation.md) |
| 2026-10-07 | [Windows 目录自然展开滚动排查与回归](./2026-10/2026-10-07-directory-natural-scroll.md) |
| 2026-10-06 | [web-auth 退出观察延迟归因与有限轮询优化](./2026-10/2026-10-06-web-auth-polling-latency.md) |
| 2026-10-06 | [web-auth 清理等待与 Windows 路径阻塞修复](./2026-10/2026-10-06-web-auth-blocker-fixes.md) |
| 2026-10-05 | [web-auth Rust 生命周期迁移：阶段 1a](./2026-10/2026-10-05-web-auth-owned-process.md) |
| 2026-10-05 | [macOS 发布包版本元数据同步](./2026-10/2026-10-05-macos-bundle-release-version.md) |
| 2026-10-04 | [v0.1.22 发布与版本标识收尾](./2026-10/2026-10-04-v0.1.22-release.md) |
| 2026-10-04 | [v0.1.22 发布前验收（发布阻塞）](./2026-10/2026-10-04-v0.1.22-pre-release-acceptance.md) |
| 2026-10-04 | [Release UI launcher：遵循 Cargo 配置的编译器](./2026-10/2026-10-04-release-ui-configured-compiler.md) |
| 2026-10-03 | [发布验收入口 Rust 化：本地 fixture 验收与独立 Draft PR](./2026-10/2026-10-03-release-ui-runner-rust.md) |
| 2026-10-03 | [PR #20 审阅修复：工具选择、host 产物、背压与 Git provenance](./2026-10/2026-10-03-release-ui-runner-review-fixes.md) |
| 2026-10-03 | [文章列表目录交互意图澄清与设计记录](./2026-10/2026-10-03-entries-directory-follow-design.md) |
| 2026-10-03 | [PR19 目录状态回归与性能测量校正](./2026-10/2026-10-03-directory-review-correction.md) |
| 2026-10-03 | [文章列表目录 Rust 状态统一与新契约验收](./2026-10/2026-10-03-directory-follow-rust.md) |
| 2026-10-02 | [目录行为回归与最小 GUI 性能基线](./2026-10/2026-10-02-directory-regression-baseline.md) |
| 2026-10-01 | [审计修复后的反向复核与修补](./2026-10/2026-10-01-post-audit-repair.md) |

### 2026-09（45 份）

| 日期 | 记录 |
| --- | --- |
| 2026-09-30 | [v0.1.21 发布](./2026-09/2026-09-30-v0.1.21-release.md) |
| 2026-09-30 | [v0.1.21 发布准备与 CI 复核](./2026-09/2026-09-30-v0.1.21-release-readiness.md) |
| 2026-09-30 | [UI 优化建议复核](./2026-09/2026-09-30-ui-review-reassessment.md) |
| 2026-09-30 | [UI 焦点、语义与异步反馈完善](./2026-09/2026-09-30-ui-accessibility-feedback.md) |
| 2026-09-30 | [主线推送前审查、空态修复与 Pages 合并](./2026-09/2026-09-30-review-fix-ci.md) |
| 2026-09-30 | [日常阅读与筛选流程审查](./2026-09/2026-09-30-primary-workflow-review.md) |
| 2026-09-30 | [未读导航、筛选反馈与阅读布局优化](./2026-09/2026-09-30-primary-workflow-optimizations.md) |
| 2026-09-30 | [当前文档与源码契约对齐](./2026-09/2026-09-30-documentation-refresh.md) |
| 2026-09-28 | [GitHub Pages 接入补丁](./2026-09/2026-09-28-github-pages-demo.md) |
| 2026-09-26 | [Web 多标签与跨片存储一致性修复](./2026-09/2026-09-26-web-storage-consistency.md) |
| 2026-09-26 | [v0.1.20 发布](./2026-09/2026-09-26-v0.1.20-release.md) |
| 2026-09-26 | [刷新响应上限与重复正文写入优化](./2026-09/2026-09-26-refresh-bounds-and-write-cost.md) |
| 2026-09-26 | [阅读页上下滚动性能诊断](./2026-09/2026-09-26-reader-scroll-performance.md) |
| 2026-09-26 | [阅读位置改为操作时采集](./2026-09/2026-09-26-reader-operation-position-capture.md) |
| 2026-09-26 | [主线推送与发布前浏览器验收修复](./2026-09/2026-09-26-push-and-release-readiness.md) |
| 2026-09-26 | [任务 2–5：开发依赖补齐、主线集成与最终复验](./2026-09/2026-09-26-integration-revalidation.md) |
| 2026-09-26 | [全局职责与性能检查、顶栏收起](./2026-09/2026-09-26-global-review-collapsible-navigation.md) |
| 2026-09-26 | [高内聚低耦合与过度优化复查](./2026-09/2026-09-26-cohesion-and-optimization-review.md) |
| 2026-09-25 | [任务 5：刷新真实新增计数](./2026-09/2026-09-25-task5-refresh-inserted-count.md) |
| 2026-09-25 | [任务 4：网站首页订阅发现](./2026-09/2026-09-25-task4-subscription-discovery.md) |
| 2026-09-25 | [任务 3：按当前筛选批量标为已读](./2026-09/2026-09-25-task3-filtered-mark-read.md) |
| 2026-09-25 | [任务 2：本次运行内的位置恢复](./2026-09/2026-09-25-task2-reading-position.md) |
| 2026-09-25 | [任务 1：原生、Android 与浏览器契约补验](./2026-09/2026-09-25-task1-platform-validation.md) |
| 2026-09-25 | [任务 1：阅读元信息、来源全部未读数与设备本地时间](./2026-09/2026-09-25-reader-metadata-source-unread.md) |
| 2026-09-24 | [v0.1.19 发布](./2026-09/2026-09-24-v0.1.19-release.md) |
| 2026-09-24 | [v0.1.18 发布与边界回归验证](./2026-09/2026-09-24-v0.1.18-release.md) |
| 2026-09-24 | [全局检查：SQLite 身份、Web 网络与回跳、Debian 输出边界](./2026-09/2026-09-24-global-boundary-review.md) |
| 2026-09-24 | [CI 关键路径、wasm Clippy 缺口与文章列表动作层级](./2026-09/2026-09-24-ci-critical-path-and-entry-actions.md) |
| 2026-09-23 | [v0.1.17 发布与包验收](./2026-09/2026-09-23-v0.1.17-release.md) |
| 2026-09-23 | [v0.1.16 tag 与发布核验](./2026-09/2026-09-23-v0.1.16-release.md) |
| 2026-09-23 | [README 实际界面预览图](./2026-09/2026-09-23-readme-overview-screenshot.md) |
| 2026-09-23 | [README 与使用、部署文档整理](./2026-09/2026-09-23-readme-docs-navigation.md) |
| 2026-09-23 | [README 发布、CI、许可证与平台徽章](./2026-09/2026-09-23-readme-badges.md) |
| 2026-09-23 | [Linux .deb 普通用户数据目录修复](./2026-09/2026-09-23-linux-deb-user-data.md) |
| 2026-09-23 | [Home 刷新反馈、窄侧栏搜索与原生右键菜单](./2026-09/2026-09-23-home-search-context-menu.md) |
| 2026-09-23 | [全局风险审查：Linux 包、CLI 刷新与 Web 代理](./2026-09/2026-09-23-global-review-package-cli-proxy.md) |
| 2026-09-23 | [Amethyst Glass 小屏搜索宽度 CI 修复](./2026-09/2026-09-23-amethyst-search-ci.md) |
| 2026-09-22 | [Windows 原生阅读体验、首次列表偏好与跨端构建复核](./2026-09/2026-09-22-windows-native-ux-performance.md) |
| 2026-09-22 | [v0.1.15 正式发布](./2026-09/2026-09-22-v0.1.15-release.md) |
| 2026-09-22 | [Rust 验收执行、失败恢复与订阅交互优化](./2026-09/2026-09-22-rust-acceptance-refresh-recovery.md) |
| 2026-09-22 | [Home / Read、共享刷新与 Reader 交互](./2026-09/2026-09-22-home-reader-shared-interactions.md) |
| 2026-09-22 | [Home / Reader 交互复审修复](./2026-09/2026-09-22-home-reader-review-fixes.md) |
| 2026-09-22 | [全局工程质量、并发 CI 与阅读体验优化](./2026-09/2026-09-22-global-quality-ci-ux.md) |
| 2026-09-22 | [CI 模块并发、Rust 产物复用与 Actions 版本更新](./2026-09/2026-09-22-ci-matrix-actions-modernization.md) |
| 2026-09-22 | [既有改动分批本地提交](./2026-09/2026-09-22-batched-local-commits.md) |

### 2026-08（2 份）

| 日期 | 记录 |
| --- | --- |
| 2026-08-02 | [UI 批次 2 审查修复、Android 主题导出与 safe-area 预演](./2026-08/2026-08-02-theme-verbs-chip-truncate-directory-hint.md) |
| 2026-08-01 | [7 类「丑」框架 UI 审计与确认项修复（移动端优先）](./2026-08/2026-08-01-ui-audit-7-ugliness-framework.md) |

### 2026-07（18 份）

| 日期 | 记录 |
| --- | --- |
| 2026-07-31 | [移动端 UI P1/P2 修复：data-nav 作用域、触控目标、信息组织、平台健壮性](./2026-07/2026-07-31-mobile-ui-p1p2-fix.md) |
| 2026-07-31 | [移动端 UI P0 修复：极端内容溢出、按钮粘连、分页样式](./2026-07/2026-07-31-mobile-ui-p0-fix.md) |
| 2026-07-31 | [移动端 UI 审计（Vibe-Coding 七类框架 + claude-code 清单复验）](./2026-07/2026-07-31-mobile-ui-audit.md) |
| 2026-07-27 | [Web 端刷新落盘合并为整轮一次](./2026-07/2026-07-27-web-refresh-write-batch.md) |
| 2026-07-27 | [Android 正式签名发布链打通与发布断言修正](./2026-07/2026-07-27-android-release-signing.md) |
| 2026-07-26 | [顶栏偏好跨平台持久化、搜索框失焦修复与死代理清理](./2026-07/2026-07-26-shell-prefs-nav-refresh-and-dead-proxy-removal.md) |
| 2026-07-26 | [全仓审计：越界设置 panic、抓取超时与跨平台排序一致性](./2026-07/2026-07-26-repo-wide-audit-minor-fixes.md) |
| 2026-07-26 | [阅读页标记切换不再整页重载；自定义 CSS 校验补上导入侧；设置数值输入不再静默吞输入](./2026-07/2026-07-26-reader-toggle-and-boundary-validation.md) |
| 2026-07-26 | [文章页 presenter memo 依赖收窄 + 服务端会话过期恢复路径](./2026-07/2026-07-26-presenter-memo-and-server-session-recovery.md) |
| 2026-07-26 | [性能优化轮：文章页克隆、渲染期克隆、刷新并发与 WAL](./2026-07/2026-07-26-performance-optimization-round.md) |
| 2026-07-26 | [文章页分组树改为携带条目引用，切换已读/收藏不再重建](./2026-07/2026-07-26-entries-grouping-tree-index-payload.md) |
| 2026-07-26 | [全仓审计整改（第二轮）：校验收敛、正文处理下沉、归档筛选下推、WebDAV 认证](./2026-07/2026-07-26-audit-remediation-round-2.md) |
| 2026-07-22 | [CI/Release 工作流 action 升级：消除 Node 20 弃用警告](./2026-07/2026-07-22-workflow-actions-node24-bump.md) |
| 2026-07-22 | [文章列表页 facade 逐卡片深拷贝导致的内存放大修复](./2026-07/2026-07-22-entries-page-facade-clone-memory.md) |
| 2026-07-22 | [内置主题优化：暗色变体、性能加固与 Amethyst Glass 更名](./2026-07/2026-07-22-builtin-theme-overhaul.md) |
| 2026-07-21 | [2026-07-21 Dioxus 工具链升级 0.7.3 → 0.7.9](./2026-07/2026-07-21-dioxus-0-7-9-adoption.md) |
| 2026-07-21 | [2026-07-21 Token 契约加固、内置主题瘦身与 Apple 风格按钮](./2026-07/2026-07-21-css-token-hardening-and-apple-buttons.md) |
| 2026-07-20 | [2026-07-20 默认 CSS token 化与暗色模式修复](./2026-07/2026-07-20-default-css-tokenization.md) |

### 2026-04（27 份）

| 日期 | 记录 |
| --- | --- |
| 2026-04-17 | [阅读页正文图片本地化修复](./2026-04/2026-04-17-reader-image-localization.md) |
| 2026-04-16 | [index.db + content.db 双库改造](./2026-04/2026-04-16-index-content-split.md) |
| 2026-04-15 | [Agent 架构护栏补充](./2026-04/2026-04-15-agent-architecture-guardrails.md) |
| 2026-04-14 | [rssr-infra wasm 测试目标分流修复](./2026-04/2026-04-14-rssr-infra-wasm-test-gating.md) |
| 2026-04-14 | [文章页分页浏览、每页条数设置与目录轻量跟随](./2026-04/2026-04-14-entries-page-pagination.md) |
| 2026-04-13 | [Shared Composition Builder](./2026-04/2026-04-13-shared-composition-builder.md) |
| 2026-04-13 | [Post Cleanup Full Regression](./2026-04/2026-04-13-post-cleanup-full-regression.md) |
| 2026-04-13 | [NVIDIA Feed 图片加载 Smoke](./2026-04/2026-04-13-nvidia-feed-image-smoke.md) |
| 2026-04-13 | [Fetch Response Classification](./2026-04/2026-04-13-fetch-response-classification.md) |
| 2026-04-13 | [Fetch Client File Split](./2026-04/2026-04-13-fetch-client-file-split.md) |
| 2026-04-13 | [Feed Normalization And Localizer Stability](./2026-04/2026-04-13-feed-normalization-and-localizer-stability.md) |
| 2026-04-13 | [文章页大列表性能优化](./2026-04/2026-04-13-entry-page-large-list-performance.md) |
| 2026-04-13 | [文档整理：CSS 基线与测试目录](./2026-04/2026-04-13-docs-testing-cleanup.md) |
| 2026-04-13 | [2026-04-13 Daily Rollup](./2026-04/2026-04-13-daily-rollup.md) |
| 2026-04-13 | [Config Import Summary Dedup](./2026-04/2026-04-13-config-import-summary-dedup.md) |
| 2026-04-13 | [Client Localizer Decomposition](./2026-04/2026-04-13-client-localizer-decomposition.md) |
| 2026-04-13 | [Client Image Html Module Split](./2026-04/2026-04-13-client-image-html-module-split.md) |
| 2026-04-13 | [Auth Login Page Renderer Split](./2026-04/2026-04-13-auth-login-page-renderer-split.md) |
| 2026-04-13 | [Auth Login Flow Helpers](./2026-04/2026-04-13-auth-login-flow-helpers.md) |
| 2026-04-13 | [App State Cleanup Port Unification](./2026-04/2026-04-13-app-state-cleanup-port-unification.md) |
| 2026-04-12 | [2026-04-12 Daily Rollup](./2026-04/2026-04-12-daily-rollup.md) |
| 2026-04-11 | [2026-04-11 Daily Rollup](./2026-04/2026-04-11-daily-rollup.md) |
| 2026-04-10 | [2026-04-10 交接汇总](./2026-04/2026-04-10-daily-rollup.md) |
| 2026-04-09 | [2026-04-09 交接汇总](./2026-04/2026-04-09-daily-rollup.md) |
| 2026-04-08 | [2026-04-08 交接汇总](./2026-04/2026-04-08-daily-rollup.md) |
| 2026-04-07 | [2026-04-07 交接汇总](./2026-04/2026-04-07-daily-rollup.md) |
| 2026-04-06 | [2026-04-06 交接汇总](./2026-04/2026-04-06-daily-rollup.md) |

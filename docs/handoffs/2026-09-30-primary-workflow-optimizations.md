# 未读导航、筛选反馈与阅读布局优化

- 日期：2026-09-30
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：`f2eb108`（最后一批代码提交；本记录随回归与文档批次提交）
- 相关 commit：`e9442b6`、`004dcb0`、`f2eb108` 及本记录所在的回归与文档提交；审查记录为 `e052ee6`
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

用户确认按[前置审查](./2026-09-30-primary-workflow-review.md)实施四项优化。修正未读导航回退、Amethyst Glass 窄屏按钮覆盖和元信息留白、筛选摘要与空态，以及阅读操作成功反馈引起的正文位移。

## 影响范围

- 模块：`rssr-app` 的 Reader / Entries 页面、`StatusBanner`、基础列表样式和 Amethyst Glass 预设。
- 平台：共享 UI 影响 Web、desktop、Android。实际交互验收使用 Chromium；原生 Linux、Web wasm 与 Android ARM64 编译检查通过。
- 文档 / workflow：更新用户指南、前端命令与主题接口、发布 UI 清单；增强既有 small viewport smoke，因此既有 CI 五主题任务会覆盖新增断言。没有修改 workflow 文件。
- 数据与依赖：没有 application / domain / infra、数据库迁移、配置格式或依赖变化；沿用现有偏好保存与查询路径。

## 关键变更

### 未读导航与阅读反馈

- 上一 / 下一未读和方向快捷键只取未读目标；没有目标时禁用按钮并提供 title 说明。保留独立同订阅文章导航。
- `StatusBanner` 增加默认关闭的 `visually_hidden` 呈现选项。Reader 的成功反馈使用既有 `sr-only` live region 和按钮状态；错误仍显示状态条。其他页面的反馈呈现沿用原行为。
- 状态节点持续挂载、polite / atomic 播报语义和正文 DOM 保留，成功操作不再将正文向下推移。

### 筛选摘要与空态

- 面板外的 `entry-filter-summary` 显示当前搜索、已读、收藏和来源条件；单来源显示名称，多来源显示数量。单订阅路由不显示未参与查询的来源偏好。
- `clear-entry-filters` 一次清除搜索和上述条件、回到第一页；保留分组方式、归档显示偏好及当前订阅路由。复用查询、持久化和批量预览取消逻辑。
- 空结果区分：没有订阅、订阅尚无文章、文章已归档、当前筛选无匹配。筛选无匹配时仍保留摘要和清除入口。
- 新增稳定接口为 `data-layout="entry-filter-summary"`、`data-slot="entry-filter-summary-text"`、`data-action="clear-entry-filters"`，已记录到两份界面契约。

### Amethyst Glass

- 将整体元信息分隔线和留白从逐行 `reader-meta` 移至 `reader-meta-block`。
- 移除标已读 / 收藏按钮的 `6rem` 最小宽度，由底栏等宽网格分配空间，避免覆盖相邻按钮。
- 冻结修改前 CSS 为 `legacy/amethyst-glass-v2.css` 并加入预设识别表；已验证与 `06bf9df` 的原 CSS 逐字节相同。不覆盖用户已保存的 CSS，重新应用预设后才取得新布局。

### 本地提交拆分

- 用户要求分批 commit 后，将原本地实现提交 `0c046ba` 按职责拆成四批：`e9442b6` 阅读导航与反馈、`004dcb0` Amethyst 布局、`f2eb108` 列表筛选，以及本记录所在的回归与文档提交。
- 拆分基线为 `e052ee6`，更早提交保持原样；原提交保留于本地备份分支 `backup/primary-workflow-before-split-2026-09-30`。
- 拆分只补充本记录的提交与验证说明，其余文件内容与原提交一致。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`：通过。
- `cargo check -p rssr-app --locked`：通过。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：通过。
- `cargo test --workspace --locked`：312 项通过，0 失败，2 项既有测试忽略。
- `cargo check -p rssr-app --target wasm32-unknown-unknown --locked`：通过。
- `dx build --package rssr-app --platform web --locked`：退出 0；实际产物用于本轮浏览器验收。仍有既存的 dx 0.7.10 / Dioxus 0.7.9 版本不匹配提示，没有升级工具或依赖。
- `cargo check -p rssr-app --target aarch64-linux-android --locked`：按现有 CI 方式指定已安装 NDK 27.3.13750724 / API 34 的 `CC_aarch64_linux_android`、`AR_aarch64_linux_android` 与 linker 后通过；没有安装新工具链。
- `node --check scripts/browser/rssr_small_viewport_assertions.mjs`、`git diff --check`：通过。
- 主题冻结文件逐字节比较、变更文档的本地链接和新增 selector 检查：通过。
- 提交拆分验证：逐文件对比 `0c046ba`，除本交接记录外无差异；各批次 `git diff --cached --check` 通过，提交后工作区干净。此次未重复运行上述构建与功能测试，因为被测源码、样式和回归脚本均未改变。

### 实际浏览器验收

- 既有 `run_static_web_small_viewport_smoke.sh`：默认及四套内置主题各 143 条断言通过，共 715 条，0 console error。覆盖 360×800 触屏媒体条件及既有 1280×800 场景。
- 复现命令：`bash scripts/run_static_web_small_viewport_smoke.sh --skip-build --port <port> --preset <key> --chrome-bin /home/deve/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome --log-dir target/primary-workflow-validation/<目录>`。默认主题省略 `--preset`。最终默认 / Amethyst / Atlas / Midnight / Newsprint 分别使用 8125 / 8126 / 8127 / 8128 / 8129。
- 新增行为回归：键盘实际进入未读文章；全部已读后两个方向均禁用且快捷键不换页；同订阅按钮仍可进入已读文章；折叠摘要包含全部选定条件；清除后重新加载仍保持清空结果及分组 / 归档偏好；收藏、标已读保持正在阅读的正文位置和 live region；模拟写入失败后错误仍可见且状态未误改。
- 底栏补充矩形相交检查，避免只检查按钮尺寸却漏掉点击区域覆盖。五主题均通过，Amethyst 在 360px 下四个按钮均为 63.5px 宽且互不覆盖。
- 首轮默认主题的 142 条功能断言通过，最后的 console 检查被旧 `reader-demo` 自动访问 example.com 的 CORS 错误拦下。测试仅在该导航用例期间拦截其自动刷新并返回 304，随后复验五主题全部通过；其他刷新和真实请求回归维持原入口。
- 单独运行 `node target/primary-workflow-validation/layout.cjs`：默认 / Amethyst 在 360 和 1280px 下元信息各行无重复留白、底栏不重叠；桌面单订阅页清除筛选后仍在原订阅且保留归档显示；无文章与无订阅的提示区分正确。
- 同一短文章对比：Amethyst 的 360px 正文起点由 y=485.70 提前到 y=350.70，改善 135px；1280px 由 y=544.31 提前到 y=399.31，改善 145px。保留了主题对整块元信息的原有留白。
- 五主题的收藏 / 标已读实际触控验收中，正文锚点与 scrollY 均保持不变；默认主题原先的 64px 位移归零。错误提示仍采用可见布局。
- 人工查看修复后的 Amethyst 窄屏 Reader、默认主题折叠筛选及桌面单订阅页截图，确认按钮文字、条件摘要和清除入口可读。

## 结果

- 四项优化均已实现并完成上述验证；前置审查独立提交为 `e052ee6`，实现、回归与文档已按上述四批本地提交。
- 本轮未 push、打 tag、发布或更新用户安装。已保存的旧主题 CSS 需要在设置页重新应用 Amethyst Glass 才会取得预设布局修正。
- 隔离浏览器与临时静态服务已退出；未修改用户实际订阅、正文、收藏或配置。

## 风险与后续事项

- 未执行 desktop / Android 原生 UI、Windows / macOS 实机和真实屏幕阅读器验收。Web DOM 与 Chromium 行为验证不能代替这些环境的实际交互。
- 自定义 CSS 和旧主题文本保持原样；仍可能覆盖基础呈现。没有将本轮结果扩展为任意用户 CSS 的兼容保证。
- 本次只处理已确认的日常路径，没有开展额外的存储、并发、极端内容或大数据优化。
- 拆分前查询远端分支并获取对象，确认远端 `main` 已在 `da3d104`，不包含 `0c046ba`；远端另有三个独立提交，本次未合并、变基或修改远端。

## 给下一位 Agent 的备注

- 实现入口：`reader_page/session.rs`、`reader_page/mod.rs`、`components/status_banner.rs`、`entries_page/facade.rs` / `controls.rs` / `reducer.rs` 和 `assets/themes/amethyst-glass.css`。
- 回归继续使用现有 `scripts/browser/rssr_small_viewport_assertions.mjs`；新增用例为 `checkPrimaryWorkflow`，并在 `checkReader` 检查底栏区域相交。
- `target/primary-workflow-validation/` 保存检查日志、五主题断言、布局测量及截图；它们是本地验证产物，不进入版本库。前置问题的原始测量位于 `target/important-ux-review/`。

# Amethyst Glass 小屏搜索宽度 CI 修复

- 日期：2026-09-23
- 作者 / Agent：Codex
- 分支：`main`
- 当前 HEAD（任务基线）：`5ec4d38d1a18ced994574afbbc9d3392e6b61bf7`
- 相关 commit：`commit: pending`（记录撰写时；提交号以本文件 Git 历史为准）
- 相关 tag / release：`v0.1.15` 不含先前 `2038eeb` 的搜索修复，也不含本次 CSS 调整；未打新 tag
- 状态：`validated`（本地 Web release 小视口 smoke；修复后的远端 CI 待重跑）

## 工作摘要

推送 README/文档与前一条产品修复后，主线 CI 的 `web-ui (amethyst-glass)` 唯一失败：360×800 Reader 搜索框仅 138 CSS px，低于既有 140px 可输入宽度断言。其余模块、wasm 契约、Android、Web 构建和其他主题完成任务通过；Docker workflow 成功。

## 影响范围

- 模块：`assets/themes/amethyst-glass.css` 的小屏导航内边距；本交接记录。
- 平台：共享 Dioxus Web / Desktop / Android 的 Amethyst Glass 主题呈现；无 Rust、业务或平台 adapter 变化。
- 额外影响：不改变命令、刷新、搜索状态或 CSS 默认主题。

## 关键变更

- 主题原导航水平内边距为 14px，在 360px Reader 中返回、R、搜索三个 44px 按钮后，只给输入框留下 138px。
- 仅在视口不大于 480px 时将该主题导航水平内边距设为 8px。没有放宽触控按钮尺寸，也没有让搜索跨行导致 Reader 正文整体下跳。
- 曾试验扩大共享窄容器断点，输入框跨行可读，但使该主题搜索前后 shell 高度从 66px 变为 116px；已撤回，最终只修改主题小屏内边距。

## 验证与验收

### 自动化验证

- [首次远端 CI](https://github.com/Develata/RSS-Reader/actions/runs/35853732969)：`web-ui (amethyst-glass)` 失败，日志证据为搜索宽 138px；最终汇总失败。其他已完成 job 成功。
- `bash scripts/run_static_web_small_viewport_smoke.sh --release --preset amethyst-glass --chrome-bin /home/deve/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome --port 8093 --log-dir target/amethyst-search-ci-fix-final-20260923`：通过；重新构建 Web release，360×800 DPR3 及现有桌面路径共 128 项断言通过，无 console error。Reader 搜索框 150×44 CSS px、可点击并聚焦，搜索前后 shell 高度同为 66px；目录下保留未提交的截图和断言 JSON。
- `git diff --check`、`cargo fmt --all --check`：通过。
- [本次 push 后 Docker workflow](https://github.com/Develata/RSS-Reader/actions/runs/35853732950)：成功；这是文档提交 `5ec4d38` 的镜像构建，不含本次 CSS 修复。

### 手工验收

- 查看小屏 `reader-search.png`：输入框与返回、R、搜索按钮同排，输入占用可见横向空间，无横向溢出或下方正文位移。
- 首次本机尝试误用 Windows Chrome 作为 WSL 浏览器；其 WSL UNC profile 下 CDP 端口未就绪。最终改用本机 Linux Playwright Chromium，未把该环境失败算作产品回归。
- Android/macOS 设备：未运行，当前仅有浏览器呈现证据。

## 结果

- CSS 修复本地目标 smoke 通过；需推送后由同一 CI matrix 在 Linux Chrome for Testing 环境复验。

## 风险与后续事项

- 150px 为 360px Reader + 三个 44px 按钮条件下的可用宽度；更窄视口或第三方自定义主题仍依赖共享窄容器规则。现有 360px smoke 不能代替所有自定义主题的人工验收。
- 不处理此前文档审计披露的 Linux `.deb` 数据目录权限问题；那需要独立的数据迁移设计。

## 给下一位 Agent 的备注

- CI 证据在 `web-ui (amethyst-glass)` job 的“Run fixed small viewport smoke”步骤；本机目标产物在忽略的 `target/amethyst-search-ci-fix-final-20260923/`。
- 推送后确认最终 `lint-and-test` 汇总，不要仅凭本机 Chromium 或单个 job 宣称整条 CI 成功。

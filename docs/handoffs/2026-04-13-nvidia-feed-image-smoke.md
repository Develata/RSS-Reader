# NVIDIA Feed 图片加载 Smoke

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：fbb8265
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

按 release matrix 手工项补测 `https://blogs.nvidia.com/feed/` 在 `rssr-web` 实页部署壳中的首刷与正文图片加载状态，重点确认 NVIDIA 正文图片和 GIF 是否成功加载。

## 影响范围

- 模块：
  - `rssr-web` 手工浏览器 smoke
  - `rssr-app` Web 阅读页渲染路径
  - `target/rssr-web-browser-smoke/20260413-codex-nvidia-feed-images/`
- 平台：
  - Web
  - Linux
- 额外影响：
  - release manual smoke 记录

## 关键变更

### 手工 Smoke 产物

- 新增本次验证记录：`target/rssr-web-browser-smoke/20260413-codex-nvidia-feed-images/summary.md`。
- 新增截图产物：`target/release-ui-regression/20260413-codex-post-cleanup/manual-rssr-web-nvidia-feed-image-loaded.png`。

### 验证观察

- `https://blogs.nvidia.com/feed/` 添加成功，页面显示 `NVIDIA Blog`，`订阅数 1`，`文章数 18`。
- 首篇文章 `National Robotics Week — Latest Physical AI Research, Breakthroughs and Resources` 可进入阅读页。
- 阅读页 DOM 中 `img` 总数 19，`complete=true && naturalWidth > 0` 为 19，破图数 0。
- Network 面板显示 `/feed-proxy?url=https%3A%2F%2Fblogs.nvidia.com%2Ffeed%2F` 返回 200。
- Network 面板显示 NVIDIA 内容图/GIF 请求 7 个，均返回 200。

## 验证与验收

### 自动化验证

- `bash scripts/run_rssr_web_browser_smoke.sh --skip-build --port 18092 --log-dir target/rssr-web-browser-smoke/20260413-codex-nvidia-feed-images --feed-url https://blogs.nvidia.com/feed/`：通过，需在 sandbox 外运行；sandbox 内绑定监听地址失败。

### 手工验收

- `/login` 使用 `smoke / smoke-pass-123` 登录：通过。
- `/feeds` 添加 `https://blogs.nvidia.com/feed/`：通过。
- `/feeds/1/entries` 文章列表：通过。
- `/entries/1` 阅读页：通过。
- 正文图片加载：通过，`19/19` 图片加载完成，破图数 `0`。

## 结果

- 本次 NVIDIA feed 图片加载 smoke 通过，可作为 release matrix 中真实远端 feed 图片加载的补充证据。
- 图片目前直接从 `https://blogs.nvidia.com/wp-content/...` 加载，浏览器网络请求返回 200。

## 风险与后续事项

- `scripts/run_rssr_web_browser_smoke.sh` 的 summary heredoc 仍有反引号命令替换问题，会把模板中的 `/entries`、`/reader` 等文本吞掉；不影响本次 smoke，但应后续修复。
- 复用 debug Web bundle 时 console 存在 `/_dioxus` WebSocket 404 噪声；本次判断为非功能阻塞。
- 当前 worktree 仍有未跟踪 `.codex`，本次未改动。

## 给下一位 Agent 的备注

- 本轮验证入口是 `target/rssr-web-browser-smoke/20260413-codex-nvidia-feed-images/summary.md`。
- 如需复现，先确保 `target/dx/rssr-app/debug/web/public` 已存在，再用相同脚本参数启动 `rssr-web`。

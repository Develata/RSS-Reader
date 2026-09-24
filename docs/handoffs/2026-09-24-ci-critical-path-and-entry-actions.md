# CI 关键路径、wasm Clippy 缺口与文章列表动作层级

- 日期：2026-09-24
- 作者 / Agent：Claude Code (Opus)
- 分支：main
- 当前 HEAD（工作开始时）：543b03f
- 相关 commit：4d97613（CI 清理 / Gradle 缓存）、1989ce6（刷新串行路径 + wasm Clippy）、40fcff9（文章列表动作与筛选开关）；本文与文档随后提交
- 相关 tag / release：N/A（已按 Develata 授权 push main，未 tag）
- 状态：`validated`（本地可执行范围；CI 耗时收益待远端运行确认）

## 工作摘要

按"全局检查与优化"要求，先用最近一次 CI（run 35994270439）的逐步耗时定位关键路径，再审查热路径与默认界面。只改有实测依据的地方：CI 两处浪费、一处 wasm 专属 lint 缺口（已存在的 clippy 错误）、文章列表的视觉层级。

## 影响范围

- 模块：`.github/actions/cleanup-github-runner`、`.github/workflows/ci.yml`、`rssr-application::refresh_service`、`assets/styles/{entries,responsive}.css`、`rssr-app` entries 页 controls。
- 平台：CI；Web / 桌面 / Android 共用的页面样式；application 刷新串行路径（wasm 与原生 `max_concurrency == 1`）。
- 文档：主线验收矩阵、主题作者 selector 参考。

## 关键变更

### CI

- 实测：ubuntu-latest 根分区清理前已有 86 GiB 空闲，`Cleanup runner` 却耗时 7–105 s（rssr-app 一次 88 s，落在其 302 s 关键路径上）。清理改为空闲低于 `min-free-gb`（默认 40）才执行；release/docker 共用同一 action，同样受益。
- Android smoke：dx 内部 Gradle（"Bundling app" 118 s + "package bundler" 55 s）每次重新下载 wrapper 与 AGP 依赖。新增 `~/.gradle` 缓存，键为 dx 版本 + `Dioxus.toml` / `android/**` / `prepare_android_bundle.py`。
- `web-smoke` 的 wasm `cargo check` 换成 wasm `cargo clippy -D warnings`：原生模块 job 的 Clippy 看不到 `cfg(target_arch = "wasm32")` 分支。

### 刷新服务

- `refresh_targets` 的 wasm 分支以显式 `return` 结尾，在 wasm 目标下触发 `clippy::needless_return`（此前一直未被 CI 发现）。合并为单一串行路径：原生 `max_concurrency > 1` 走并发，其余（wasm、并发度 1）走同一循环。语义不变。

### 文章列表 UI

- 每篇文章的「标已读 / 收藏」原为实色强调块，移动端还各自撑满整行，视觉权重高于标题。改为内容宽度的描边胶囊，只用各主题都定义的表面 token；选择器特异性高于主题的 `.button[data-variant="secondary"]`（内置主题给次级按钮配深底浅字，第一次实现只继承其字色，导致 newsprint 等主题文字几乎不可见——已在本地截图中发现并修正）。
- 订阅卡片动作在移动端保持整行堆叠。
- 收起状态的筛选开关只有箭头，补可见文字「筛选与组织」/「收起筛选」；可访问名称仍包含可见文字。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`：通过。
- `cargo clippy --locked -p rssr-app --target wasm32-unknown-unknown -- -D warnings`：修复前失败（needless_return），修复后通过。
- `cargo clippy --locked -p rssr-application --all-targets -- -D warnings`、`cargo test --locked -p rssr-application`（52 passed）：通过。
- `actionlint`：通过；清理阈值判断逻辑用本机 df 在阈值 1 / 100000 两侧手动验证。
- `dx bundle --platform web --release` 后 `scripts/run_static_web_small_viewport_smoke.sh --release --skip-build`，default + 4 个内置主题：各 128 项断言通过，控制台错误 0。
- 截图人工核对 5 个主题的文章动作可读、移动端并排。

### 未执行或环境受限

- 本机缺 GTK/WebKit 开发库，未跑原生 `rssr-app` 构建/测试；缺 Android NDK 环境变量，未跑 Android check。两者由 CI 覆盖。
- 收起状态筛选开关的可见文字未截图核对（smoke 未截该状态），仅断言通过。
- CI 耗时收益（清理跳过、Gradle 缓存命中）需要 push 后的远端运行确认；Gradle 缓存首跑为 miss。
- smoke 断言没有对比度检查，本次主题可读性问题靠人工截图发现。

## 结果

- 三个本地 commit，未 push。预计 rssr-app 模块路径缩短约一个清理步骤（实测 7–105 s 波动），Android smoke 在缓存命中后减少 Gradle 下载时间；具体数值待远端验证。

## 风险与后续事项

- 导航：经 Develata 确认，S 改为 RSS 波纹 SVG、⚙ 改为滑杆 SVG（与搜索图标同一描边体系，避免字母需记忆、emoji 跨平台字体不一致），R 保留为品牌与首页。README / README.en / 使用指南同步更新；`assets/readme/rss-reader-overview.png` 仍是旧图标，待下次截图更新。该组合此前随第一次 Web 包跑过 default smoke（128 项通过）。
- 未采纳：`upsert_entries_and_resolve_contents` 对正文字符串的 clone。量级约为每订阅每次刷新 1 MB 以内，远小于网络与 SQLite 成本，不值得改公共签名。
- 可考虑给 small viewport smoke 增加动作按钮文字 / 背景对比度断言，防止主题覆盖再次造成不可读。

## 给下一位 Agent 的备注

- CI 耗时分析方法：`gh api repos/Develata/RSS-Reader/actions/runs/<id>/jobs` 取各 step 的 started/completed 时间。
- 本地跑 UI smoke：先 `dx bundle` 再 `--skip-build`，每个预设用不同 `--port`，否则冷构建会超过就绪等待、连续运行会撞端口。

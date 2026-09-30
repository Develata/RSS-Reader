# UI 焦点、语义与异步反馈完善

- 日期：2026-09-30
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：`379ac22`（前置文档刷新提交）
- 相关 commit：本记录所在的 UI 提交；前置文档提交为 `379ac22`
- 相关 tag / release：N/A（没有 push、tag 或发布）
- 状态：`validated`

## 工作摘要

先完成并独立提交仓库文档刷新，再实施用户确认的六项小范围 UI 改进。保留既有命令、刷新生命周期、阅读快捷键作用域与布局结构；改进禁用反馈、标题层级、Reader 可访问语义、动态状态通知及 checkbox 配色。

## 影响范围

- 模块：`rssr-app` 页面、`StatusBanner`、基础 CSS、Amethyst Glass / Midnight Ledger、旧预设识别、浏览器回归与当前 UI 文档。
- 平台：共享 UI 影响 Web、桌面和 Android；本次实际交互验收使用 Chromium，另完成 Linux 原生目标与 Android ARM64 编译检查。
- 额外影响：无 application/domain/infra 行为、数据库、配置格式、依赖或 workflow 修改。

## 关键变更

### 呈现与可访问性

- `.button:disabled` 统一透明度、光标与阴影；基础及内置主题 hover/active 排除禁用按钮。原生 checkbox 使用主题 `--accent`。
- 页面主标题改为 `h1`，应用小节按层级递进。`card-title` 显式保留原字号，避免 h3 → h2 触发浏览器默认字号放大；来源正文 HTML 不改写。
- Reader 保留局部 `tabindex=0` 入口，增加具名 region、关联快捷键说明和主题焦点环；收藏发布 `aria-pressed`，底栏装饰字符不进入可访问名称。
- Amethyst 浅色与 Midnight 焦点色使用 `--accent-strong`，修复实际渲染中对比度偏低的半透明焦点环。

### 异步与状态

- 订阅页“刷新全部”从既有 shell 状态读取忙态，同步文案、`disabled` 和 `aria-busy`；没有新增页面局部刷新状态或重复请求路径。
- 动态 `StatusBanner` 持续挂载，使用 polite / atomic status live region。空消息采用 `sr-only`，更新时不抢焦点；静态空列表和归档说明不播报。覆盖文章、订阅、设置、Reader 与本地 Web 登录错误。
- 删除为条件卸载消息组件服务的 `has_status_message()` facade 方法，同步当前架构说明。

### 主题兼容与文档

- 按现有 `LEGACY_PRESET_CSS` 机制冻结本次修改前的 Amethyst / Midnight CSS，保留旧预设名称和移除行为；不覆盖已保存的 CSS。重新应用内置预设后才取得新版主题样式。
- 前置文档阶段已补默认 primary、断点职责和对比度契约；本阶段记录实际状态/语义接口、空 live region 样式边界、主题快照行为与新增回归覆盖。
- 原有小屏 smoke 新增 5 条结果断言：状态节点持续挂载、异步反馈不抢焦点、Reader 语义、跨入口刷新忙态和结束后恢复。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`：通过。
- `cargo clippy --locked -p rssr-app --all-targets -- -D warnings`：通过。
- `cargo test --locked -p rssr-app`：108 单元测试和 4 集成测试通过，1 个既有性能测试忽略。首轮受沙箱本机端口限制，获准后重跑通过；最后主题兼容表变更再定向运行 5 个预设测试，通过。
- `cargo check --locked -p rssr-app --target wasm32-unknown-unknown`：通过；后续最终 `dx build --package rssr-app --platform web --locked` 也成功，退出 0。
- `cargo check --locked -p rssr-app --target aarch64-linux-android`：首次未配置 C compiler 而失败；按现有 CI 方式指定本机已安装的 NDK 27.3.13750724 / API 34 后通过。没有安装或修改全局工具链。
- Web 构建仍报告已存在的 dx 0.7.10 / Dioxus 0.7.9 版本不匹配提示；未因此改动项目依赖。最终产物已用于真实浏览器验收。
- `node --check scripts/browser/rssr_small_viewport_assertions.mjs`、`git diff --check`、文档本地链接/锚点与公开 selector 检查：通过。
- 两份新增 legacy CSS 与 `379ac22` 中原始文件逐字节一致。

### 实际浏览器验收

- Chromium 151，独立 profile 与 fixture，不读取用户浏览器数据。最终五套主题小屏 smoke 全部通过，每套 133 条，共 665 条断言，0 未处理 console error。
- 复现命令：`bash scripts/run_static_web_small_viewport_smoke.sh --skip-build --port <port> --preset <preset> --chrome-bin /home/deve/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome --log-dir target/ui-accessibility-validation/final-smoke-<preset>`。默认主题省略 `--preset`；默认 / Atlas / Newsprint 分别完成于 8117，Amethyst / Midnight 最后使用 8118 / 8119。连续复用 8117 时曾被端口检查拦截一次，尚未进入页面断言；后续确认没有遗留监听，以独立端口完成复验。
- 五套主题 × 360 / 1280px × 文章/订阅/设置/Reader，共 40 个页面场景通过；对照基线的 240 个同名标题尺寸无变化，0 横向溢出。
- 五套主题的 light、dark、system-light、system-dark 共 20 种组合检查实际 Tab 路径、Reader 快捷键、修饰键/IME 放行、搜索输入隔离、checkbox Space 操作和禁用 hover/active；两套焦点色修正后再复验相应 8 种组合，通过。
- 本地登录表单空用户名失败时更新原有 status 节点，不抢焦点；Reader 收藏的实际 F 操作更新 `aria-pressed`。
- 截图实际合成色取样：Amethyst 浅色 Reader 焦点由约 2.08:1 提高到 5.37:1，Midnight 由约 2.21:1 提高到 8.51:1。20 组合中 Reader 焦点取样最小 3.36:1，来源文字取样最小 4.70:1；这些点位结果不代表全站 WCAG 认证。
- 人工查看代表性亮暗/窄屏截图，核对卡片标题字号、焦点可见性与既有视觉层级。

## 结果

- 文档先行要求已由独立提交 `379ac22` 保证；UI 实现、兼容与针对性验证完成，随本记录本地提交。
- 没有 push、发布或更新用户实际安装。

## 风险与后续事项

- 未运行 Android、Windows/macOS 原生交互及真实屏幕阅读器；本轮也未重做 Linux 原生窗口交互。Web DOM / 色彩验收不能代替这些平台的实际体验。
- 已保存的旧预设和任意自定义 CSS 保留原文，可能仍覆盖基础反馈；重新应用新版内置主题才能取得本次预设规则。
- 不宣称全站或用户自定义主题完整满足 WCAG；取样仅覆盖列明的 Reader 焦点和来源文字。

## 给下一位 Agent 的备注

- 运行产物在忽略目录 `target/ui-accessibility-validation/`：构建/测试日志、`baseline/` / `final/` 截图、`heading-comparison.json`、`modes/` / `modes-final/`、`contrast-before.json` / `contrast-final.json` 和各主题 smoke。临时测量脚本未作为生产或 CI 工具提交。
- 持续回归从 `scripts/browser/rssr_small_viewport_assertions.mjs` 进入；当前语义见 `docs/design/frontend-command-reference.md` 与主题参考。
- 只使用本轮隔离服务和测试数据，浏览器与静态测试服务已退出；验证产物保留在忽略目录，没有修改用户数据。

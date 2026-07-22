# 内置主题优化：暗色变体、性能加固与 Amethyst Glass 更名

- 日期：2026-07-22
- 作者 / Agent：Claude Code (math-architect)
- 分支：main
- 当前 HEAD：9405f26
- 相关 commit：9405f26（feat，主题整体优化 + 更名 + legacy 识别）
- 相关 tag / release：N/A
- 状态：`validated`（自动化验证通过；视觉效果待用户实测确认）

## 工作摘要

按用户确认的四条优化轨道，对 Atlas Sidebar / Newsprint / Amethyst Glass 三套内置主题做整体优化：

1. **性能与健壮性**（不改视觉身份）：Amethyst 视口渐变从 `background-attachment: fixed`
   改为 `body::before` 固定合成层（滚动零重绘）；整页 `backdrop-filter` 从 22px 降到
   12px、导航从 18px 降到 12px（面板透明度 0.45→0.5 补偿视感）；补
   `prefers-reduced-motion` 回退。Atlas 侧栏两套重复规则合并声明；`grid-column: 2`
   枚举列表改为 `> :not([data-layout="app-nav-shell"])` 单条规则（新增页面区块自动落位）；
   修正 body 底纹 `background-size` 层数不匹配（第三层渐变被错误平铺成 28px 条带）。
2. **暗色变体**：三套主题各补 `.theme-dark` + `@media (prefers-color-scheme: dark)`
   下 `.theme-system` 的完整 token 覆写块。此前它们只覆写 `:root`（亮层），暗色模式下
   会退化成默认暗色调 + 残留亮色结构规则（Atlas 暗色下出现发亮米色侧栏）。
   随模式翻转的结构色提炼为主题局部变量（`--atlas-rail-*`、`--glass-*`），结构规则本身
   模式无关。body 在 `.app-shell` 外拿不到主题类变量，暗色底纹用 `body:has(.theme-dark)`
   （及 `.theme-system` 媒体查询变体）显式声明——与 tokens.css 既有做法一致。
3. **Newsprint 排版深化**：阅读正文改衬线（`var(--font-display)`）、两端对齐 +
   `hyphens: auto`，段落加 `text-wrap: pretty`（渐进增强）；删除对 CJK 导航无效的
   `text-transform: uppercase` 死规则。
4. **更名**：`forest-desk` → `amethyst-glass`（文件、preset key、UI、脚本、文档），
   与 UI 显示名「Amethyst Glass」对齐。

**旧版识别兼容**：设置只持久化 CSS 文本，`detect_preset_key` 逐字节比对；改版会让
已应用旧版主题的用户退化显示「自定义主题」。为此冻结旧版副本到
`assets/themes/legacy/*-v1.css`，`detect_preset_key` 追加旧文本 → 现行 key 的别名匹配
（含 forest-desk-v1 → amethyst-glass）。冻结副本必须保持逐字节不变，因此**刻意不纳入**
`test_builtin_theme_contracts`（未来 contract 收紧不能追溯冻结文件）。

## 影响范围

- 模块：
  - `assets/themes/`：`atlas-sidebar.css`、`newsprint.css` 重写；`forest-desk.css` →
    `amethyst-glass.css`；新增 `legacy/{atlas-sidebar,newsprint,forest-desk}-v1.css`
  - `crates/rssr-app/src/pages/settings_page/themes/{theme_preset.rs, presets.rs}`
  - `crates/rssr-app/tests/test_builtin_theme_contracts.rs`
  - `scripts/{run_static_web_reader_theme_matrix.sh, run_web_spa_regression_server.sh,
    browser/rssr_visible_regression.mjs}`
  - `docs/design/{theme-author-selector-reference.md, web-spa-regression-server.md}`
    （选择器参考新增暗色变体 / body:has / 主题局部变量 / 性能约定四条覆写建议）
- 平台：Windows / macOS / Linux / Android / Web（主题 CSS 全端共用；合成层与模糊
  减负对 WebView2 与 Android WebView 收益最大）
- 额外影响：历史 handoff 中的 forest-desk 引用按惯例不改（历史记录）。

## 关键变更

- 级联依据：自定义 CSS 注入在基础样式之后（`app.rs:47-49` 先 `APP_STYLESHEET` 后
  `#user-custom-css`），同优先级声明主题后到先赢；`.theme-dark` 在 `.app-shell` 上，
  壳内元素的 token 由它压过 `:root`，故暗色块必须覆写主题亮层设置过、且基础暗色
  也设置的每个 token（radius / 字体 / `--shell-max-width` 等基础暗色不碰的可省略）。
- 强调态（`--accent-line` 等）：Atlas / Newsprint 沿用 color-mix 自动派生（tokens.css
  `@supports` 块引用 `var(--accent)`，主题只改 accent 即整体跟随）；Amethyst 与其亮层
  风格一致地显式声明。
- `detect_preset_key` 从 if-else 链改为现行 key 循环 + legacy 循环，新增单测：
  legacy 文本映射到现行 key、冻结副本与现行 CSS 确实不同（防冻结形同虚设）。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`：通过
- `cargo clippy --workspace --all-targets -- -D warnings`：通过
- `cargo test -p rssr-app`：通过（含 4 个 theme_preset 单测与 2 个主题 contract 测试）
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo check -p rssr-app --target aarch64-linux-android`：未执行（无移动端交互语义变化）

### 手工验收（待用户）

- 桌面端逐一应用三套主题，在主题模式 亮色 / 暗色 / 跟随系统 下检查四页视觉：
  重点 Atlas 暗色侧栏、Newsprint 暗色纸面 + 衬线正文、Amethyst 暗色玻璃与滚动流畅度。
- 曾应用旧版主题的既有配置载入后，设置页应仍识别为对应预设（非「自定义主题」）。
- 建议跑 `bash scripts/run_static_web_reader_theme_matrix.sh` 复核 Web 端主题矩阵。

## 结果

- 已作为单个 feat commit 提交至 main（9405f26），未推送。
- 用户可见影响：三套主题获得完整暗色模式；Amethyst 滚动性能与可及性改善；
  Newsprint 正文更具报纸质感；主题下拉与卡片的 key 显示为 amethyst-glass。

## 风险与后续事项

- `body:has(...)` 需 Chromium 105+ / Safari 15.4+：WebView2 与现代浏览器均满足；
  不支持时退化为「视口底色保持亮色、壳内正常变暗」，与 tokens.css 既有退化一致。
- 暗色 token 块在 `.theme-dark` 与 `.theme-system` 媒体变体间成对重复是 CSS 无 mixin
  的固有代价（基础 tokens.css 同样如此）；两处必须同步修改。
- 主题改版流程沉淀：再次改版内置主题时，先把现行文件冻结为 `legacy/<key>-vN.css`
  并在 `LEGACY_PRESET_CSS` 追加一条，再改现行文件。
- Amethyst 亮色 `--panel` 0.45→0.5 的补偿是估值，若用户觉得«雾感»不足/过重可微调。

## 给下一位 Agent 的备注

- 入口：`theme_preset.rs`（key / 检测 / legacy 机制）、`assets/themes/`（现行 + 冻结）。
- 冻结副本逐字节不可改；contract 测试只覆盖现行四个主题文件。
- 主题作者规范（含暗色 / 性能约定）见 `docs/design/theme-author-selector-reference.md`
  「覆写建议」。

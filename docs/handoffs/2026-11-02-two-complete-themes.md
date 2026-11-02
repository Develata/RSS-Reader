# 内置主题收敛为两套完整主题

- 日期：2026-11-02
- 作者 / Agent：Claude Code（Opus 5.5）
- 分支：claude/css-theme-styles-41840a
- 当前 HEAD：见 PR 合并记录
- 相关 commit：e56d5900（主题收敛）、本 handoff 所在的复审修复提交；已同步 main 287256da
- 相关 tag / release：N/A
- 状态：`draft`

## 工作摘要

设置里的浅色 / 深色对 Midnight Ledger 不起作用（它把三种模式写成同一份暗色），另外三个预设的暗色只覆盖部分 token，状态色混入默认配色。按 Develata 的方向收敛为「一个主题 = 一份完整 CSS」：默认主题 = 旧默认浅色 + Midnight Ledger 配色的深色；Amethyst Glass 补齐为浅暗两套完整主题；模式选择改名为「外观」，用于白天浅色、夜间深色。

## 影响范围

- 模块：
  - `assets/styles/tokens.css`、`assets/themes/`
  - `crates/rssr-app/src/pages/settings_page/{preferences.rs,themes/}`
  - `crates/rssr-app/tests/test_builtin_theme_contracts.rs`
  - `scripts/browser/*`、`scripts/run_server_web_regression.sh`、`scripts/release-ui/main.rs`、`.github/workflows/ci.yml`
- 平台：Windows / macOS / Linux / Android / Web（共享样式）
- 额外影响：用户文档与测试文档；CI web-ui 矩阵从 5 个预设缩为 `default, amethyst-glass`

## 关键变更

### 默认主题

- `.theme-dark` 与跟随系统暗色块改为 Midnight Ledger 的墨青配色（47 个随模式翻转的 token 全覆盖，结构 token 如圆角、阅读宽度保持与浅色一致）；`:root --app-bg-dark` 改为 Midnight 的视口渐变。
- 暗色次按钮为中性石板色：`color-mix` 派生的 `--button-secondary-bg` 只留在 `:root`，其余强调态派生不变。
- Midnight 原有的少量非 token 精修（导航描边、标题字距、按钮文字阴影等）未搬入，保持 tokens.css「只覆写 token」的约定。

### 预设

- 现存内置预设只有 Amethyst Glass（去掉「实验」标记），补齐浅暗两套的 `--danger`、`--status-*`、`--button-secondary-fg`，浅色补 `--inset-highlight`、`--app-bg`。改版前文本冻结为 `legacy/amethyst-glass-v4.css`。
- Atlas Sidebar、Newsprint、Midnight Ledger 下线，现行文件移入 `legacy/`（`atlas-sidebar-v4`、`newsprint-v2`、`midnight-ledger-v3`）。已保存的这些 CSS 仍原样生效，`detect_preset_key` 识别为 `retired`，设置页提示「已下线的内置主题，外观设置可能不起作用」。不自动改写用户设置。

### 设置页

- 模式选择的标签由「主题」改为「外观」；默认主题卡片说明改为「暖纸浅色与墨青深色，随上方外观切换」。

### 契约测试

- `test_builtin_theme_contracts`：默认主题的 `:root` 与跟随系统暗色必须覆盖 `.theme-dark` 的全部 token；每个内置主题的 `:root`、`.theme-dark` 也必须全覆盖，且跟随系统暗色块与 `.theme-dark` 声明集合一致。解析器先去掉注释，跟随系统暗色块与 `.theme-dark` 按名称→值比较。变异验证：删去或注释掉 Amethyst 暗色 `--danger`、只改跟随系统暗色的 `--danger` 值，测试都会失败。
- `test_default_style_token_contract`：表面背景 token（`--surface-panel/reader/card/veil/chip(-hover)`）不得出现在渐变参数里；把 `workspaces.css` 改回嵌套写法后测试失败。

### Codex 复审（gpt-6-astra，high，范围 f6c4a0cd..d807d561）

- 结论：无 BLOCKER；2 项 SHOULD_FIX 均已核实并修复，1 项 NEEDS_VERIFICATION 记为未覆盖。
- SHOULD_FIX 1（已修复）：新默认深色的 `--surface-card`、`--surface-veil` 是渐变，而 `workspaces.css`（添加订阅卡片、高级 CSS 面板）与 `shell.css`（订阅统计卡片）把它们当渐变色标，整条 `background` 失效。文档本就把这些 token 定义为背景，因此改的是消费方：改为「色调渐变叠在表面背景之上」的分层写法，`entries.css` 的 `--surface-chip` 同样处理；主题作者文档注明 `--surface-tint` 必须是颜色。
- SHOULD_FIX 2（已修复）：覆盖测试会把注释掉的声明算作覆盖，且只比较名称。已按上面的契约测试修正。
- NEEDS_VERIFICATION 3（未覆盖）：浏览器主题矩阵只循环两个主题，没有浅色 / 深色 / 跟随系统 × 主题的计算样式断言。

### Codex 复审第二轮（同模型，范围 08cf5716）

- 结论：无 BLOCKER；2 项 SHOULD_FIX 均为测试守卫可被合法 CSS 绕过，已核实并修复；1 项 NEEDS_VERIFICATION 记为未覆盖。
- 守卫只匹配完整的 `var(--surface-card)`，带 fallback、内部空白或注释里的 `)` 会漏检。现改用共享的 `tests/css_scan`（识别注释、字符串、括号），按 `var()` 第一个参数判断，四种写法均有反例测试。
- 解析器删除注释会把 `--dan/**/ger` 拼成 `--danger`，按 `;` 直接切分会截断 `url("data:…;…")`。现注释替换为空格、只在顶层分号切分，形似自定义属性但不是单个标识符的名称直接失败；均有反例测试。
- 视觉差异（已接受，未在应用中复看）：分层写法让色调端叠在表面之上，不再是两者插值。默认浅色各通道差约 3/255；Amethyst 浅色下添加订阅卡片底部、订阅统计卡片顶部更白（不透明度从约 0.4 升到约 0.8）。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`：通过
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：通过
- `cargo test --workspace --locked`：通过（第二轮修复后 1200 通过、0 失败）
- `cargo check -p rssr-app --target wasm32-unknown-unknown --locked`：通过
- `cargo check --locked`（scripts/release-ui）：通过
- `node --check` 四个改动的浏览器脚本、`bash -n scripts/run_server_web_regression.sh`：通过

### 手工验收

- 桌面端（Linux，`cargo run -p rssr-app`，复审修复前的 e56d5900）：Develata 目测两套主题观感可接受；复审修复后的订阅页卡片背景未再目测
- 同一次运行中滚轮失效：本分支基于的 main 尚未包含 PR #61（Linux 滚轮与滚动卡顿修复，仍为 open），与本次改动无关
- 服务器 Web 回归（`run_server_web_regression.sh`、acceptance）：未执行，脚本只做了语法检查

## 结果

- 复审项已处理，Develata 已授权在复审无问题后开 PR 并合并。
- 用户影响：正在使用 Midnight Ledger 的用户（包括 Develata 本机）需在设置里点「恢复默认」并保存一次，之后由「外观」切换昼夜。

## 风险与后续事项

- Linux WebKitGTK 的 `<select>` 弹出列表由 GTK 绘制，不跟随页面暗色，本次未处理。
- `rssr-web` 登录页（`crates/rssr-web/src/auth/login_page.rs`）的暗色仍是旧的暖炭配色，与新默认深色不一致，可后续对齐。
- Amethyst 的 `backdrop-filter` 在 WebKitGTK 上的滚动性能未复测。

## 给下一位 Agent 的备注

- 入口：`assets/styles/tokens.css` 暗色块、`assets/themes/amethyst-glass.css`、`themes/theme_preset.rs`（`LEGACY_PRESET_CSS` / `RETIRED_PRESET_CSS`）。
- 新增内置主题必须同时通过 `test_builtin_theme_contracts` 的浅暗覆盖检查。

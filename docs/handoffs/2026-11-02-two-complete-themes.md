# 内置主题收敛为两套完整主题

- 日期：2026-11-02
- 作者 / Agent：Claude Code（Opus 5.5）
- 分支：claude/css-theme-styles-41840a
- 当前 HEAD：f6c4a0cd（改动前）
- 相关 commit：见本分支下一条提交
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

- `test_builtin_theme_contracts`：默认主题的 `:root` 与跟随系统暗色必须覆盖 `.theme-dark` 的全部 token；每个内置主题的 `:root`、`.theme-dark` 也必须全覆盖，且跟随系统暗色块与 `.theme-dark` 声明集合一致。已做变异验证：删去 Amethyst 暗色 `--danger` 后测试失败并指出缺失项。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`：通过
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：通过
- `cargo test --workspace --locked`：通过（0 失败；rssr-app 单元 409 通过 1 忽略）
- `cargo check -p rssr-app --target wasm32-unknown-unknown --locked`：通过
- `cargo check --locked`（scripts/release-ui）：通过
- `node --check` 四个改动的浏览器脚本、`bash -n scripts/run_server_web_regression.sh`：通过

### 手工验收

- 桌面端浅色 / 深色 / 跟随系统 × 默认 / Amethyst 的实际观感：未执行（本机未装 dioxus-cli，未跑浏览器回归）
- 服务器 Web 回归（`run_server_web_regression.sh`、acceptance）：未执行，脚本只做了语法检查

## 结果

- 代码与文档已就绪，需视觉验收后再合并。
- 用户影响：正在使用 Midnight Ledger 的用户（包括 Develata 本机）需在设置里点「恢复默认」并保存一次，之后由「外观」切换昼夜。

## 风险与后续事项

- Linux WebKitGTK 的 `<select>` 弹出列表由 GTK 绘制，不跟随页面暗色，本次未处理。
- `rssr-web` 登录页（`crates/rssr-web/src/auth/login_page.rs`）的暗色仍是旧的暖炭配色，与新默认深色不一致，可后续对齐。
- Amethyst 的 `backdrop-filter` 在 WebKitGTK 上的滚动性能未复测。

## 给下一位 Agent 的备注

- 入口：`assets/styles/tokens.css` 暗色块、`assets/themes/amethyst-glass.css`、`themes/theme_preset.rs`（`LEGACY_PRESET_CSS` / `RETIRED_PRESET_CSS`）。
- 新增内置主题必须同时通过 `test_builtin_theme_contracts` 的浅暗覆盖检查。

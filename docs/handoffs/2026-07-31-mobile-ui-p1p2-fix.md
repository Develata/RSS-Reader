# 移动端 UI P1/P2 修复：data-nav 作用域、触控目标、信息组织、平台健壮性

- 日期：2026-07-31
- 作者 / Agent：WorkBuddy (Kimi-K3)
- 分支：main
- 当前 HEAD：fd8615b
- 相关 commit：fd8615b（fix）；docs commit 见后续 log
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

按 `docs/mobile-ui-audit-2026-07.md` 的 P1/P2 批次完成修复：5 个 slice（data-nav 作用域收敛 + 触控目标、触屏键盘提示、概览卡分级 + 订阅列表上移、non-wasm 持久化 + safe-area、Amethyst Glass 治理），15 文件 +137/-80，全部经 360×800 实测验收。

## 影响范围

- 模块：
  - 样式：`assets/styles/{shell,responsive,reader,entries}.css`
  - 页面：`entries_page`（controls/mod/session/browser_interactions）、`feeds_page/mod.rs`、`reader_page`（facade/mod）、`settings_page/themes`（presets/theme_preset）
  - 偏好持久化：`ui/shell_prefs.rs`、`ui/mod.rs`（re-export）
- 平台：
  - Android / 移动 Web 为主修复面；桌面端同步变化：订阅标题去胶囊化、feed 按钮间距、概览卡分级、控件收起状态开始持久化
- 额外影响：
  - Web 端 `rssr-entry-controls-hidden` localStorage 键保留（老用户值不丢）；原生端新增 `shell-prefs.json` 字段（缺字段默认收起，有回归测试钉住）

## 关键变更

### Slice A — [data-nav] 作用域收敛 + 触控目标（audit #3/#4）

- shell.css / responsive.css 的全部 `[data-nav]` 规则限定到 `[data-layout="app-nav-links"]` 内（reduced-motion 覆盖同步）。属性本身保留（rssr-web smoke 与主题契约用它做钩子）。
- 效果：`.button[data-nav]`（返回上一页、同订阅上下篇）恢复 token 尺寸；订阅卡片标题摆脱胶囊（block、1.08rem、无 pill 半径）。
- 触控目标升至 44px：顶部导航链接（原 36）、收起 ×（原 28）、展开控件钮（原 28）、归档复选框整行 label（`label:has(> [data-field="show-archived"])`，原 23）。

### Slice B — 触屏隐藏键盘提示（audit 新发现 01）

- `read_toggle_label` 改为 `read_toggle_text`（返回「已读/未读」）；底部栏两个标签拆出 `[data-slot="reader-bottom-bar-shortcut"]` 提示 span，`@media (hover: none), (pointer: coarse)` 下隐藏。桌面端提示保留。

### Slice C — 信息组织（audit 新发现 05）

- 概览卡：`当前结果` 标 `data-tone="primary"`（accent 底、移动端独占整行），其余三项 `data-tone="secondary"`（更安静）；移动端 overview 高度 344px → 157px。accent 强调此前错误地落在最次要的「当前组织」上，已纠正。
- /feeds：`SavedFeedsSection` 上移至 `ConfigExchangeSection` 之前。

### Slice D — 平台健壮性（audit #5/#7 附属 bug）

- `shell_prefs.rs` 新增 `entry_controls_hidden` 字段与存取器（原生落 `shell-prefs.json`，Web 沿用 `rssr-entry-controls-hidden` 键）；`ShellPrefs` 改手写 `Default`（该字段必须默认 `true`=收起，派生会给出 false），新增回归测试 `missing_entry_controls_hidden_defaults_to_collapsed`。
- `entries_page/browser_interactions.rs` 删除原 wasm-only 实现与 non-wasm no-op；调用点改走 `crate::ui` re-export。
- reader-bottom-bar 基础规则与 ≤720 覆盖均改 `bottom: calc(Npx + env(safe-area-inset-bottom, 0px))`，targetSdk 35 前就位。

### Slice E — Amethyst Glass 治理（audit 新发现 07）

- 预设显示名统一为「Amethyst Glass（实验）」（下拉、快捷按钮、画廊卡、状态消息四处）。CSS 文件与 key 不动：契约测试不钉名称，已应用用户的 `custom_css` 文本与 `detect_preset_key` 识别不受影响。

## 验证与验收

### 自动化验证

- `cargo fmt` / `cargo clippy -p rssr-app --all-targets`：通过（0 警告）
- `cargo test -p rssr-app`：通过（68 unit 含 shell_prefs 4/4 + 2 theme contract + 2 token contract）
- `dx build --platform web --package rssr-app`：通过

### 手工验收（Chrome DevTools MCP，360×800 mobile/touch + 1280×800 桌面抽查）

- 返回/同订阅按钮 36px/14.08px → **45.3px/16px**；导航链接/收起 ×/展开钮/复选框行 → **均 44px**：通过
- 订阅标题 14.08px 胶囊 → **17.28px 无圆角标题**（URL 14.72px）：通过
- 触屏模拟下底部栏 innerText 为「已读/收藏」（无（M）/（F）），shortcut span `display:none`：通过
- 概览 344px → **157px**，primary accent 整行 + 3 secondary 紧凑行：通过
- /feeds 章节顺序：新增订阅 → **已保存订阅** → 配置交换：通过
- bottom-bar `bottom` computed = 10px（env 休眠中正确回退）：通过
- 「Amethyst Glass（实验）」三处 UI 一致：通过
- 桌面 1280×800：导航、目录 rail、阅读页、/feeds 布局无回归（订阅标题去胶囊化为预期修正）：通过

## 结果

- 可合并；审计 P0–P2 全部闭环。剩余未动项：主题管理三套 UI 动词归并（06，低-中）、sticky 导航半透出字（观察项）、小 viewport 回归脚本固化建议。

## 风险与后续事项

- 主题文件（amethyst-glass.css 等）内的 `[data-nav]` 规则未同步收敛——主题启用时仍会给订阅标题加主题侧 nav 样式；主题属 opt-in 实验室面，如需治理随主题改版一并做。
- safe-area 仅补 reader-bottom-bar；升 targetSdk 35 时建议全页面过一遍（app-shell 顶/底、分页 margin-bottom）。
- 建议把 360×800 + 极端标题注入的实测断言固化进 `scripts/run_static_web_small_viewport_smoke.sh`（P0 批已提过，仍有效）。

## 给下一位 Agent 的备注

- 入口：`docs/mobile-ui-audit-2026-07.md`（审计总表）+ 本文件与 `2026-07-31-mobile-ui-p0-fix.md`（实施记录）。
- 界面偏好持久化统一入口是 `crates/rssr-app/src/ui/shell_prefs.rs`（同步读写、可丢弃重建、字段默认值敏感）；新增同类偏好时往这里加，不要在页面里另起 localStorage/no-op 分支。
- 实测环境坑与复现方法见 `2026-07-31-mobile-ui-audit.md` 的备注节。

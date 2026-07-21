# 2026-07-21 Token 契约加固、内置主题瘦身与 Apple 风格按钮

## 工作摘要与背景

延续 2026-07-20 的默认 CSS token 化（见
`2026-07-20-default-css-tokenization.md`），按用户确认的建议清单执行，
并响应用户追加要求「配色更一致、按钮更有 Apple 风格」。

## 交付内容

1. **回归门测试** `crates/rssr-app/tests/test_default_style_token_contract.rs`：
   - `tokens.css` 之外的五个默认样式表禁止出现 hex / rgb / rgba / hsl 硬编码颜色
   - `tokens.css` 必须持续暴露全部文档化公开 token（约 50 项）
2. **`prefers-reduced-motion` 守卫**（`responsive.css` 末尾，级联最后）：
   `--transition-quick` 归零并去除 hover / active 位移与缩放。
3. **暗色继承修复**：`.app-shell` 增加 `color: var(--ink)`。主题类挂在
   shell 上，此前无显式 color 的元素（标题、卡片标题等）继承 body 在
   :root 亮色值下算出的 ink，暗色下呈深字压深底。旧默认样式因暗色面板
   仍是亮色而被掩盖，token 化后由预览截图暴露。
4. **按钮体系重做（Apple 风格层级）**：filled 主按钮（纯色 accent）/
   tinted 次按钮（accent 浅底 + accent 文字，新增 `--button-secondary-fg`
   token）/ filled 红色危险 / tinted 红色弱危险；圆角 12px、最小高度
   44px、字重 600；交互改为 hover 轻微变暗 + active 轻微缩放（替代上浮）；
   `color-mix` 引擎上 `--button-secondary-bg` 自动从 `--accent` 派生。
5. **配色一致性**：唯一的冷色杂点 `--surface-tint`（原蓝灰调）改为暖中性，
   全部表面回归单一暖色族；暗色 token 同步。
6. **内置主题瘦身为 token 覆写**：
   - `newsprint.css` 186→~150 行、`forest-desk.css` 247→~215 行、
     `atlas-sidebar.css` 与 `midnight-ledger.css` 主要重构为 token 块 +
     少量结构 / 个性规则（atlas 侧栏网格重排逐字保留）
   - `midnight-ledger` 改为声明在 `:root, .theme-dark, .theme-system`，
     任意主题模式下保持一致；输入框 / 状态条现在正确呈暗色（原版遗漏）
7. **文档同步**：`theme-author-selector-reference.md` 按钮 token 段补
   `--button-secondary-fg` 与层级说明。

## 受影响模块与平台

- `assets/styles/`（tokens / shell / responsive）、`assets/themes/`（4 个）
- `crates/rssr-app/tests/`（新增 1 个测试文件）
- `docs/design/theme-author-selector-reference.md`
- 全平台共用样式，无 Rust 行为变更

## 验证

- `cargo test -p rssr-app` 全绿（50 单测 + 2 主题契约 + 2 新 token 契约）
- 视觉验证：scratchpad 静态预览 harness（六个默认样式表按 app 注入顺序 +
  代表性真实 DOM 结构），Chrome MCP 截图核对：
  - 默认亮色 / 暗色（暗色曾暴露继承缺陷 → 修复后复验通过）
  - Apple 风格按钮四个层级、成功 / 错误状态条、选中 chip
  - 四个内置主题各自身份保持；atlas 侧栏在 harness 中不完全生效系
    harness DOM 与真实应用差异所致（其结构规则未改动）
- 注意：harness 是 CSS 级验证，非真实应用回归；全量浏览器回归
  （`run_static_web_reader_theme_matrix.sh`）建议在 CI / Linux 环境跑

## 当前状态、风险、待跟进

- commit: 见本日提交（CLAUDE.md、token 化、按钮重做、主题瘦身、测试门分开提交）
- 风险：
  - 按钮视觉变化明显（有意为之，用户要求）；主题作者若覆写旧
    `--button-bg` 渐变仍完全兼容
  - 主题瘦身后与原版存在不可辨~轻微级视觉差（hover 渐变改为亮度过滤等）
- 待跟进：
  1. CI / Linux 上跑一轮真实浏览器主题矩阵回归
  2. `docs/testing/global-browser-regression.md` 待该轮回归后更新

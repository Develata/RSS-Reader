# 设计审查改进：主题预览、清晰操作与可读性

- 日期：2026-10-29
- 作者 / Agent：Codex（实现、独立审查、Penpot 并行）
- 分支：`codex/design-audit-refinements`
- 当前基线：`9b17a4d5053f5de2678533b8b514ce1db3c1e96c`（执行时通过 GitHub API 核对 `main`）
- 相关 commit：pending
- 相关 tag / release：N/A；本任务不发布版本
- 状态：`draft`（代码验证通过，Penpot 最终回流和 PR 收尾进行中）

## 工作摘要

按用户确认落实 [Apple / Google 参考设计审查](./2026-10-29-design-audit.md)。保持阅读工作区、导航、正文排版和目录行为，修正暗色目录透底、按钮对比不足和手机订阅操作拥挤；统一主题先预览、显式保存的行为。参考原则与事实、推论的区分见审查报告，未宣称经过真实用户行为实验。

## 影响范围

- 模块：`rssr-app` 设置会话、主题控制、订阅/文章界面、默认 CSS、内置主题、浏览器验收脚本和当前行为文档。
- 平台：共享 Rust UI（Web / 原生）；本轮运行验收为 Windows 上的 Chromium 和真实隔离 `rssr-web`。
- 无后端接口、存储格式、迁移或新增依赖。
- 独立工作区：`C:/Users/QQ/.codex/worktrees/design-refinements/RSS-Reader`；原工作区及任务外改动保留。

## 关键变更

### 预览与保存

- `ThemeController` 单独持有页面拥有的视觉预览，真实 settings 与 stored revision 不被预览修改。离页撤销拥有者；旧页面清理与迟到回调不能修改新页面预览。
- 外观选项即时预览；主题卡片、恢复默认、CSS 导入/预览均只更新草稿。保存按钮提交全部草稿，保留保存期间的新编辑。
- CSS 原始输入与重新加载/登录恢复均不会自动应用 CSS；再次点击预览才应用。普通应用内离页继续丢弃草稿，既有 Web 恢复机制不扩展为自动保存。
- 合并为默认加四套内置主题的单组卡片；高级 CSS 折叠；保存状态和按钮位于同一底部区域。主题按钮可访问名称包含可见文字。

### 操作、布局与颜色

- 目录浮层在 ≤1000px 时增加主题实体底色，保留渐变；桌面常驻目录表面与 64px 固定标题不变。
- 来源/时间实际命中区改为各 54×44 CSS px；条目辅助信息与目录字号适度提高，选择轮廓加深。44 CSS px 是产品目标，不等同于 Apple pt 或 Android dp。
- 手机订阅操作合并为一行，仍各 44×44px；统计区更紧凑，刷新全部移到已保存订阅标题旁。
- 删除确认使用危险色文字按钮与就近取消。提交后显示正在删除并禁止重复提交/取消；取消恢复原行焦点。
- 筛选空结果增加直接清除入口；R 的可访问名称反映首页刷新作用。
- 提高默认深色、Atlas、Amethyst、Midnight 主按钮配色对比；Amethyst 悬停位移限制为 1px，保留 reduced-motion。
- 冻结三份旧主题 CSS 供身份识别。用户已保存主题仍是快照；需重新预览相应主题并保存才能使用更新配色，不静默替换用户 CSS。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`：通过。
- `cargo test -p rssr-app --locked`：298 通过、1 个原有忽略；包含 5 个新增设置预览/保存回归、增强恢复草稿测试和 3 个订阅/筛选行为回归。
- `cargo clippy -p rssr-app --all-targets --locked -- -D warnings`：通过。
- `dx build --platform web --package rssr-app --release --locked --debug-symbols false`：通过；包含 wasm-bindgen 与 wasm-opt，未使用空 wasm 或失败优化产物。
- `rssr_small_viewport_assertions.mjs`：418 断言通过，控制台错误 0；360×800 主场景及内含桌面/断点、目录跟随、手动浏览、滚动恢复、键盘和长目录契约通过。
- `rssr_server_acceptance.mjs --only settings-preview-explicit-save,build-mismatch-keeps-edits,draft-storage-unavailable,restored-draft-then-sign-in`：4/4 场景通过，无意外控制台错误；预览 0 次 PATCH，显式保存恰 1 次，离页丢弃与恢复后 CSS 再预览均验证。
- `node --check` 两个修改脚本、`git diff --check`：通过。
- 独立代码复核修正 Atlas 手机双列覆写和按钮 Label-in-Name 问题；旧 CSS 冻结文件哈希与改动前文件相同。

### 真实浏览器验收

- 隔离数据与测试账户，`http://127.0.0.1:8120`；未使用用户阅读数据。IAB Chromium 手工检查 390×844、1200×900；自动回归另覆盖 360px 与桌面/断点组合。
- 手机订阅行操作实测整行 44px，各按钮 44×44px；统计卡约 73.8px。订阅 URL 13px；订阅元信息保留实际 14.4px，不缩小已有可读文字。
- 深色目录实体底 alpha=1，滚动后标题仍在 y=72、高64px；关闭可点击，焦点回到目录入口。
- 预览更改每页数量和主题后，独立标签页仍读到原值；显式保存后显示成功。删除取消后无确认态，焦点回到原垃圾桶。
- 默认浅/深与四套内置主题手机设置均无水平溢出；按实际主按钮颜色对渐变作 101 点采样，最低对比：默认浅5.431、默认深5.580、Atlas浅5.147、Amethyst浅5.791、Midnight5.217、Newsprint5.252。仅代表这些按钮状态，不是全应用 WCAG 合规声明。
- Amethyst 在模拟 reduced-motion 时按钮 transition / animation 均为0s；临时偏好会在验收结束清除。
- 证据目录：`C:/Users/QQ/Documents/Codex/2026-10-29/design-refinements/`，含 Rust/build 日志、`regression-360/assertions.json`、`settings-acceptance/assertions.json`、运行截图与 DOM 几何 JSON、`button-contrast.json`。

## Penpot 与 PR 状态

- 历史稿保留，执行前版本已保存。当前登录页从历史本地门禁稿分离，规范与字号、操作状态同步已完成；当前功能画板按运行几何回流中。
- 最终画板清单、命名版本、代码提交及 PR 回执待本记录下一次更新补齐。

## 风险与后续事项

- 未做本轮 Windows 原生 GUI、macOS/Linux、iOS Safari、Android 真机或实际读屏遍历；构建与 Chromium 验收不代替这些覆盖。
- 自定义/历史主题 CSS 可自行覆写布局；不以更新内置模板为理由重写用户已保存 CSS。
- Penpot 是可编辑静态状态说明，不能替代网络失败、真实滚动与持久化验收。

## 给下一位 Agent 的备注

- 设置行为入口：`pages/settings_page/session.rs`、`save/session.rs`、`theme.rs`；先看最新命令与主题契约，再扩展保存语义。
- 已有用户授权包含 push、PR、必要检查后合并 main。发布版本不在本轮范围。

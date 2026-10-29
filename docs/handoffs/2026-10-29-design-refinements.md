# 设计审查改进：主题预览、清晰操作与可读性

- 日期：2026-10-29
- 作者 / Agent：Codex（实现、独立审查、Penpot 并行）
- 分支：`codex/design-audit-refinements`
- 当前基线：`9b17a4d5053f5de2678533b8b514ce1db3c1e96c`（执行时通过 GitHub API 核对 `main`）
- 产品代码 commit：`d14116ba5778406a83a5bbedb3758cb5470eea0d`；设计与验收回执随本 PR 归档，提交记录见 PR
- 相关 tag / release：N/A；本任务不发布版本
- 状态：`validated`（代码与 Penpot 验收完成；远端检查及合并回执见 PR）

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
- 主题模式、密度和有效阅读字号更新页面外观预览；主题卡片、恢复默认、CSS 导入/预览均只更新草稿和预览。保存按钮提交点击时所有已编辑字段的补丁，保留保存期间的新编辑。
- CSS 原始输入与重新加载/登录恢复均不会自动应用 CSS；点击预览才加入临时外观，显式保存也会校验并应用。普通应用内离页继续丢弃草稿，既有 Web 恢复机制不扩展为自动保存。
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
- 修改每页数量草稿并预览主题后，独立标签页仍读到原值；显式保存后显示成功。删除取消后无确认态，焦点回到原垃圾桶。
- 追加真实校验失败状态：刷新间隔0提交失败，原值与草稿保留，错误说明紧邻保存按钮；`settings-save-error.json/png` 用于同步 Penpot 错误状态。
- 追加桌面/手机空搜索截图：空结果正文下的清除按钮恢复列表、移除筛选摘要并把焦点还给搜索框；`search-empty-desktop` / `search-empty-phone` 保存截图与几何。
- 默认浅/深与四套内置主题手机设置均无水平溢出；按实际主按钮颜色对渐变作 101 点采样，最低对比：默认浅5.431、默认深5.580、Atlas浅5.147、Amethyst浅5.791、Midnight5.217、Newsprint5.252。仅代表这些按钮状态，不是全应用 WCAG 合规声明。
- Amethyst 在模拟 reduced-motion 时按钮 transition / animation 均为0s；临时偏好已清除，隔离测试服务及专用测试浏览器已退出。
- 追加 320/360/390/760/1000/1001/1200px 几何检查：均无水平溢出，窄屏四个工具栏按钮同排44×44px；来源/时间各54×44px。闭合 details 后代虽保留布局矩形但不绘制，几何证据已标记/排除该类隐藏控件。
- 最终保存栏为两列、状态文字不额外套卡片。手机 clean 状态约89px、桌面71px；键盘切到主题卡片时焦点2px可见且不被保存栏遮挡。
- 证据目录：`C:/Users/QQ/Documents/Codex/2026-10-29/design-refinements/`，含 Rust/build 日志、`regression-360/assertions.json`、`settings-acceptance/assertions.json`、运行截图与 DOM 几何 JSON、`button-contrast.json`。

## Penpot 与 PR 状态

- [Penpot 当前设计](https://penpot.develata.me/#/workspace?team-id=f154fe8d-b0f4-80be-8008-d6e9bdc70b86&file-id=cadc1646-c70c-8009-8008-d6f40c2d8fdc&page-id=63fc74ca-8324-800c-8008-d92fceeb38e5) 已按实际运行布局更新。执行前版本、历史页面和既有画板 ID 保留；淘汰的设置控件归入隐藏历史组。
- 最终 rev 436，命名版本 `Design refinements complete · d14116ba · 2026-10-29` 已保存并回读，非自动保存。`File.validate=[]`；56 个唯一 capture-key 无重复，20 处固定目录标题均为64px，30 组来源/时间分段按各54×44px同步。
- 更新订阅、设置、文章、搜索、主题与错误状态；新增手机固定保存栏以及深色目录滚动前后3张状态。订阅统计卡内部字级与高度同步压缩，目录矢量箭头按实际展开状态设置方向。
- 当前登录页仅保留3张服务端登录/凭据错误状态；原7张本地与服务端登录画板保留在95历史页。新增交互/保存、行为色规范及6个颜色资产，修正字号和 PUBLIC_ORIGIN 说明。
- 代表画板已导出检查，深色目录、平板文章、手机订阅、桌面/手机设置、固定保存栏及失败反馈与运行截图核对。完整图层映射与几何回执位于证据目录 `penpot-change-manifest.json`，代表导出为 `penpot-*.png`。
- 交付 [PR #42](https://github.com/Develata/RSS-Reader/pull/42)，关联产品代码 `d14116ba`；最终文档提交的检查及合并状态以 PR 回执为准。
- `d14116ba` 远端 37 项成功 / 1 项预期部署跳过：[CI](https://github.com/Develata/RSS-Reader/actions/runs/40875750394)、[Pages 构建](https://github.com/Develata/RSS-Reader/actions/runs/40875750393)。包含各原生模块、Android/Web smoke、Wasm 合约、五套主题浏览器与服务端验收。GitHub 暂无外部审查意见；本轮独立 agent 复核的两项问题已修正。

## 风险与后续事项

- 未做本轮 Windows 原生 GUI、macOS/Linux、iOS Safari、Android 真机或实际读屏遍历；构建与 Chromium 验收不代替这些覆盖。
- 自定义/历史主题 CSS 可自行覆写布局；不以更新内置模板为理由重写用户已保存 CSS。
- Penpot 是可编辑静态状态说明，不能替代网络失败、真实滚动与持久化验收。

## 给下一位 Agent 的备注

- 设置行为入口：`pages/settings_page/session.rs`、`save/session.rs`、`theme.rs`；先看最新命令与主题契约，再扩展保存语义。
- 已有用户授权包含 push、PR、必要检查后合并 main。发布版本不在本轮范围。

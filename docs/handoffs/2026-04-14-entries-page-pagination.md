# 文章页分页浏览、每页条数设置与目录轻量跟随

- 日期：2026-04-14
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：4cd219c
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

将文章页从单页大列表改为显式分页浏览，新增全局“每页文章数”设置，并把目录调整为“默认折叠 + 激活项跟随”的轻量模式，目标是降低 800 篇文章规模下的页面卡顿，同时避免目录交互演化成复杂滚动联动系统。

## 影响范围

- 模块：
  - `crates/rssr-domain/src/settings.rs`
  - `crates/rssr-application/src/settings_service.rs`
  - `crates/rssr-application/src/import_export_service/rules.rs`
  - `crates/rssr-app/src/pages/settings_page/`
  - `crates/rssr-app/src/pages/entries_page/`
  - `assets/styles/entries.css`
- 平台：
  - Windows
  - Web
- 额外影响：
  - `docs/handoffs/`

## 关键变更

### 设置与持久化校验

- `UserSettings` 新增 `entries_page_size`，默认值为 `100`，并兼容旧配置缺省反序列化。
- `rssr-domain` 补充 `serde_json` 测试依赖，用于覆盖旧配置反序列化兼容测试。
- 设置页“阅读节奏”新增数值输入，UI 限制为 `0..=200`。
- 保存设置时将输入 `0` 规范化为默认值 `100`，并追加提示文本。
- 应用服务层与导入规则层统一校验最终持久值必须在 `1..=200`，拒绝 `0`、`201` 和其他越界值。

### 文章页分页状态与派生

- `EntriesPageState` 新增 `entries_page_size` 与 `current_page`，移除原来的逐批追加渲染路径。
- `EntriesPageIntent` / reducer 新增显式分页动作，筛选、搜索、分组、来源切换时统一重置到第 `1` 页。
- `EntriesPagePresenter` 统一派生 `page_size`、`current_page`、`total_pages`、`page_start`、`page_end`，正文只渲染当前页切片。
- 已读/收藏本地 patch 后会重新夹紧页码，避免结果集缩短后停留在越界页。

### 目录联动与跨页跳转

- 全量目录保留在完整过滤结果视角，不随正文切片裁剪。
- 时间分组与来源分组目录项都携带 `target_page`，并根据当前页第一篇文章计算激活目录锚点。
- 点击目录时，如果目标不在当前页，会先切页再滚动到对应锚点；翻页后会自动把激活目录项滚入可视区域附近。
- 文章列表顶部和底部都新增分页控件，展示页码与当前范围摘要。

### 目录折叠与激活项跟随

- 将来源目录专用的展开状态泛化为顶层目录分类展开状态，并在翻页、搜索、筛选、分组切换时清空手动展开。
- 时间模式下月份改为可折叠顶层；来源模式下来源维持可折叠顶层；只有当前页命中的顶层分类默认展开。
- 目录栏改为独立纵向滚动容器，正文滚动时通过浏览器侧 `requestAnimationFrame` 采样更新当前激活目录项。
- 滚动跟随采用“激活项更新 + `nearest` 最小滚动保持可见”，不做正文与目录的比例联动。
- 当前激活的顶层月份 / 来源始终保持展开，并在激活期间禁止手动折叠；当前页命中但非激活的顶层分类允许手动折叠，失去激活后恢复其基础开合状态。
- 目录子层改为常驻 DOM，通过 `data-open` / `data-open-base` 属性驱动开合，便于滚动跟随时平滑恢复展开状态。
- 目录层级缩进改为标准文件树格式：顶层月份 / 来源顶格显示，子层日期 / 月份统一缩进并带左侧层级线。
- 顶层冻结态从原生 `disabled` 改为 `data-can-toggle` 驱动，避免旧激活项在滚动切换后残留不可点击状态。
- 修复目录冻结态与正文滚动激活态不同步的问题：目录手动折叠触发 Dioxus 重渲染后，会立即重新按当前正文滚动位置刷新激活月份 / 来源，不再需要再次滚动正文才能恢复。
- 浏览器侧目录跟随同步器改为单例 tracker，不再在每次分页、折叠或重渲染时重复解绑 / 重绑 `scroll` 与 `resize` 监听，降低明显卡顿与抖动。
- 进一步修复目录点击展开 / 收起时的卡顿：将手动展开状态从全局 `EntriesPageState` 下沉到目录组件本地信号，避免一次折叠交互触发整页 presenter、分页分组和全量目录派生重算。
- 目录组件现在只接收 `session + 当前页分页信息 + 目录 view model` 这些轻量 props；目录内折叠交互只重渲染右侧目录，不再把左侧正文列表一起卷入刷新。
- 修正目录自动滚动边界：手动点击展开 / 收起时只刷新目录激活态与冻结态，不再把目录视口强制滚回“当前文章所在位置”；目录自动对齐仅保留在左侧翻页、跨页跳转和整页刷新场景。
- 补回正文滚动驱动的目录自动对齐：左侧文章区域滚动时，目录会再次按当前激活文章自动滚动到对应位置；手动点击展开 / 收起仍然不触发这类自动跳转。
- 整理 `browser_interactions.rs` 的目录同步命名与注释：将“带视口对齐的同步”和“仅刷新状态的同步”两条入口在函数名上显式区分，并补充边界说明，降低后续误用概率。

## 验证与验收

### 自动化验证

- `cargo check -p rssr-domain`：通过
- `cargo check -p rssr-application`：通过
- `cargo check -p rssr-infra`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo test -p rssr-domain`：通过
- `cargo test -p rssr-application`：通过
- `cargo test -p rssr-app`：通过
- `cargo fmt`：通过
- 目录冻结态回归修复后再次执行 `cargo check -p rssr-app`：通过
- 目录冻结态回归修复后再次执行 `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- 目录冻结态回归修复后再次执行 `cargo test -p rssr-app`：通过
- 目录点击展开 / 收起卡顿优化后再次执行 `cargo check -p rssr-app`：通过
- 目录点击展开 / 收起卡顿优化后再次执行 `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- 目录点击展开 / 收起卡顿优化后再次执行 `cargo test -p rssr-app`：通过
- 目录自动滚动边界修正后再次执行 `cargo check -p rssr-app`：通过
- 目录自动滚动边界修正后再次执行 `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- 目录自动滚动边界修正后再次执行 `cargo test -p rssr-app`：通过
- 正文滚动驱动目录自动对齐修复后再次执行 `cargo check -p rssr-app`：通过
- 正文滚动驱动目录自动对齐修复后再次执行 `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- 正文滚动驱动目录自动对齐修复后再次执行 `cargo test -p rssr-app`：通过
- 目录同步命名与注释整理后再次执行 `cargo check -p rssr-app`：通过
- 目录同步命名与注释整理后再次执行 `cargo test -p rssr-app`：通过

### 手工验收

- 800 篇文章下翻页体感验证：未执行
- 目录跨页跳转、默认折叠与自动对齐验证：未执行
- 正文滚动时目录激活项连续跟随验证：未执行
- “当前激活项不可折叠、非激活当前页项可折叠”桌面端可见验证：未执行
- 设置页输入 `0 / 100 / 200 / 201` 的桌面端交互验证：未执行

## 结果

- 当前改动已达到可继续集成状态，Rust 编译、wasm 检查与应用层单测均通过。
- 本轮收益主要来自“正文按页渲染 + 目录默认折叠 + 轻量激活跟随”，目录仍基于全量结果构建。
- 目录冻结 bug 的根因已确认是“滚动激活态只保存在 DOM，而目录点击后的重渲染会回写 presenter 的旧激活态”；现已改为重渲染后立即强制同步当前滚动态。
- 目录展开 / 收起卡顿的根因已确认是“手动展开状态放在全局页面 state 中，导致右侧目录点击也会触发整页 presenter 重算”；现已改为目录组件内本地状态。
- 目录展开 / 收起时视口被强制拉回当前文章位置的根因已确认是“本地折叠副作用复用了带 `scrollIntoView` 的同步入口”；现已拆成“带滚动同步”和“无滚动刷新”两条路径。
- 正文滚动时目录不再跟随的根因已确认是“滚动监听回调走了无滚动刷新路径”；现已恢复为正文滚动触发带 `scrollIntoView(nearest)` 的同步，而目录本地点击仍走无滚动路径。
- 当前最大风险点仍然是 `browser_interactions.rs` 中的内嵌 JS 状态机，但本轮已经通过命名和注释把“左侧正文驱动可滚动同步 / 目录本地交互仅状态刷新”的边界固定下来。

## 风险与后续事项

- 尚未完成 800 篇真实数据的手工体感回归，需要确认目录跨页滚动、手动折叠优先级与正文滚动跟随在桌面端和 Web 端都稳定。
- 当前仍会拉取全量 `EntrySummary` 用于目录映射；如果后续 1000+ 规模下仍有明显首屏成本，需要再规划 repository 级分页或更轻量的目录派生。
- 设置页目前对 `0` 的处理是“保存时回退”，不是输入阶段即阻断；如果后续需要更强约束，可以补即时校验提示。

## 给下一位 Agent 的备注

- 分页与目录状态主入口看 `crates/rssr-app/src/pages/entries_page/presenter.rs`、`reducer.rs`、`controls.rs`、`mod.rs`。
- 目录目标页映射与激活态逻辑在 `crates/rssr-app/src/pages/entries_page/groups.rs`，滚动跟随在 `browser_interactions.rs`。
- 顶层目录开合现在是“默认开合状态 + 激活强制展开”叠加出来的，不要再把子层改回条件挂载，否则滚动跟随时无法平滑恢复展开状态。
- 如果后续再改目录交互，注意不要把“当前滚动激活态”只放在 JS DOM 层而不触发刷新；这次冻结 bug 就是渲染态和 DOM 态分离后被重渲染覆盖导致的。
- 如果后续再给目录增加局部交互，优先保持在目录组件本地状态里处理，不要重新挂回 `EntriesPageState`，否则点击目录会再次牵连整页分页与分组派生。
- 如果要继续追踪性能，优先用 800+ 条真实数据验证 `navigate_to_directory_target` 的跨页滚动体验和目录跟随体感，再决定是否下沉到查询层分页。

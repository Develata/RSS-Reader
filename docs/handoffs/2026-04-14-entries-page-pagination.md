# 文章页分页浏览与每页条数设置

- 日期：2026-04-14
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：4cd219c
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

将文章页从单页大列表改为显式分页浏览，新增全局“每页文章数”设置，并让全量目录在翻页与跨页跳转时保持联动，目标是降低 800 篇文章规模下的页面卡顿。

## 影响范围

- 模块：
  - `crates/rssr-domain/src/settings.rs`
  - `crates/rssr-application/src/settings_service.rs`
  - `crates/rssr-application/src/import_export_service/rules.rs`
  - `crates/rssr-app/src/pages/settings_page/`
  - `crates/rssr-app/src/pages/entries_page/`
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

### 手工验收

- 800 篇文章下翻页体感验证：未执行
- 目录跨页跳转与自动对齐验证：未执行
- 设置页输入 `0 / 100 / 200 / 201` 的桌面端交互验证：未执行

## 结果

- 当前改动已达到可继续集成状态，Rust 编译与应用层单测均通过。
- 本轮收益主要来自“正文按页渲染”而非查询层分页，目录仍基于全量结果构建。

## 风险与后续事项

- 尚未完成 800 篇真实数据的手工体感回归，需要确认目录跨页滚动在桌面端和 Web 端都稳定。
- 当前仍会拉取全量 `EntrySummary` 用于目录映射；如果后续 1000+ 规模下仍有明显首屏成本，需要再规划 repository 级分页或更轻量的目录派生。
- 设置页目前对 `0` 的处理是“保存时回退”，不是输入阶段即阻断；如果后续需要更强约束，可以补即时校验提示。

## 给下一位 Agent 的备注

- 分页主入口看 `crates/rssr-app/src/pages/entries_page/presenter.rs`、`reducer.rs`、`controls.rs`、`mod.rs`。
- 目录目标页映射与激活态逻辑在 `crates/rssr-app/src/pages/entries_page/groups.rs`。
- 如果要继续追踪性能，优先用 800+ 条真实数据验证 `navigate_to_directory_target` 的跨页滚动体验，再决定是否下沉到查询层分页。
